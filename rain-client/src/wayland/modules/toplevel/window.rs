use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1;

use super::ToplevelPendingWindow;

#[derive(Debug)]
pub struct WindowProtocolWrapper {
    pub protocol: ZwlrForeignToplevelHandleV1,
    pub pedding_window: ToplevelPendingWindow,
}

impl WindowProtocolWrapper {
    pub(crate) fn new(proxy: ZwlrForeignToplevelHandleV1) -> Self {
        Self {
            protocol: proxy,
            pedding_window: ToplevelPendingWindow::default(),
        }
    }
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
