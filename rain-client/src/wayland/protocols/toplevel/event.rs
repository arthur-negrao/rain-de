use super::window::*;
use crate::wayland::Event;

#[derive(Debug, Default)]
pub enum EventType {
    #[default]
    Opened,
    Closed,
    StateChanged,
}

#[derive(Debug, Default)]
pub struct ToplevelPendingEvent {
    pub event_type: EventType,
    pub app_id: Option<String>,
    pub app_title: Option<String>,
    pub state: WindowState,
}

#[derive(Debug)]
pub enum ToplevelEvent {
    Opened(WindowData),
    Closed(WindowData),
    StateChanged(WindowData),
}

impl From<ToplevelEvent> for Event {
    fn from(value: ToplevelEvent) -> Self {
        Event::Toplevel(value)
    }
}

impl TryFrom<Event> for ToplevelEvent {
    type Error = Event;

    fn try_from(value: Event) -> Result<Self, Self::Error> {
        match value {
            Event::Toplevel(toplevel) => Ok(toplevel),
        }
    }
}
