//! Update checks are performed periodically (when enabled).
//!
//! This module runs checks in a thread and remembers the last time the check ran and the most
//! recent version available.

use self::persist::{Error as PersistError, Persist};
use crate::framework::UserEvent;
use crate::timer::Timer;
use futures_channel::oneshot::{self, Receiver, Sender};
use futures_util::future::FutureExt;
use http_client::{h1::H1Client, HttpClient, Request};
use http_types::convert::Deserialize;
use log::error;
use semver::Version;
use std::any::Any;
use std::thread::JoinHandle;
use std::time::Duration;
use thiserror::Error;
use winit::event_loop::EventLoopProxy;

mod persist;

const HTTP_TIMEOUT: u64 = 15;
const RELEASES_URL: &str = "https://api.github.com/repos/parasyte/cartunes/releases/latest";

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

    /// Check ever 7 days.
    Weekly,
}

/// Offers update checking functionality.
pub(crate) struct UpdateChecker {
    thread: JoinHandle<()>,
    sender: Sender<()>,
}

/// The thread container for update checking. This does all the actual work.
struct UpdateCheckerThread {
    client: H1Client,
    event_loop_proxy: EventLoopProxy<UserEvent>,
    receiver: Option<Receiver<()>>,
    duration: Duration,
    persist: Persist,
}

/// Parsed API response body.
#[derive(Debug, Deserialize)]
pub(crate) struct ReleaseBody {
    pub(crate) name: String,
    pub(crate) body: String,
    pub(crate) html_url: String,
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
        let (sender, receiver) = oneshot::channel();
        let thread = UpdateCheckerThread::new(event_loop_proxy, Some(receiver), duration)?;
        let thread = std::thread::spawn(move || thread.run());

        Ok(Some(Self { thread, sender }))
    }

    /// Stop the update checker.
    pub(crate) fn stop(self) -> Result<(), Error> {
        self.sender.send(()).map_err(|_| Error::Stop)?;
        self.thread.join().map_err(|err| Error::ThreadPanic(err))?;

        Ok(())
    }
}

impl UpdateCheckerThread {
    /// Create a thread for the update checker.
    fn new(
        event_loop_proxy: EventLoopProxy<UserEvent>,
        receiver: Option<Receiver<()>>,
        duration: Duration,
    ) -> Result<Self, Error> {
        // TODO: Load from save directory...
        let persist = Persist::new()?;

        Ok(Self {
            client: H1Client::default(),
            event_loop_proxy,
            receiver,
            duration,
            persist,
        })
    }

    /// Periodically check for updates.
    fn run(mut self) {
        let future = async {
            let mut stop = self.receiver.take().expect("Missing receiver").fuse();

            loop {
                // Timeout for HTTP requests
                let timer = Timer::new(Duration::from_secs(HTTP_TIMEOUT));

                futures_util::select! {
                    _ = timer.sleep().fuse() => continue,
                    _ = stop => break,
                    _ = self.check().fuse() => (),
                }

                // Sleep for the total duration
                let timer = Timer::new(self.duration);

                futures_util::select! {
                    _ = timer.sleep().fuse() => (),
                    _ = stop => break,
                }
            }
        };

        pollster::block_on(future);
    }

    /// Check for the latest version.
    async fn check(&mut self) {
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
        let mut req = Request::get(RELEASES_URL);
        req.insert_header("Accept", "application/vnd.github.v3+json");
        req.insert_header(
            "User-Agent",
            concat!("cartunes/", env!("CARGO_PKG_VERSION")),
        );

        let mut res = match self.client.send(req).await {
            Ok(res) => res,
            Err(error) => {
                error!("HTTP request error: {:?}", error);
                return;
            }
        };

        // Parse the response
        let mut body: ReleaseBody = match res.body_json().await {
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

        // Check last update version
        if &version > self.persist.last_version() {
            self.persist.update_last_version(version);

            // Remove carriage-return characters
            body.body = body.body.replace("\r", "");

            // Notify user of the new update
            self.event_loop_proxy
                .send_event(UserEvent::UpdateAvailable(body))
                .expect("Event loop must exist");
        }

        // Save the last update time
        if let Err(error) = self.persist.update_last_check() {
            error!("SystemTime error: {:?}", error);
            return;
        }

        // Write persistence to the file system
        if let Err(error) = self.persist.write_toml() {
            error!("Persistence error: {:?}", error);
        }
    }
}
