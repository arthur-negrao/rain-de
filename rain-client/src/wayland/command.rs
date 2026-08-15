use super::modules::toplevel::command::ToplevelCommand;

#[derive(Debug)]
pub enum Command {
    Toplevel(ToplevelCommand),
    /// Close the wayland thread
    Quit,
}
