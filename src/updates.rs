//! Update checks are performed periodically (when enabled).
//!
//! This module runs checks in a thread and remembers the last time the check ran and the most
//! recent version available.

use self::persist::{Error as PersistError, Persist};
use crate::framework::UserEvent;
use crate::timer::Timer;
use log::error;
use semver::Version;
use serde::Deserialize;
use std::any::Any;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::JoinHandle;
use std::time::Duration;
use thiserror::Error;
use winit::event_loop::EventLoopProxy;

mod persist;

const HTTP_TIMEOUT: u64 = 15;
const RELEASES_URL: &str = "https://api.github.com/repos/parasyte/cartunes/releases/latest";
const USER_AGENT: &str = concat!("cartunes/", env!("CARGO_PKG_VERSION"));

/// All the ways in which update checking can fail.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// The update thread may panic.
    #[error("Update thread panicked")]
    ThreadPanic(Box<dyn Any + Send + 'static>),

    /// Stopping the update thread may not succeed.
    #[error("Unable to stop update thread")]
    Stop,

    /// Parsing or writing persistence may fail.
    #[error("Persistence error: {0}")]
    Persist(#[from] PersistError),
}

/// How often to check for updates.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum UpdateFrequency {
    /// Do not check for updates. (default)
    Never,

    /// Check every 24 hours.
    Daily,

    /// Check every 7 days.
    Weekly,
}

#[derive(Debug, Copy, Clone)]
enum UpdateCheckerMessage {
    Stop,
    Ping,
    Timeout,
}

/// Offers update checking functionality.
pub(crate) struct UpdateChecker {
    thread: JoinHandle<()>,
    sender: SyncSender<UpdateCheckerMessage>,
}

/// The thread container for update checking. This does all the actual work.
struct UpdateCheckerThread {
    event_loop_proxy: EventLoopProxy<UserEvent>,
    sender: SyncSender<UpdateCheckerMessage>,
    receiver: Option<Receiver<UpdateCheckerMessage>>,
    duration: Duration,
    persist: Persist,
}

/// Parsed API response body.
#[derive(Debug, Deserialize)]
pub(crate) struct ReleaseBody {
    name: String,
    body: String,
    html_url: String,
}

/// Update notification.
#[derive(Debug)]
pub(crate) struct UpdateNotification {
    pub(crate) version: Version,
    pub(crate) release_notes: String,
    pub(crate) update_url: String,
}

impl Default for UpdateFrequency {
    fn default() -> Self {
        Self::Never
    }
}

impl std::fmt::Display for UpdateFrequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Never => "Never",
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
        };
        write!(f, "{}", text)
    }
}

impl From<&str> for UpdateFrequency {
    fn from(value: &str) -> Self {
        match value {
            "daily" => Self::Daily,
            "weekly" => Self::Weekly,
            _ => Self::Never,
        }
    }
}

impl UpdateFrequency {
    /// Convert this frequency into a [`Duration`].
    ///
    /// Returns `None` when the frequency is `Never`.
    fn into_duration(self) -> Option<Duration> {
        const DAY: u64 = 60 * 60 * 24;
        const WEEK: u64 = DAY * 7;

        match self {
            Self::Never => None,
            Self::Daily => Some(Duration::from_secs(DAY)),
            Self::Weekly => Some(Duration::from_secs(WEEK)),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Never => "never",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
        }
    }
}

/// Check the GitHub API periodically for a new version.
impl UpdateChecker {
    /// Create an update checker.
    ///
    /// Returns `None` when `freq` == `Never`.
    pub(crate) fn new(
        event_loop_proxy: EventLoopProxy<UserEvent>,
        freq: UpdateFrequency,
    ) -> Result<Option<Self>, Error> {
        let duration = match freq.into_duration() {
            None => return Ok(None),
            Some(duration) => duration,
        };
        let (sender, receiver) = sync_channel(2);
        let thread =
            UpdateCheckerThread::new(event_loop_proxy, sender.clone(), receiver, duration)?;
        let thread = std::thread::spawn(move || thread.run());

        Ok(Some(Self { thread, sender }))
    }

    /// Stop the update checker.
    pub(crate) fn stop(self, blocking: bool) -> Result<(), Error> {
        self.sender
            .send(UpdateCheckerMessage::Stop)
            .map_err(|_| Error::Stop)?;

        if blocking {
            self.thread.join().map_err(|err| Error::ThreadPanic(err))?;
        }

        Ok(())
    }
}

impl UpdateCheckerThread {
    /// Create a thread for the update checker.
    fn new(
        event_loop_proxy: EventLoopProxy<UserEvent>,
        sender: SyncSender<UpdateCheckerMessage>,
        receiver: Receiver<UpdateCheckerMessage>,
        duration: Duration,
    ) -> Result<Self, Error> {
        // TODO: Load from save directory...
        let persist = Persist::new()?;

        Ok(Self {
            event_loop_proxy,
            sender,
            receiver: Some(receiver),
            duration,
            persist,
        })
    }

    // TODO: Replace this async runner with a simple threaded runner.
    // - Needs mpsc with a message type for:
    //   - Stop
    //   - HTTP response
    //   - HTTP request timeout
    /// Periodically check for updates.
    fn run(mut self) {
        let mut _timer = Timer::new(
            self.duration,
            self.sender.clone(),
            UpdateCheckerMessage::Ping,
            UpdateCheckerMessage::Timeout,
        );

        // Send update notification on startup if it has been persisted
        self.send_update_notification();

        // Perform initial update check
        self.check();

        for msg in self.receiver.take().expect("Missing receiver").iter() {
            match dbg!(msg) {
                UpdateCheckerMessage::Stop => break,
                UpdateCheckerMessage::Ping => continue,
                UpdateCheckerMessage::Timeout => {
                    _timer = Timer::new(
                        self.duration,
                        self.sender.clone(),
                        UpdateCheckerMessage::Ping,
                        UpdateCheckerMessage::Timeout,
                    );

                    self.check();
                }
            }
        }
    }

    /// Check for the latest version.
    fn check(&mut self) {
        // Check last update time
        match self.persist.last_check() {
            Ok(last_check) => {
                if last_check < self.duration {
                    return;
                }
            }
            Err(error) => {
                error!("SystemTime error: {:?}", error);
                return;
            }
        }

        // Send API request
        let req = ureq::get(RELEASES_URL);
        let req = req.timeout(Duration::from_secs(HTTP_TIMEOUT));
        let req = req.set("Accept", "application/vnd.github.v3+json");
        let req = req.set("User-Agent", USER_AGENT);

        let res = match req.call() {
            Ok(res) => res,
            Err(error) => {
                error!("HTTP request error: {:?}", error);
                return;
            }
        };

        // Parse the response
        let body: ReleaseBody = match res.into_json() {
            Ok(body) => body,
            Err(error) => {
                error!("HTTP response error: {:?}", error);
                return;
            }
        };

        // Parse the version in the response
        let version = match Version::parse(&body.name) {
            Ok(version) => version,
            Err(error) => {
                error!("SemVer parse error: {:?}", error);
                return;
            }
        };

        // Save the last update time
        if let Err(error) = self.persist.update_last_check() {
            error!("SystemTime error: {:?}", error);
            return;
        }

        // Update persistence
        self.persist.update_last_version(version);
        self.persist
            .update_release_notes(body.body.replace("\r", ""));
        self.persist.update_url(body.html_url);

        // Write persistence to the file system
        if let Err(error) = self.persist.write_toml() {
            error!("Persistence error: {:?}", error);
            return;
        }

        // Send the update notification
        self.send_update_notification();
    }

    fn send_update_notification(&self) {
        // Check last update version
        if self.persist.last_version() > self.persist.current_version() {
            // Notify user of the new update
            self.event_loop_proxy
                .send_event(UserEvent::UpdateAvailable(
                    self.persist.get_update_notification(),
                ))
                .expect("Event loop must exist");
        }
    }
}
