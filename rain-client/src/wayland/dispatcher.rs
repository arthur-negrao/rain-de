use std::io;
use std::thread::{self, JoinHandle};

use calloop::{EventLoop, channel::Channel};
use kanal::AsyncSender;
use tracing::{error, info};

use wayland_client::{
    Connection, Dispatch, Proxy, delegate_dispatch,
    protocol::{wl_registry, wl_seat::WlSeat},
};

use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
};

use crate::wayland::protocol::{OptionalProtocolHandler, ProtocolModule};

use super::command::Command;
use super::event::Event;
use super::modules::toplevel::ToplevelModule;
use super::wayland_source::WaylandSource;

#[derive(Debug, Default)]
pub struct Protocols {
    pub toplevel: bool,
    pub seat: bool,
}

/// The general state to connect with wayland server.
pub struct WaylandState {
    /// channel to send [`crate::wayland::Event`].
    pub(super) events_sender: AsyncSender<Event>,

    /// active protocols flags
    protocols: Protocols,

    /// run flag
    still_running: bool,

    // --- resources --- //
    /// wayland resource to access general inputs
    pub(super) seat: Option<WlSeat>,

    // --- protocol modules --- //
    /// the optional toplevel protocol module
    pub(super) toplevel: Option<ToplevelModule>,
}

impl WaylandState {
    pub fn new(
        events_sender: AsyncSender<Event>,
        commands_receiver: Channel<Command>,
        protocols: Protocols,
    ) -> io::Result<JoinHandle<()>> {
        let conn = Connection::connect_to_env()
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;

        let display = conn.display();

        let mut state = Self {
            events_sender,
            toplevel: None,
            seat: None,
            protocols,
            still_running: true,
        };

        let mut event_queue = conn.new_event_queue::<WaylandState>();
        let qh = event_queue.handle();

        let _registry = display.get_registry(&qh, ());

        // receive the old global events by bind
        event_queue
            .roundtrip(&mut state)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // receive the old toplevel events
        event_queue
            .roundtrip(&mut state)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let (init_sender, init_receiver) = std::sync::mpsc::sync_channel(0);

        let handler = thread::Builder::new()
            .name("wayland-events".to_string())
            .spawn(move || {
                // add a event loop to sleep the thread until a source emits a event
                let mut event_loop = match EventLoop::<Self>::try_new() {
                    Ok(el) => el,
                    Err(e) => {
                        let _ = init_sender.send(Err(io::Error::new(io::ErrorKind::Other, e)));
                        return;
                    }
                };

                let event_handle = event_loop.handle();

                if let Err(e) =
                    WaylandSource::new(conn.clone(), event_queue).insert(event_handle.clone())
                {
                    let _ = init_sender.send(Err(io::Error::new(io::ErrorKind::Other, e)));
                    return;
                };

                if let Err(e) = event_handle.insert_source(
                    commands_receiver,
                    |event, _metadata, shared_state| {
                        if let calloop::channel::Event::Msg(cmd) = event {
                            handle_wayland_command(shared_state, cmd);
                        }
                    },
                ) {
                    let _ = init_sender.send(Err(io::Error::new(io::ErrorKind::Other, e.error)));
                    return;
                };

                let _ = init_sender.send(Ok(()));

                loop {
                    if !state.still_running {
                        info!("Wayland thread stoping by command.");
                        break;
                    }

                    if let Err(e) = event_loop.dispatch(None, &mut state) {
                        error!("Error dispatching wayland events: {}", e);
                        break;
                    }

                    while let Err(e) = conn.flush() {
                        error!("Error flushing wayland events: {}", e);
                        break;
                    }
                }
            })?;

        match init_receiver.recv() {
            Err(e) => Err(io::Error::new(io::ErrorKind::BrokenPipe, e)),
            Ok(msg) => match msg {
                Ok(_) => Ok(handler),
                Err(e) => Err(e),
            },
        }
    }
}

/// Handle a received command to Wayland thread.
fn handle_wayland_command(shared_state: &mut WaylandState, cmd: Command) {
    match cmd {
        Command::Toplevel(command) => {
            shared_state.toplevel.handle_command(command);
        }
        Command::Quit => {
            shared_state.still_running = false;
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
            if interface == ZwlrForeignToplevelManagerV1::interface().name {
                if state.protocols.toplevel {
                    let mut toplevel = ToplevelModule::init(state.events_sender.clone());

                    // try pass the seat
                    if let Some(seat) = state.seat.as_ref() {
                        toplevel.set_seat(seat.clone());
                    }

                    state.toplevel = Some(toplevel);

                    // bind the protocol
                    proxy.bind::<ZwlrForeignToplevelManagerV1, _, _>(name, version, &qhandle, ());
                }
            } else if interface == WlSeat::interface().name {
                if state.protocols.seat {
                    let seat = proxy.bind::<WlSeat, _, _>(name, version, &qhandle, ());

                    // if the toplevel exists
                    if let Some(toplevel) = state.toplevel.as_mut() {
                        toplevel.set_seat(seat.clone());
                    }

                    state.seat = Some(seat)
                }
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

delegate_dispatch!(WaylandState: [ZwlrForeignToplevelManagerV1: ()] => ToplevelModule);
delegate_dispatch!(WaylandState: [ZwlrForeignToplevelHandleV1: ()] => ToplevelModule);

impl AsMut<ToplevelModule> for WaylandState {
    fn as_mut(&mut self) -> &mut ToplevelModule {
        self.toplevel
            .as_mut()
            .expect("The Toplevel protocol is unitialized.")
    }
}
