//! Asynchronous timers.

use std::marker::PhantomData;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Simple async timer.
pub(crate) struct Timer<T> {
    _thread: JoinHandle<()>,
    _phantom: PhantomData<T>,
}

impl<T> Timer<T>
where
    T: Copy + Send + 'static,
{
    /// Create a timer that will expire after some duration.
    pub(crate) fn new(
        duration: Duration,
        sender: SyncSender<T>,
        ping_message: T,
        stop_message: T,
    ) -> Self {
        let start = Instant::now();
        let _thread = std::thread::spawn(move || {
            Self::run(start, duration, sender, ping_message, stop_message)
        });

        Self {
            _thread,
            _phantom: PhantomData,
        }
    }

    /// Runs a thread that just sleeps.
    ///
    /// The thread will periodically check if the receiver has been dropped. This allows timers to
    /// be canceled without creating a large number of sleeping zombie threads. AKA brain-dead
    /// garbage collection.
    fn run(
        start: Instant,
        duration: Duration,
        sender: SyncSender<T>,
        ping_message: T,
        stop_message: T,
    ) {
        let one_minute = Duration::from_secs(60);

        // Wake up every minute (max) to check if the receiving side has been closed
        loop {
            std::thread::sleep(duration.min(one_minute));

            if Instant::now().duration_since(start) >= duration {
                break;
            }
            let ping = sender.try_send(ping_message);
            if let Err(TrySendError::Disconnected(_)) = ping {
                break;
            }
        }

        sender.send(stop_message).ok();
    }
}
