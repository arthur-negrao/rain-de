#[derive(Debug)]
pub enum ToplevelCommand {
    Close(u32),
    Maximize((u32, bool)),
    Minimize((u32, bool)),
    Fullscreen((u32, bool)),
    Focus(u32),
}
