use std::{error::Error, fmt::Display};

use chrono::{DateTime, Local, Utc};
use rusqlite::params;

use crate::{Config, ShiftDb, Task};

#[derive(Debug)]
pub enum StopError {
    MultipleTasks(Vec<Task>),
    UpdateError(Task),
    SqlError(String),
    NoTasks,
}

impl Error for StopError {}

impl Display for StopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("todo")
    }
}

/// Update task with stop time
pub fn stop(s: &ShiftDb, args: &Config) -> Result<(), StopError> {
    let query = "SELECT * FROM tasks WHERE stop IS NULL";
    let mut stmt = s.conn.prepare(query).expect("SQL statement is valid");
    let tasks = stmt
        .query_map([], |row| Task::try_from(row))
        .expect("No parameters should always bind correctly")
        .flatten()
        .collect::<Vec<Task>>();

    match &args.uid {
        Some(id) => {
            let tasks_name_match = s.get_tasks(id);
            match tasks_name_match.len() {
                0 => {
                    dbg!(&tasks_name_match);
                    return Err(StopError::NoTasks);
                }
                1 => {
                    if let Some(t) = tasks_name_match.first() {
                        return match s.conn.execute(
                            "UPDATE tasks SET stop = ?1 WHERE name = ?2 and stop IS NULL",
                            params![DateTime::<Utc>::from(Local::now()), t.name],
                        ) {
                            Ok(count) => {
                                if count == 1 {
                                    Ok(())
                                } else {
                                    Err(StopError::UpdateError(t.clone()))
                                }
                            }
                            Err(err) => Err(StopError::SqlError(err.to_string())),
                        };
                    }
                }
                2.. => {
                    return Err(StopError::MultipleTasks(tasks_name_match));
                }
            }

            s.conn
                .execute(
                    "UPDATE tasks SET stop = ?1 WHERE id LIKE ?2",
                    params![DateTime::<Utc>::from(Local::now()), format!("%{id}")],
                )
                .expect("SQL statement is valid");
        }
        None if tasks.len() == 1 || args.all => {
            s.conn
                .execute(
                    "UPDATE tasks SET stop = ?1 WHERE stop IS NULL",
                    params![DateTime::<Utc>::from(Local::now())],
                )
                .expect("SQL statement is vaild");
        }
        None => match tasks.len() {
            0 => {
                return Err(StopError::NoTasks);
            }
            _ => {
                return Err(StopError::MultipleTasks(tasks));
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::commands::tasks::tasks;
    use crate::{commands::test::start_with_name, Config, ShiftDb};

    use super::StopError;

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
        let tasks = tasks(&s, &config).expect("Should get task1");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        assert!(tasks[0].stop != None, "the task stop field was not set")
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
            StopError::MultipleTasks(t) => {
                assert_eq!(t.len(), 2, "Should get both task1 and task2");
                assert_eq!(
                    t.iter().map(|t| &t.name).collect::<Vec<&String>>(),
                    vec!["task1", "task2"]
                )
            }
            _ => panic!("error"),
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
        let tasks = tasks(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 2, "Didn't get expected amount of tasks");
        for t in tasks {
            assert!(t.stop != None, "the task stop field was not set: {t:?}")
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
        let tasks = tasks(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        assert!(
            tasks.first().unwrap().stop != None,
            "the task stop field was not set: {:?}",
            tasks.first()
        )
    }
}
