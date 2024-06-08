use chrono::{DateTime, Local};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ShiftDb, TaskEvent, TaskSession, TaskState};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Could not decide which task stop from {0:?}")]
    MultipleSessions(Vec<TaskSession>),
    #[error("Expected to update one task but updated {count} rows for {task}")]
    UpdateError { count: usize, task: TaskEvent },
    #[error("Could not find any tasks to stop")]
    NoTasks,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct StopOpts {
    pub uid: Option<String>,
    pub all: bool,
    pub stop_time: Option<DateTime<Local>>,
}

/// Update task with stop time
pub fn stop(s: &ShiftDb, args: &StopOpts) -> Result<(), Error> {
    let ongoing = s.ongoing_sessions();
    // TODO handle paused sessions

    match &args.uid {
        Some(name) => {
            let ongoing_with_uid = ongoing
                .into_iter()
                .filter(|s| &s.name == name || s.id.to_string().ends_with(name))
                .collect::<Vec<TaskSession>>();
            match ongoing_with_uid.len() {
                0 => {
                    return Err(Error::NoTasks);
                }
                1 => {
                    let session = ongoing_with_uid
                        .first()
                        .expect("Should be exactly one session in the list");
                    let stop = TaskEvent::new(
                        session.name.to_string(),
                        Some(session.id),
                        args.stop_time,
                        TaskState::Stopped,
                    );
                    return match s
                        .conn
                        .execute(
                            "INSERT INTO task_events VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![stop.id, stop.name, stop.session, stop.state, stop.time],
                        )
                        .expect("SQL statement is vaild")
                    {
                        1 => Ok(()),
                        c => Err(Error::UpdateError {
                            count: c,
                            task: stop.clone(),
                        }),
                    };
                }
                2.. => {
                    return Err(Error::MultipleSessions(ongoing_with_uid));
                }
            }
        }
        None if ongoing.len() == 1 || args.all && !ongoing.is_empty() => {
            let time = args.stop_time.map_or(Local::now(), |a| a);
            for session in ongoing {
                let event = TaskEvent::new(
                    session.name.to_string(),
                    Some(session.id),
                    Some(time),
                    TaskState::Stopped,
                );
                s.conn
                    .execute(
                        "INSERT INTO task_events VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![event.id, event.name, event.session, event.state, event.time],
                    )
                    .expect("SQL statement is vaild");
            }
        }
        None => match ongoing.len() {
            0 => {
                return Err(Error::NoTasks);
            }
            _ => {
                return Err(Error::MultipleSessions(ongoing));
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use chrono::Local;

    use crate::commands::sessions::sessions;
    use crate::commands::stop::StopOpts;
    use crate::TaskState;
    use crate::{commands::test::start_with_name, Config, ShiftDb};

    use super::Error;

    use super::stop;

    #[test]
    fn stop_task() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");

        stop(&s, &StopOpts::default()).expect("Should stop without error");
        let config = Config {
            count: 10,
            ..Default::default()
        };
        let tasks = sessions(&s, &config).expect("Should get task1");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        assert!(
            tasks[0].events.first().unwrap().state == TaskState::Stopped,
            "the task stop field was not set"
        )
    }

    #[test]
    fn stop_error_multiple_tasks() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");

        let a = stop(&s, &StopOpts::default()).expect_err("Can't stop two tasks");
        match a {
            Error::MultipleSessions(t) => {
                assert_eq!(t.len(), 2, "Should get both task1 and task2");
                assert_eq!(
                    t.iter().map(|t| &t.name).collect::<Vec<&String>>(),
                    vec!["task1", "task2"]
                )
            }
            _ => panic!("error {}", a),
        }
    }

    #[test]
    fn stop_all() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");

        let config = StopOpts {
            all: true,
            ..Default::default()
        };
        stop(&s, &config).expect("Can stop all");
        let config = Config {
            all: true,
            ..Default::default()
        };
        let tasks = sessions(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 2, "Didn't get expected amount of tasks");
        for t in tasks {
            assert_eq!(
                t.events
                    .iter()
                    .filter(|e| e.state == TaskState::Stopped)
                    .count(),
                1,
                "the task stop field was not set: {t:?}"
            )
        }
    }

    #[test]
    fn stop_with_name_and_time() {
        let s = ShiftDb::new("");
        let time = Local::now();

        start_with_name(&s, "task1");

        let config = StopOpts {
            uid: Some("task1".to_string()),
            stop_time: Some(time),
            ..Default::default()
        };

        stop(&s, &config).expect("Can stop with name");
        let config = Config {
            all: true,
            ..Default::default()
        };
        let tasks = sessions(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        let stop_event = tasks.first().unwrap().events.last().unwrap();
        assert!(
            stop_event.state == TaskState::Stopped,
            "the task stop field was not set: {:?}",
            tasks.first()
        );
        assert!(
            stop_event.time == time,
            "the stop time was not the same: {:?}",
            tasks.first()
        );
    }
}
