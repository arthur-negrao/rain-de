use tracing::{debug, trace};

use wayland_client::Proxy;
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    State, ZwlrForeignToplevelHandleV1,
};

use super::command::Command;
use super::dispatcher::WaylandState;
use super::event::{Event, EventType, WindowData, WindowHeader, WindowState};

// TOPLEVEL EVENTS //

pub(super) fn handle_toplevel_state_event(
    state: &mut WaylandState,
    proxy: &ZwlrForeignToplevelHandleV1,
    window_states: Vec<u8>,
) {
    if let Some(window) = state.pending_events.get_mut(proxy) {
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

pub(super) fn handle_toplevel_done_event(
    state: &mut WaylandState,
    proxy: &ZwlrForeignToplevelHandleV1,
) {
    if let Some(window) = state.pending_events.get_mut(proxy) {
        if let (Some(app_id), Some(app_title)) = (window.app_id.clone(), window.app_title.clone()) {
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
                    let _ = state
                        .events_sender
                        .as_sync()
                        .send(Event::Opened(window_data));
                }
                EventType::Closed => {
                    debug!(?window_data, "The window has been closed");
                    // remove the closed window
                    state.pending_events.remove(proxy);
                    let _ = state
                        .events_sender
                        .as_sync()
                        .send(Event::Closed(window_data));
                }
                EventType::StateChanged => {
                    trace!(?window_data, "The window has the state changed");
                    // send a message to receiver to change states
                    let _ = state
                        .events_sender
                        .as_sync()
                        .send(Event::StateChanged(window_data));
                }
            }
        }
    }
}

// WAYLAND COMMANDS //

pub(super) fn handle_wayland_command(state: &mut WaylandState, cmd: Command) {
    trace!("Command to wayland receive");
    match cmd {
        Command::Close(window_id) => {
            if let Some(protocol) = state.find_protocol_by_id(window_id) {
                protocol.close();
            }
        }
        Command::Fullscreen((window_id, set)) => {
            if let Some(protocol) = state.find_protocol_by_id(window_id) {
                if set {
                    protocol.set_fullscreen(None);
                } else {
                    protocol.unset_fullscreen();
                }
            }
        }
        Command::Maximize((window_id, set)) => {
            if let Some(protocol) = state.find_protocol_by_id(window_id) {
                if set {
                    protocol.set_maximized();
                } else {
                    protocol.unset_maximized();
                }
            }
        }
        Command::Minimize((window_id, set)) => {
            if let Some(protocol) = state.find_protocol_by_id(window_id) {
                if set {
                    protocol.set_minimized();
                } else {
                    protocol.unset_minimized();
                }
            }
        }
        Command::Focus(window_id) => {
            if let Some(protocol) = state.find_protocol_by_id(window_id) {
                if let Some(seat) = &state.seat {
                    protocol.activate(&seat);
                }
            }
        }
    }
}
