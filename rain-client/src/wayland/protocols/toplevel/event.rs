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

#[derive(Debug, Default, Clone)]
pub struct WindowHeader {
    pub app_id: String,
    pub app_title: String,
}

#[derive(Debug, Default, Clone)]
pub struct WindowState {
    pub is_focused: bool,
    pub is_maximized: bool,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
}

#[derive(Debug, Default)]
pub struct WindowData {
    pub window_id: u32,
    pub header: WindowHeader,
    pub state: WindowState,
}

#[derive(Debug)]
pub enum ToplevelEvent {
    Opened(WindowData),
    Closed(WindowData),
    StateChanged(WindowData),
}
