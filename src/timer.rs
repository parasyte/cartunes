//! Asynchronous timers.

use futures_channel::oneshot::{self, Receiver, Sender};
use std::pin::Pin;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Simple async timer.
pub(crate) struct Timer {
    _thread: JoinHandle<()>,
    receiver: Receiver<()>,
}

impl Timer {
    /// Create an async timer that will expire after some duration.
    pub(crate) fn new(duration: Duration) -> Self {
        let start = Instant::now();
        let (sender, receiver) = oneshot::channel();
        let _thread = std::thread::spawn(move || Self::run(start, duration, sender));

        Self { _thread, receiver }
    }

    /// Runs a thread that just sleeps.
    ///
    /// The thread will periodically check if the receiver has been dropped. This allows timers to
    /// be canceled without creating a large number of sleeping zombie threads. AKA brain-dead
    /// garbage collection.
    fn run(start: Instant, duration: Duration, sender: Sender<()>) {
        let one_minute = Duration::from_secs(60);

        // Wake up every minute (max) to check if the receiving side has been closed
        loop {
            std::thread::sleep(duration.min(one_minute));

            if Instant::now().duration_since(start) >= duration || sender.is_canceled() {
                break;
            }
        }

        sender.send(()).ok();
    }

    /// Asynchronously sleep for the timer's full duration.
    pub(crate) async fn sleep(mut self) {
        let receiver = Pin::new(&mut self.receiver);
        receiver.await.ok();
    }
}
