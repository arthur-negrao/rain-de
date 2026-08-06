use std::{collections::HashMap, thread};

use calloop::{EventLoop, channel::Channel};
use kanal::AsyncSender;
use tracing::{debug, error};

use wayland_client::{
    Connection, Dispatch, Proxy, event_created_child,
    protocol::{wl_registry, wl_seat::WlSeat},
};

use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{Event as ToplevelEvent, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1 as toplevel_manager,
};

use super::command::Command;
use super::event::{Event, EventType, PendingEvent};
use super::handlers::handle_wayland_command;
use super::handlers::{handle_toplevel_done_event, handle_toplevel_state_event};
use super::wayland_source::WaylandSource;

#[derive(Debug)]
pub struct WaylandState {
    pub(super) pending_events: HashMap<ZwlrForeignToplevelHandleV1, PendingEvent>,
    pub(super) events_sender: AsyncSender<Event>,
    pub(super) seat: Option<WlSeat>,
}

impl WaylandState {
    pub fn new(events_sender: AsyncSender<Event>, commands_receiver: Channel<Command>) {
        thread::Builder::new()
            .name("wayland-events".to_string())
            .spawn(move || {
                let conn =
                    Connection::connect_to_env().expect("Fail to connect with wayland server");

                let display = conn.display();

                let mut state = Self {
                    pending_events: HashMap::default(),
                    events_sender: events_sender,
                    seat: None,
                };

                let mut event_queue = conn.new_event_queue::<WaylandState>();
                let qh = event_queue.handle();

                let _registry = display.get_registry(&qh, ());

                // receive the old global events by bind
                event_queue
                    .roundtrip(&mut state)
                    .expect("Fail to register the global events");

                // receive the old toplevel events
                event_queue
                    .roundtrip(&mut state)
                    .expect("Fail to read the current windows");

                // add a event loop to sleep the thread until a source emits a event
                let mut event_loop =
                    EventLoop::<Self>::try_new().expect("Fail to create the event loop");
                let event_handle = event_loop.handle();

                WaylandSource::new(conn.clone(), event_queue)
                    .insert(event_handle.clone())
                    .expect("Fail to insert the EventQueue");

                event_handle
                    .insert_source(commands_receiver, |event, _metadata, shared_state| {
                        if let calloop::channel::Event::Msg(cmd) = event {
                            handle_wayland_command(shared_state, cmd);
                        }
                    })
                    .expect("Fail to connect the channel on EventLoop");

                loop {
                    if let Err(e) = event_loop.dispatch(None, &mut state) {
                        error!("Error dispatching wayland events: {}", e);
                        break;
                    }

                    while let Err(e) = conn.flush() {
                        error!("Error flushing wayland events: {}", e);
                        break;
                    }
                }
            })
            .expect("Error to init the wayland thread");
    }

    pub fn find_protocol_by_id(&mut self, window_id: u32) -> Option<ZwlrForeignToplevelHandleV1> {
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

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == toplevel_manager::ZwlrForeignToplevelManagerV1::interface().name {
                proxy.bind::<toplevel_manager::ZwlrForeignToplevelManagerV1, _, _>(
                    name,
                    version,
                    &qhandle,
                    (),
                );
            } else if interface == WlSeat::interface().name {
                let seat = proxy.bind::<WlSeat, _, _>(name, version, &qhandle, ());
                state.seat = Some(seat);
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        // just ignore to get the seat
    }
}

impl Dispatch<toplevel_manager::ZwlrForeignToplevelManagerV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &toplevel_manager::ZwlrForeignToplevelManagerV1,
        event: <toplevel_manager::ZwlrForeignToplevelManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            toplevel_manager::Event::Toplevel { toplevel } => {
                state
                    .pending_events
                    .insert(toplevel, PendingEvent::default());
            }
            _ => {}
        };
        debug!("Toplevel Event received");
    }

    event_created_child!(
        WaylandState,
        toplevel_manager::ZwlrForeignToplevelManagerV1,
        [
            toplevel_manager::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ())
        ]
    );
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            ToplevelEvent::Title { title } => {
                if let Some(window) = state.pending_events.get_mut(proxy) {
                    window.app_title = Some(title);
                }
            }
            ToplevelEvent::AppId { app_id } => {
                if let Some(window) = state.pending_events.get_mut(proxy) {
                    window.app_id = Some(app_id);
                }
            }
            ToplevelEvent::State {
                state: window_states,
            } => {
                handle_toplevel_state_event(state, proxy, window_states);
            }
            ToplevelEvent::Closed => {
                if let Some(window) = state.pending_events.get_mut(proxy) {
                    // cleanup the window
                    window.event_type = EventType::Closed;
                }
            }
            ToplevelEvent::Done => {
                handle_toplevel_done_event(state, proxy);
            }
            _ => {}
        }
    }
}
