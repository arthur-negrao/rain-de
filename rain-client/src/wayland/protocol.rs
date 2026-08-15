use kanal::AsyncSender;

use super::Event;

/// Defines the communincation between the wayland and protocols
pub trait ProtocolModule {
    type Command;
    type Event;
    type Proxy;

    /// Init the module resources.
    fn init(events_sender: AsyncSender<Event>) -> Self;

    /// Handle the command provided by the [`wayland_client::Dispatch`].
    fn handle_command(&mut self, cmd: Self::Command);

    /// Handle the event provided by the [`wayland_client::Dispatch`].
    fn handle_event(&mut self, proxy: &Self::Proxy, event: Self::Event);
}

/// Safe handler when the [`crate::wayland::protocol::ProtocolModule`] is a
/// `Option<T>`.
pub(crate) trait OptionalProtocolHandler<T>
where
    T: ProtocolModule,
{
    /// Handle the command if is `Some(T)`.
    fn handle_command(&mut self, cmd: <T as ProtocolModule>::Command);
}

impl<T: ProtocolModule> OptionalProtocolHandler<T> for Option<T> {
    fn handle_command(&mut self, cmd: <T as ProtocolModule>::Command) {
        if let Some(module) = self.as_mut() {
            module.handle_command(cmd);
        }
    }
}
