use super::modules::toplevel::event::ToplevelEvent;

#[derive(Debug)]
pub enum Event {
    Toplevel(ToplevelEvent),
}
