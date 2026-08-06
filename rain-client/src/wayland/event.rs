use super::protocols::toplevel::event::ToplevelEvent;

#[derive(Debug)]
pub enum Event {
    Toplevel(ToplevelEvent),
}
