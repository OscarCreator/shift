use crate::{Config, ShiftDb, TaskSession};

// Get curret ongoing task(s)
pub fn status(s: &ShiftDb, _args: &Config) -> Vec<TaskSession> {
    s.ongoing_sessions()
}
