use std::{error::Error, fmt::Display};

use rusqlite::params;

use crate::{Config, ShiftDb, TaskEvent, TaskSession, TaskState};

#[derive(Debug, PartialEq, Eq)]
pub enum PauseResumeError {
    MultipleSessions(Vec<TaskSession>),
    MultiplePauses(Vec<TaskSession>),
    UpdateError(TaskSession),
    SqlError(String),
    NoTasks,
    NoPauses,
}

impl Error for PauseResumeError {}

// TODO split pause/resume so we can have better error messages
impl Display for PauseResumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PauseResumeError::MultipleSessions(tasks) => f.write_fmt(format_args!(
                "Multiple tasks: {}",
                tasks
                    .iter()
                    .map(|t| t.name.to_owned())
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
            PauseResumeError::MultiplePauses(sessions) => f.write_str("Multiple pauses ongoing"),
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
    let ongoing = s
        .ongoing_sessions()
        .into_iter()
        .filter(|s| !s.is_paused())
        .collect::<Vec<TaskSession>>();

    match &args.uid {
        Some(uid) => {
            let tasks_with_uid = ongoing
                .into_iter()
                .filter(|s| &s.name == uid || s.id.to_string().ends_with(uid))
                .collect::<Vec<TaskSession>>();
            match tasks_with_uid.len() {
                0 => return Err(PauseResumeError::NoTasks),
                1 => {
                    let t = tasks_with_uid
                        .first()
                        .expect("Sessions should have one element");
                    let pause = TaskEvent::new(t.name.to_string(), Some(t.id), TaskState::Paused);
                    return match s.conn.execute(
                        "INSERT INTO task_events VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            pause.id.to_string(),
                            pause.name,
                            pause.session.to_string(),
                            pause.state,
                            pause.time
                        ],
                    ) {
                        Ok(1) => Ok(()),
                        Ok(count) => Err(PauseResumeError::UpdateError(t.clone())),
                        Err(err) => Err(PauseResumeError::SqlError(err.to_string())),
                    };
                }
                2.. => {
                    return Err(PauseResumeError::MultipleSessions(tasks_with_uid));
                }
            }
        }
        None if ongoing.len() == 1 || args.all => {
            for session in ongoing {
                let e = TaskEvent::new(session.name, Some(session.id), TaskState::Paused);
                s.conn
                    .execute(
                        "INSERT INTO task_events VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![e.id, e.name, e.session, e.state, e.time],
                    )
                    .expect("SQL statement is vaild");
            }
        }
        None => match ongoing.len() {
            0 => {
                return Err(PauseResumeError::NoTasks);
            }
            _ => {
                return Err(PauseResumeError::MultipleSessions(ongoing));
            }
        },
    }

    Ok(())
}

pub fn resume(s: &ShiftDb, args: &Config) -> Result<(), PauseResumeError> {
    let task_pauses = s
        .ongoing_sessions()
        .into_iter()
        .filter(|s| s.is_paused())
        .collect::<Vec<TaskSession>>();

    match &args.uid {
        // resume task with id (name or uuid)
        Some(name) => {
            let tasks_with_uid = task_pauses
                .into_iter()
                .filter(|s| &s.name == name || s.id.to_string().ends_with(name))
                .collect::<Vec<TaskSession>>();

            match tasks_with_uid.len() {
                0 => return Err(PauseResumeError::NoTasks),
                1 => {
                    if let Some(t) = tasks_with_uid.first() {
                        let resume =
                            TaskEvent::new(t.name.to_string(), Some(t.id), TaskState::Resumed);
                        return match s.conn.execute(
                            "INSERT INTO task_events VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![
                                resume.id,
                                resume.name,
                                resume.session,
                                resume.state,
                                resume.time
                            ],
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
                    return Err(PauseResumeError::MultipleSessions(tasks_with_uid));
                }
            }
        }
        None if task_pauses.len() == 1 || args.all => {
            for p in task_pauses {
                let resume = TaskEvent::new(p.name.to_string(), Some(p.id), TaskState::Resumed);
                s.conn
                    .execute(
                        "INSERT INTO task_events VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            resume.id,
                            resume.name,
                            resume.session,
                            resume.state,
                            resume.time
                        ],
                    )
                    .expect("SQL statement is vaild");
            }
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
        commands::{
            pause::PauseResumeError, sessions::sessions, stop::stop, test::start_with_name,
        },
        Config, ShiftDb, TaskEvent, TaskSession,
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
        let tasks = sessions(&s, &config).expect("Should get task1 and task2");
        assert_eq!(tasks.len(), 2, "Started 2 tasks");
        assert_eq!(
            tasks.iter().filter(|t| t.name == "task2").count(),
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
            uid: Some(task1.session.to_string()),
            ..Default::default()
        };

        pause(&s, &config).expect("Can pause task");
        resume(&s, &config).expect("Can resume resume task");
        stop(&s, &config).expect("Can stop after break");

        let config = Config {
            count: 100,
            ..Default::default()
        };
        let tasks = sessions(&s, &config).expect("Should get task1 and task2");
        assert_eq!(tasks.len(), 2, "Started 2 tasks");
        assert_eq!(s.ongoing_sessions().len(), 1, "Stopped task1");
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
        assert_eq!(
            s.ongoing_sessions()
                .iter()
                .filter(|s| s.is_paused())
                .count(),
            100
        );
        resume(&s, &config).expect("Can resume resume all task");
        assert_eq!(
            s.ongoing_sessions()
                .iter()
                .filter(|s| s.is_paused())
                .count(),
            0,
            "Stopped all tasks"
        );
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
            PauseResumeError::NoTasks
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
