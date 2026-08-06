use std::thread;

use calloop::{EventLoop, channel::Channel};
use kanal::AsyncSender;
use tracing::{debug, error};

use wayland_client::{
    Connection, Dispatch, Proxy, event_created_child,
    protocol::{wl_registry, wl_seat::WlSeat},
};

use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    zwlr_foreign_toplevel_manager_v1 as toplevel_manager,
};

use super::command::Command;
use super::event::Event;
use super::protocols::toplevel::ToplevelState;
use super::wayland_source::WaylandSource;

pub struct WaylandState {
    pub(super) toplevel_state: ToplevelState,
}

impl WaylandState {
    pub fn new(events_sender: AsyncSender<Event>, commands_receiver: Channel<Command>) {
        thread::Builder::new()
            .name("wayland-events".to_string())
            .spawn(move || {
                let conn =
                    Connection::connect_to_env().expect("Fail to connect with wayland server");

                let display = conn.display();

                let toplevel_state = ToplevelState::new(events_sender);

                let mut state = Self {
                    toplevel_state: toplevel_state,
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
                state.toplevel_state.set_seat(seat);
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
                state.toplevel_state.insert_pending_event(toplevel);
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
        state.toplevel_state.handle_event(proxy, event);
    }
}

fn handle_wayland_command(shared_state: &mut WaylandState, cmd: Command) {
    match cmd {
        Command::Toplevel(command) => {
            shared_state.toplevel_state.handle_command(command);
        }
    }
}
