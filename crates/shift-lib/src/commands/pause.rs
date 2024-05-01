use std::{error::Error, fmt::Display};

use chrono::{DateTime, Local, Utc};
use rusqlite::params;

use crate::{Config, ShiftDb, Task, TaskPause};

#[derive(Debug, PartialEq, Eq)]
pub enum PauseResumeError {
    MultipleTasks(Vec<Task>),
    MultiplePauses(Vec<TaskPause>),
    UpdateError(Task),
    SqlError(String),
    NoTasks,
    NoPauses,
}

impl Error for PauseResumeError {}

// TODO split pause/resume so we can have better error messages
impl Display for PauseResumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PauseResumeError::MultipleTasks(tasks) => f.write_fmt(format_args!(
                "Multiple tasks: {}",
                tasks
                    .iter()
                    .map(|t| t.name.to_owned())
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
            PauseResumeError::MultiplePauses(_) => f.write_str("Multiple pauses ongoing"),
            PauseResumeError::UpdateError(u) => {
                f.write_fmt(format_args!("Could not update task: '{}'", u.name))
            }
            PauseResumeError::SqlError(s) => f.write_str(s),
            PauseResumeError::NoTasks => f.write_str("No ongoing tasks"),
            PauseResumeError::NoPauses => f.write_str("No tasks which can be paused/resumed"),
        }
    }
}

pub fn pause(s: &ShiftDb, args: &Config) -> Result<(), PauseResumeError> {
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
                0 => return Err(PauseResumeError::NoTasks),
                1 => {
                    if let Some(t) = tasks_name_match.first() {
                        let pause = TaskPause::new(t.id.clone());
                        return match s.conn.execute(
                            "INSERT INTO task_pauses VALUES (?1, ?2, ?3, ?4)",
                            params![pause.id, pause.task_id, pause.start, pause.stop],
                        ) {
                            Ok(count) => {
                                if count == 1 {
                                    Ok(())
                                } else {
                                    Err(PauseResumeError::UpdateError(t.clone()))
                                }
                            }
                            Err(err) => Err(PauseResumeError::SqlError(err.to_string())),
                        };
                    }
                }
                2.. => {
                    return Err(PauseResumeError::MultipleTasks(tasks_name_match));
                }
            }
        }
        None if tasks.len() == 1 || args.all => {
            let query = "SELECT * FROM task_pauses WHERE stop IS NULL";
            let mut stmt = s.conn.prepare(query).expect("SQL statement is valid");
            let ongoing_pauses = stmt
                .query_map([], |row| TaskPause::try_from(row))
                .expect("No parameters should always bind correctly")
                .flatten()
                .map(|p| p.task_id)
                .collect::<Vec<String>>();

            let pauses = tasks
                .into_iter()
                .filter_map(|t| {
                    if ongoing_pauses.contains(&t.id) {
                        None
                    } else {
                        Some(TaskPause::new(t.id))
                    }
                })
                .collect::<Vec<TaskPause>>();

            // TODO update all at the same time
            if pauses.is_empty() {
                return Err(PauseResumeError::NoPauses);
            }
            for p in pauses {
                s.conn
                    .execute(
                        "INSERT INTO task_pauses VALUES (?1, ?2, ?3, ?4)",
                        params![p.id, p.task_id, p.start, p.stop],
                    )
                    .expect("SQL statement is vaild");
            }
        }
        None => match tasks.len() {
            0 => {
                return Err(PauseResumeError::NoTasks);
            }
            _ => {
                return Err(PauseResumeError::MultipleTasks(tasks));
            }
        },
    }

    Ok(())
}

pub fn resume(s: &ShiftDb, args: &Config) -> Result<(), PauseResumeError> {
    // TODO joint tasks and task_pauses so we get the name also from this query
    let task_pauses = s.get_ongoing_pauses();

    match &args.uid {
        // resume task with id (name or uuid)
        Some(id) => {
            let tasks_with_uid = s.get_tasks(id);

            match tasks_with_uid.len() {
                0 => return Err(PauseResumeError::NoTasks),
                1 => {
                    if let Some(t) = tasks_with_uid.first() {
                        let pause = s.is_paused(&t.id)?;

                        return match s.conn.execute(
                            "UPDATE task_pauses SET stop = ?1 WHERE id = ?2",
                            params![DateTime::<Utc>::from(Local::now()), pause.id],
                        ) {
                            Ok(count) => {
                                if count == 1 {
                                    Ok(())
                                } else {
                                    Err(PauseResumeError::UpdateError(t.clone()))
                                }
                            }
                            Err(err) => Err(PauseResumeError::SqlError(err.to_string())),
                        };
                    }
                }
                2.. => {
                    // It does not make sence to have two tasks with same name
                    // and have ongoing pauses, therefor this is not allowed.
                    return Err(PauseResumeError::MultipleTasks(tasks_with_uid));
                }
            }
        }
        None if task_pauses.len() == 1 || args.all => {
            s.conn
                .execute(
                    "UPDATE task_pauses SET stop = ?1 WHERE stop IS NULL",
                    params![DateTime::<Utc>::from(Local::now())],
                )
                .expect("SQL statement is vaild");
        }
        None => match task_pauses.len() {
            0 => {
                return Err(PauseResumeError::NoPauses);
            }
            _ => {
                return Err(PauseResumeError::MultiplePauses(task_pauses));
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{
        commands::{pause::PauseResumeError, stop::stop, tasks::tasks, test::start_with_name},
        Config, ShiftDb, Task,
    };

    use super::{pause, resume};

    #[test]
    fn resume_task() {
        let s = ShiftDb::new("");
        start_with_name(&s, "task1");
        let config = Config {
            ..Default::default()
        };

        pause(&s, &config).expect("Can pause task");
        resume(&s, &config).expect("Can resume paused task");
        stop(&s, &config).expect("Can stop after break");
    }

    #[test]
    fn resume_with_name() {
        let s = ShiftDb::new("");
        start_with_name(&s, "task1");
        start_with_name(&s, "task2");
        let config = Config {
            uid: Some("task2".to_string()),
            ..Default::default()
        };

        pause(&s, &config).expect("Can pause task");
        resume(&s, &config).expect("Can resume resume task");
        stop(&s, &config).expect("Can stop after break");

        let config = Config {
            count: 100,
            ..Default::default()
        };
        let tasks = tasks(&s, &config).expect("Should get task1 and task2");
        assert_eq!(tasks.len(), 2, "Started 2 tasks");
        assert_eq!(
            tasks
                .iter()
                .filter(|t| t.name == "task2")
                .collect::<Vec<&Task>>()
                .len(),
            1,
            "Stopped task2"
        )
    }

    #[test]
    fn resume_with_uuid() {
        let s = ShiftDb::new("");
        let task1 = start_with_name(&s, "task1");
        start_with_name(&s, "task2");
        let config = Config {
            uid: Some(task1.id.to_string()),
            ..Default::default()
        };

        pause(&s, &config).expect("Can pause task");
        resume(&s, &config).expect("Can resume resume task");
        stop(&s, &config).expect("Can stop after break");

        let config = Config {
            count: 100,
            ..Default::default()
        };
        let tasks = tasks(&s, &config).expect("Should get task1 and task2");
        assert_eq!(tasks.len(), 2, "Started 2 tasks");
        assert_eq!(s.get_tasks("task1").len(), 1, "Stopped task1");
    }

    #[test]
    fn resume_all() {
        let s = ShiftDb::new("");
        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }
        let config = Config {
            all: true,
            ..Default::default()
        };
        pause(&s, &config).expect("Can pause all task");
        assert_eq!(s.get_ongoing_pauses().len(), 100);
        resume(&s, &config).expect("Can resume resume all task");
        assert_eq!(s.get_ongoing_pauses().len(), 0, "Stopped all tasks");
    }

    #[test]
    fn pause_already_paused_task() {
        let s = ShiftDb::new("");
        start_with_name(&s, "t1");
        let config = Config {
            ..Default::default()
        };

        pause(&s, &config).expect("Allowed to pause first time");
        assert_eq!(
            pause(&s, &config).expect_err("Not allowd to pause a second time"),
            PauseResumeError::NoPauses
        );
    }

    #[test]
    fn resume_already_resumed_task() {
        let s = ShiftDb::new("");
        start_with_name(&s, "t1");
        let config = Config {
            ..Default::default()
        };

        pause(&s, &config).expect("Allowed to pause first time");
        resume(&s, &config).expect("Allowed to resume first time");
        assert_eq!(
            resume(&s, &config).expect_err("Not allowd to resume a second time"),
            PauseResumeError::NoPauses
        );
    }
}
