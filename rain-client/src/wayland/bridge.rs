use std::io;
use std::sync::mpsc;

use calloop::channel::channel as calloop_channel;
use kanal::bounded_async;
use tracing::info;

use super::command::Command;
use super::dispatcher::{Protocols, WaylandState};
use super::event::Event;
use super::runner::Runner;

/// A abstraction to create connection with Wayland Thread.
///
/// The [`Bridge`] can send and receive messages from Wayland thread and can
/// be shared with other threads. This behavior makes it possible consumes and
/// interact with wayland protocols.
#[derive(Debug, Clone)]
pub struct Bridge {
    events_receiver: kanal::AsyncReceiver<Event>,
    commands_sender: calloop::channel::Sender<Command>,
}

impl Bridge {
    /// A function to init the wayland thread listener and returns a
    /// [`Bridge`] to send commands and receive events from Wayland.
    ///
    /// This method returns a [`Bridge`] and a [`Runner`], both assumes
    /// distincts roles. The `bridge` will be used to send and receive messages
    /// from Wayland Thread, and the `runner` is a guard to handle the Wayland
    /// thread.
    ///
    /// The `flags` are the protocols that will be activated when the [`Bridge`]
    /// starts. This flags can be used to minimize the unnecessary
    /// [`Event`] in the channel.
    ///
    /// Some wayland protocols need other protocols to use, then is more easely
    /// and safe to use the [`BridgeBuilder`] to create a new [`Bridge`]
    /// instance.
    ///
    /// # Cautions
    ///
    /// Avoid use this method more than 1 time, because The `new()` alloc some
    /// resources and spawn a new Wayland thread. Use the `clone()` to create a
    /// new bridge with the same resources.
    pub fn new(flags: Protocols) -> io::Result<(Self, Runner)> {
        info!("Starting the Wayland Thread.");

        let (events_sender, events_receiver) = bounded_async(32);
        let (commands_sender, commands_receiver) = calloop_channel();

        // init the wayland loop
        let wayland_handler = WaylandState::new(events_sender, commands_receiver, flags)?;

        // a wrapper to thread handle
        let runner = Runner::new(wayland_handler, commands_sender.clone());

        // wrap the channels
        let bridge = Self {
            events_receiver,
            commands_sender,
        };

        Ok((bridge, runner))
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

/// A Bridge Builder to only active the necessary wayland protocols.
pub struct BridgeBuilder {
    protocols: Protocols,
}

impl BridgeBuilder {
    /// Create a new builder instance.
    pub fn new() -> Self {
        Self {
            protocols: Protocols::default(),
        }
    }

    /// Active the toplevel module.
    pub fn enable_toplevel(mut self) -> Self {
        self.protocols.toplevel = true;

        // the toplevel needs the WlSeat protocols to move the focus
        self.protocols.seat = true;

        self
    }

    /// Build the [`Bridge`] and the [`Runner`].
    pub fn build(self) -> io::Result<(Bridge, Runner)> {
        Bridge::new(self.protocols)
    }
}
