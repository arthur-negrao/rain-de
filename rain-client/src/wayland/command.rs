use super::protocols::toplevel::command::ToplevelCommand;

#[derive(Debug)]
pub enum Command {
    Toplevel(ToplevelCommand),
}
