pub mod pause;
pub mod start;
pub mod status;
pub mod stop;
pub mod tasks;

// TODO is this correct way to share test functions?
#[cfg(test)]
mod test {
    use crate::{Config, ShiftDb, Task};

    use super::start::start;

    pub fn start_with_name(shift: &ShiftDb, s: &str) -> Task {
        let config = Config {
            uid: Some(s.to_string()),
            ..Default::default()
        };
        start(&shift, &config).unwrap()
    }
}
