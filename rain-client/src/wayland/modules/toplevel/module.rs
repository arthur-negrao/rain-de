use std::collections::HashMap;

use kanal::AsyncSender;
use tracing::{debug, trace};
use wayland_client::{Dispatch, event_created_child};
use wayland_client::{Proxy, protocol::wl_seat::WlSeat};

use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    Event as WindowEvent, State, ZwlrForeignToplevelHandleV1,
};

use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_manager_v1 as toplevel_manager,
    zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
};

use crate::wayland::dispatcher::WaylandState;
use crate::wayland::event::Event;
use crate::wayland::modules::toplevel::WindowProtocolWrapper;
use crate::wayland::protocol::ProtocolModule;

use super::command::ToplevelCommand;
use super::event::{EventType, ToplevelEvent, ToplevelPendingWindow};
use super::window::{WindowData, WindowHeader, WindowState};

/// A module to connect with the Toplevel wayland protocol.
pub struct ToplevelModule {
    /// channel to send events.
    events_sender: AsyncSender<Event>,

    /// hashmap with the window propeties and proxy.
    pub(crate) windows: HashMap<u32, WindowProtocolWrapper>,

    /// the [`wayland_client::protocol::wl_seat::WlSeat`] to control the window
    /// focus.
    seat: Option<WlSeat>,
}

impl ToplevelModule {
    /// Set the [`wayland_client::protocol::wl_seat::WlSeat`] to crontrol the
    /// focus.
    pub(crate) fn set_seat(&mut self, seat: WlSeat) {
        self.seat = Some(seat);
    }

    /// handle a window state change event
    fn handle_state_event(&mut self, proxy: &ZwlrForeignToplevelHandleV1, window_states: Vec<u8>) {
        if let Some(window) = self.get_pedding_window(proxy.id().protocol_id()) {
            // reset all states
            let mut state = WindowState::default();

            window_states
                .chunks(4)
                .filter_map(|chunck| {
                    let Ok(bytes) = chunck.try_into() else {
                        return None;
                    };
                    let raw_states = u32::from_ne_bytes(bytes);
                    State::try_from(raw_states).ok()
                })
                .for_each(|current_state| match current_state {
                    State::Fullscreen => state.is_fullscreen = true,
                    State::Maximized => state.is_maximized = true,
                    State::Minimized => state.is_minimized = true,
                    State::Activated => state.is_focused = true,
                    _ => {} // unknow state
                });

            window.state = state;
        }
    }

    /// Handle a [`WindowEvent::Done`] to send the message by the
    /// `event_channel`.
    fn handle_done_event(&mut self, proxy: &ZwlrForeignToplevelHandleV1) {
        if let Some(window) = self.get_pedding_window(proxy.id().protocol_id()) {
            if let (Some(app_id), Some(app_title)) =
                (window.app_id.clone(), window.app_title.clone())
            {
                let header = WindowHeader { app_id, app_title };
                let window_state = window.state.clone();
                let window_data = WindowData {
                    window_id: proxy.id().protocol_id(),
                    header: header,
                    state: window_state,
                };

                match window.event_type {
                    EventType::Opened => {
                        debug!(?window_data, "The window has been opened");

                        // change the state
                        window.event_type = EventType::StateChanged;
                        // send a message to receiver
                        let _ = self
                            .events_sender
                            .as_sync()
                            .send(ToplevelEvent::Opened(window_data).into());
                    }
                    EventType::Closed => {
                        debug!(?window_data, "The window has been closed");
                        // remove the closed window
                        self.windows.remove(&proxy.id().protocol_id());
                        let _ = self
                            .events_sender
                            .as_sync()
                            .send(ToplevelEvent::Closed(window_data).into());
                    }
                    EventType::StateChanged => {
                        trace!(?window_data, "The window has the state changed");
                        // send a message to receiver to change states
                        let _ = self
                            .events_sender
                            .as_sync()
                            .send(ToplevelEvent::StateChanged(window_data).into());
                    }
                }
            }
        }
    }

    /// Create a new toplevel event to handle.
    pub(crate) fn insert_pending_event(&mut self, toplevel: ZwlrForeignToplevelHandleV1) {
        self.windows.insert(
            toplevel.id().protocol_id(),
            WindowProtocolWrapper::new(toplevel),
        );
    }

    /// Get a pedding window if is inner of the module.
    #[inline]
    fn get_pedding_window(&mut self, window_id: u32) -> Option<&mut ToplevelPendingWindow> {
        let window = self.windows.get_mut(&window_id)?;
        Some(&mut window.pedding_window)
    }

    /// Get a window protocol if is inner of the module.
    #[inline]
    fn get_window_protocol(&mut self, window_id: u32) -> Option<&mut ZwlrForeignToplevelHandleV1> {
        let window = self.windows.get_mut(&window_id)?;
        Some(&mut window.protocol)
    }

    /// Apply a function if the protocol is not `None`.
    #[inline]
    fn with_protocol<F>(&mut self, window_id: u32, f: F)
    where
        F: FnOnce(&mut ZwlrForeignToplevelHandleV1),
    {
        if let Some(protocol) = self.get_window_protocol(window_id) {
            f(protocol);
        }
    }

    /// Apply a function if the window is not `None`.
    #[inline]
    fn with_pedding_window<F>(&mut self, window_id: u32, f: F)
    where
        F: FnOnce(&mut ToplevelPendingWindow),
    {
        if let Some(event) = self.get_pedding_window(window_id) {
            f(event);
        }
    }
}

impl ProtocolModule for ToplevelModule {
    type Command = ToplevelCommand;
    type Event = <ZwlrForeignToplevelHandleV1 as wayland_client::Proxy>::Event;
    type Proxy = ZwlrForeignToplevelHandleV1;

    fn init(events_sender: AsyncSender<Event>) -> Self {
        Self {
            events_sender,
            windows: HashMap::default(),
            seat: None,
        }
    }

    fn handle_event(&mut self, proxy: &Self::Proxy, event: Self::Event) {
        match event {
            WindowEvent::Title { title } => {
                self.with_pedding_window(proxy.id().protocol_id(), |window| {
                    window.app_title = Some(title);
                });
            }
            WindowEvent::AppId { app_id } => {
                self.with_pedding_window(proxy.id().protocol_id(), |window| {
                    window.app_id = Some(app_id);
                });
            }
            WindowEvent::State {
                state: window_states,
            } => {
                self.handle_state_event(proxy, window_states);
            }
            WindowEvent::Closed => {
                self.with_pedding_window(proxy.id().protocol_id(), |window| {
                    // cleanup the window
                    window.event_type = EventType::Closed;
                });
            }
            WindowEvent::Done => {
                self.handle_done_event(proxy);
            }
            _ => {}
        }
    }

    fn handle_command(&mut self, cmd: Self::Command) {
        trace!("Command to wayland receive");
        match cmd {
            ToplevelCommand::Close(window_id) => {
                self.with_protocol(window_id, |protocol| protocol.close());
            }
            ToplevelCommand::Fullscreen((window_id, set)) => {
                self.with_protocol(window_id, |protocol| {
                    if set {
                        protocol.set_fullscreen(None);
                    } else {
                        protocol.unset_fullscreen();
                    }
                });
            }
            ToplevelCommand::Maximize((window_id, set)) => {
                self.with_protocol(window_id, |protocol| {
                    if set {
                        protocol.set_maximized();
                    } else {
                        protocol.unset_maximized();
                    }
                });
            }
            ToplevelCommand::Minimize((window_id, set)) => {
                self.with_protocol(window_id, |protocol| {
                    if set {
                        protocol.set_minimized();
                    } else {
                        protocol.unset_minimized();
                    }
                });
            }
            ToplevelCommand::Focus(window_id) => {
                if let Some(seat) = &self.seat.clone() {
                    self.with_protocol(window_id, |protocol| {
                        protocol.activate(&seat);
                    });
                }
            }
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, (), WaylandState> for ToplevelModule {
    fn event(
        state: &mut WaylandState,
        _proxy: &ZwlrForeignToplevelManagerV1,
        event: <ZwlrForeignToplevelManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        match event {
            toplevel_manager::Event::Toplevel { toplevel: event } => {
                let module: &mut Self = state.as_mut();
                module.insert_pending_event(event);
            }
            _ => {}
        };
        debug!("Toplevel Event received");
    }

    event_created_child!(
        WaylandState,
        ZwlrForeignToplevelManagerV1,
        [
            toplevel_manager::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ())
        ]
    );
}

impl Dispatch<ZwlrForeignToplevelHandleV1, (), WaylandState> for ToplevelModule {
    fn event(
        state: &mut WaylandState,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        let module: &mut Self = state.as_mut();
        module.handle_event(proxy, event);
    }
}
