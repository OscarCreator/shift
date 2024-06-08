use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ShiftDb;

#[derive(Error, Debug)]
pub enum Error {
    #[error("")]
    A,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Opts {}

/// return the row count removed
pub fn undo(s: &ShiftDb, opts: &Opts) -> Result<usize, Error> {
    Ok(s.conn
        .execute(
            "DELETE FROM task_events
            WHERE time = (
                SELECT MAX(time) FROM task_events
            )",
            [],
        )
        .expect("SQL statement is valid"))
}

#[cfg(test)]
mod test {
    use chrono::Local;

    use crate::{
        commands::{
            pause::{pause, resume},
            sessions::sessions,
            start::{start, StartOpts},
            stop::{self, stop, StopOpts},
            test::start_with_name,
            undo,
        },
        Config, ShiftDb,
    };

    use super::{undo, Opts};

    #[test]
    fn undo_start() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task2");

        let config = Config {
            all: true,
            ..Default::default()
        };
        let sessions_before = sessions(&s, &config).unwrap();

        start_with_name(&s, "task1");

        undo(&s, &Opts::default()).unwrap();

        let sessions_after = sessions(&s, &config).unwrap();

        assert_eq!(sessions_before, sessions_after);
    }

    #[test]
    fn undo_stop() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        let opts = StopOpts {
            uid: Some("task1".to_owned()),
            ..Default::default()
        };
        stop(&s, &opts).unwrap();

        undo(&s, &undo::Opts::default()).unwrap();

        stop(&s, &opts).expect("Can stop again after undoing the last stop");
        assert_eq!(
            stop(&s, &opts).expect_err("Can't stop twice"),
            stop::Error::NoTasks
        );
    }

    #[test]
    fn undo_stop_all() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");
        start_with_name(&s, "task3");
        start_with_name(&s, "task4");
        let opts = StopOpts {
            all: true,
            ..Default::default()
        };
        stop(&s, &opts).unwrap();

        assert_eq!(undo(&s, &undo::Opts::default()).unwrap(), 4);

        for i in 1..=4 {
            let opts = StopOpts {
                uid: Some(format!("task{i}").to_owned()),
                ..Default::default()
            };
            stop(&s, &opts).expect("Can stop again after undoing the last stop");
            assert_eq!(
                stop(&s, &opts).expect_err("Can't stop twice"),
                stop::Error::NoTasks
            );
        }
    }

    #[test]
    fn undo_switch() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        let time = Local::now();
        let opts = StopOpts {
            all: true,
            stop_time: Some(time),
            ..Default::default()
        };
        stop(&s, &opts).unwrap();
        let opts = StartOpts {
            uid: Some("task2".to_string()),
            start_time: Some(time),
            ..Default::default()
        };
        start(&s, &opts).unwrap();

        assert_eq!(undo(&s, &undo::Opts::default()).unwrap(), 2);
    }

    #[test]
    fn undo_pause() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        let opts = Config {
            ..Default::default()
        };
        pause(&s, &opts).unwrap();

        assert_eq!(undo(&s, &undo::Opts::default()).unwrap(), 1);
        pause(&s, &opts).expect("Can pause after undo");
    }

    #[test]
    fn undo_pause_all() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");
        start_with_name(&s, "task3");
        let opts = Config {
            all: true,
            ..Default::default()
        };
        pause(&s, &opts).unwrap();

        assert_eq!(undo(&s, &undo::Opts::default()).unwrap(), 3);
        pause(&s, &opts).expect("Can pause after undo");
    }

    #[test]
    fn undo_resume() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        let opts = Config {
            all: true,
            ..Default::default()
        };
        pause(&s, &opts).unwrap();
        resume(&s, &opts).unwrap();

        assert_eq!(undo(&s, &undo::Opts::default()).unwrap(), 1);
        resume(&s, &opts).expect("Can pause after undo");
    }

    #[test]
    fn undo_resume_all() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");
        start_with_name(&s, "task3");
        let opts = Config {
            all: true,
            ..Default::default()
        };
        pause(&s, &opts).unwrap();
        resume(&s, &opts).unwrap();

        assert_eq!(undo(&s, &undo::Opts::default()).unwrap(), 3);
        resume(&s, &opts).expect("Can pause after undo");
    }
}
