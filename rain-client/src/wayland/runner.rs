use std::thread::JoinHandle;

use calloop::channel::Sender;
use tracing::error;

use super::Command;

/// A wrapper to handle the Wayland thread.
///
/// This struct must be kept in the main scope, because the `drop()` will quit
/// the Wayland thread.
pub struct Runner {
    thread_handle: Option<JoinHandle<()>>,
    commands_sender: Sender<Command>,
}

impl Runner {
    /// Create a new runner by the Wayland thread handler and the
    /// `commands_sender` to send the [`Command::Quit`] message.
    pub(crate) fn new(thread_handle: JoinHandle<()>, commands_sender: Sender<Command>) -> Self {
        Self {
            thread_handle: Some(thread_handle),
            commands_sender,
        }
    }

    /// Return the current Wayland thread status.
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.thread_handle
            .as_ref()
            .map(|handle| handle.is_finished())
            .unwrap_or(true)
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        let _ = self.commands_sender.send(Command::Quit);

        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                error!("The Wayland thread failed to quit: {:?}", e);
            }
        }
    }
}
