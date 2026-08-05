use calloop::channel::channel as calloop_channel;

use crate::shared::command::WaylandCommand;
use crate::shared::event::WaylandEvent;
use crate::state::{dock_app::DockApp, manager::DockState};
use crate::wayland::dispatcher::WaylandState;

/// A abstraction to create connection between wayland and gtk
pub struct Bridge {}

impl Bridge {
    pub fn init(state: DockState) {
        let (events_sender, events_receiver) = async_channel::unbounded();
        let (commands_sender, commands_receiver) = calloop_channel::<WaylandCommand>();

        state.set_commands_sender(commands_sender);

        WaylandState::new(events_sender, commands_receiver);

        let context = gtk::glib::MainContext::default();
        context.spawn_local(async move {
            while let Ok(event) = events_receiver.recv().await {
                handle_wayland_event(&state, event);
            }
        });
    }
}

fn handle_wayland_event(state: &DockState, event: WaylandEvent) {
    match event {
        WaylandEvent::Opened(data) => {
            let app = DockApp::new(data.window_id, &data.header.app_title, data.state);
            state.add_app(&data.header.app_id, app);
        }
        WaylandEvent::Closed(data) => {
            state.remove_app(&data.header.app_id, data.window_id);
        }
        WaylandEvent::StateChanged(data) => {
            state.process_state_changed(data);
        }
    }
}
