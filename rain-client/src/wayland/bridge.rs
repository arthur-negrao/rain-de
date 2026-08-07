use std::io;
use std::sync::{OnceLock, mpsc};

use calloop::channel::channel as calloop_channel;
use kanal::bounded_async;
use tracing::{error, info};

use super::command::Command;
use super::dispatcher::WaylandState;
use super::event::Event;

/// avoid more than 1 init
static WAYLAND_STATE_STARTED: OnceLock<()> = OnceLock::new();

/// A abstraction to create connection with Wayland Thread.
#[derive(Debug, Clone)]
pub struct Bridge {
    events_receiver: kanal::AsyncReceiver<Event>,
    commands_sender: calloop::channel::Sender<Command>,
}

impl Bridge {
    /// A function to init the wayland thread listener and returns a
    /// [`Bridge`] to send commands and receive events from Wayland.
    ///
    /// # Cautions
    ///
    /// If the method is called more than 1 time in all program process, it will
    /// returns a Error.
    pub fn new() -> io::Result<Self> {
        if WAYLAND_STATE_STARTED.set(()).is_err() {
            let error_msg = "The Wayland Bridge has already started! Can not start the Bridge more than 1 time. Use `clone()` instead of `new()`.";

            error!("{}", error_msg);

            return Err(io::Error::new(io::ErrorKind::AlreadyExists, error_msg));
        }

        info!("Starting the Wayland Thread.");

        let (events_sender, events_receiver) = bounded_async(32);
        let (commands_sender, commands_receiver) = calloop_channel();

        // init the wayland loop
        WaylandState::new(events_sender, commands_receiver);

        Ok(Self {
            events_receiver,
            commands_sender,
        })
    }

    /// Send a command to Wayland Thread.
    ///
    /// The `cmd` is a [`Command`] to control a window client behavior.
    ///
    /// The method returns a [`std::sync::mpsc::SendError`] if the
    /// [`calloop::channel::Channel`] was disconnected in Wayland Thread.
    pub fn send(&self, cmd: impl Into<Command>) -> Result<(), mpsc::SendError<Command>> {
        self.commands_sender.send(cmd.into())
    }

    /// Receive a [`Event`] from Wayland Thread.
    ///
    /// Receive a event emitted by a window client. The Wayland Thread will
    /// send the [`Event`] or a [`kanal::ReceiveError`].
    pub async fn recv(&self) -> Result<Event, kanal::ReceiveError> {
        self.events_receiver.recv().await
    }
}
