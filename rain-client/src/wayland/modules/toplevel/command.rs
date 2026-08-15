use crate::wayland::Command;

#[derive(Debug)]
pub enum ToplevelCommand {
    Close(u32),
    Maximize((u32, bool)),
    Minimize((u32, bool)),
    Fullscreen((u32, bool)),
    Focus(u32),
}

impl From<ToplevelCommand> for Command {
    fn from(value: ToplevelCommand) -> Self {
        Command::Toplevel(value)
    }
}
