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
