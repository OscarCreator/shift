pub mod pause;
pub mod sessions;
pub mod start;
pub mod status;
pub mod stop;
pub mod undo;

// TODO remove this shared test function
#[cfg(test)]
mod test {
    use crate::{ShiftDb, TaskEvent};

    use super::start::start;

    pub fn start_with_name(shift: &ShiftDb, s: &str) -> TaskEvent {
        let config = super::start::StartOpts {
            uid: Some(s.to_string()),
            ..Default::default()
        };
        start(&shift, &config).unwrap()
    }
}
