use std::collections::HashMap;

use kanal::AsyncSender;

use tracing::{debug, trace};
use wayland_client::{Proxy, protocol::wl_seat::WlSeat};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    Event as WindowEvent, State, ZwlrForeignToplevelHandleV1,
};

use super::event::{EventType, ToplevelPendingEvent, WindowData, WindowHeader, WindowState};
use crate::wayland::{
    event::Event,
    protocols::toplevel::{command::ToplevelCommand, event::ToplevelEvent},
};

pub struct ToplevelState {
    events_sender: AsyncSender<Event>,
    pub(crate) pending_events: HashMap<ZwlrForeignToplevelHandleV1, ToplevelPendingEvent>,
    seat: Option<WlSeat>,
}

impl ToplevelState {
    pub fn new(events_sender: AsyncSender<Event>) -> Self {
        Self {
            events_sender,
            pending_events: HashMap::default(),
            seat: None,
        }
    }

    pub(crate) fn set_seat(&mut self, seat: WlSeat) {
        self.seat = Some(seat);
    }

    pub fn handle_event(
        &mut self,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as wayland_client::Proxy>::Event,
    ) {
        match event {
            WindowEvent::Title { title } => {
                if let Some(window) = self.pending_events.get_mut(proxy) {
                    window.app_title = Some(title);
                }
            }
            WindowEvent::AppId { app_id } => {
                if let Some(window) = self.pending_events.get_mut(proxy) {
                    window.app_id = Some(app_id);
                }
            }
            WindowEvent::State {
                state: window_states,
            } => {
                self.handle_state_event(proxy, window_states);
            }
            WindowEvent::Closed => {
                if let Some(window) = self.pending_events.get_mut(proxy) {
                    // cleanup the window
                    window.event_type = EventType::Closed;
                }
            }
            WindowEvent::Done => {
                self.handle_done_event(proxy);
            }
            _ => {}
        }
    }

    pub fn handle_command(&mut self, cmd: ToplevelCommand) {
        trace!("Command to wayland receive");
        match cmd {
            ToplevelCommand::Close(window_id) => {
                if let Some(protocol) = self.find_protocol_by_id(window_id) {
                    protocol.close();
                }
            }
            ToplevelCommand::Fullscreen((window_id, set)) => {
                if let Some(protocol) = self.find_protocol_by_id(window_id) {
                    if set {
                        protocol.set_fullscreen(None);
                    } else {
                        protocol.unset_fullscreen();
                    }
                }
            }
            ToplevelCommand::Maximize((window_id, set)) => {
                if let Some(protocol) = self.find_protocol_by_id(window_id) {
                    if set {
                        protocol.set_maximized();
                    } else {
                        protocol.unset_maximized();
                    }
                }
            }
            ToplevelCommand::Minimize((window_id, set)) => {
                if let Some(protocol) = self.find_protocol_by_id(window_id) {
                    if set {
                        protocol.set_minimized();
                    } else {
                        protocol.unset_minimized();
                    }
                }
            }
            ToplevelCommand::Focus(window_id) => {
                if let Some(protocol) = self.find_protocol_by_id(window_id) {
                    if let Some(seat) = &self.seat {
                        protocol.activate(&seat);
                    }
                }
            }
        }
    }

    fn handle_state_event(&mut self, proxy: &ZwlrForeignToplevelHandleV1, window_states: Vec<u8>) {
        if let Some(window) = self.pending_events.get_mut(proxy) {
            // reset all states
            window.state = WindowState::default();

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
                    State::Fullscreen => window.state.is_fullscreen = true,
                    State::Maximized => window.state.is_maximized = true,
                    State::Minimized => window.state.is_minimized = true,
                    State::Activated => window.state.is_focused = true,
                    _ => {} // unknow state
                });
        }
    }

    fn handle_done_event(&mut self, proxy: &ZwlrForeignToplevelHandleV1) {
        if let Some(window) = self.pending_events.get_mut(proxy) {
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
                            .send(Event::Toplevel(ToplevelEvent::Opened(window_data)));
                    }
                    EventType::Closed => {
                        debug!(?window_data, "The window has been closed");
                        // remove the closed window
                        self.pending_events.remove(proxy);
                        let _ = self
                            .events_sender
                            .as_sync()
                            .send(Event::Toplevel(ToplevelEvent::Closed(window_data)));
                    }
                    EventType::StateChanged => {
                        trace!(?window_data, "The window has the state changed");
                        // send a message to receiver to change states
                        let _ = self
                            .events_sender
                            .as_sync()
                            .send(Event::Toplevel(ToplevelEvent::StateChanged(window_data)));
                    }
                }
            }
        }
    }

    pub(crate) fn insert_pending_event(&mut self, toplevel: ZwlrForeignToplevelHandleV1) {
        self.pending_events
            .insert(toplevel, ToplevelPendingEvent::default());
    }

    fn find_protocol_by_id(&mut self, window_id: u32) -> Option<ZwlrForeignToplevelHandleV1> {
        let result = self
            .pending_events
            .iter()
            .find(|(protocol, _)| protocol.id().protocol_id() == window_id);

        match result {
            None => None,
            Some((protocol, _)) => Some(protocol.clone()),
        }
    }
}
