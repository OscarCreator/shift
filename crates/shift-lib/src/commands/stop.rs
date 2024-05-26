use rusqlite::params;
use thiserror::Error;

use crate::{Config, ShiftDb, TaskEvent, TaskSession, TaskState};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Could not decide which task stop from {0:?}")]
    MultipleSessions(Vec<TaskSession>),
    #[error("Expected to update one task but updated {count} rows for {task}")]
    UpdateError { count: usize, task: TaskEvent },
    #[error("Could not find any tasks to stop")]
    NoTasks,
}

/// Update task with stop time
pub fn stop(s: &ShiftDb, args: &Config) -> Result<(), Error> {
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
        None if ongoing.len() == 1 || args.all => {
            for session in ongoing {
                let event = TaskEvent::new(
                    session.name.to_string(),
                    Some(session.id),
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
    use crate::commands::sessions::sessions;
    use crate::TaskState;
    use crate::{commands::test::start_with_name, Config, ShiftDb};

    use super::Error;

    use super::stop;

    #[test]
    fn stop_task() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");

        let config = Config {
            count: 10,
            ..Default::default()
        };
        stop(&s, &config).expect("Should stop without error");
        let tasks = sessions(&s, &config).expect("Should get task1");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        assert!(
            tasks[0].events.last().unwrap().state == TaskState::Stopped,
            "the task stop field was not set"
        )
    }

    #[test]
    fn stop_error_multiple_tasks() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");

        let config = Config {
            count: 10,
            ..Default::default()
        };
        let a = stop(&s, &config).expect_err("Can't stop two tasks");
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

        let config = Config {
            all: true,
            ..Default::default()
        };
        stop(&s, &config).expect("Can stop all");
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
    fn stop_with_name() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");

        let config = Config {
            uid: Some("task1".to_string()),
            ..Default::default()
        };

        stop(&s, &config).expect("Can stop with name");
        let config = Config {
            all: true,
            ..Default::default()
        };
        let tasks = sessions(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        assert!(
            tasks.first().unwrap().events.last().unwrap().state == TaskState::Stopped,
            "the task stop field was not set: {:?}",
            tasks.first()
        )
    }
}
