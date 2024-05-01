use std::{error::Error, fmt::Display, path::Path};

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: String,
    pub name: String,

    pub start: DateTime<Utc>,
    pub stop: Option<DateTime<Utc>>,
}

impl Task {
    fn new(name: String) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            name,
            start: DateTime::from(Local::now()),
            stop: None,
        }
    }
}

impl<'a> TryFrom<&Row<'a>> for Task {
    type Error = rusqlite::Error;

    fn try_from(value: &Row) -> Result<Self, Self::Error> {
        Ok(Task {
            id: value.get(0)?,
            name: value.get(1)?,
            start: value.get(2)?,
            stop: value.get(3)?,
        })
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            self.id
                .get(self.id.len() - 7..)
                .expect("Could not get subslice of uuid"),
        )?;
        f.write_str("  ")?;
        self.start.naive_local().date().fmt(f)?;
        f.write_str(" ")?;
        self.start.naive_local().format("%H:%M:%S").fmt(f)?;
        if let Some(stop_time) = self.stop {
            f.write_str(" to ")?;
            if self.start.naive_local().date() != stop_time.naive_local().date() {
                stop_time.naive_local().date().fmt(f)?;
                f.write_str(" ")?;
            }
            stop_time.naive_local().format("%H:%M:%S").fmt(f)?;
            f.write_str("  ")?;
            let duration = stop_time - self.start;
            if duration.num_hours() != 0 {
                f.write_fmt(format_args!("{}h ", duration.num_hours()))?;
            }
            if duration.num_minutes() % 60 != 0 || duration.num_hours() != 0 {
                f.write_fmt(format_args!("{}m ", duration.num_minutes() % 60))?;
            }
            f.write_fmt(format_args!("{}s", duration.num_seconds() % 60))?;
        }

        f.write_str("  ")?;
        f.write_str(&self.name)?;
        Ok(())
    }
}

impl<'a> TryFrom<&Row<'a>> for TaskPause {
    type Error = rusqlite::Error;

    fn try_from(value: &Row) -> Result<Self, Self::Error> {
        Ok(TaskPause {
            id: value.get(0)?,
            task_id: value.get(1)?,
            start: value.get(2)?,
            stop: value.get(3)?,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TaskPause {
    id: String,
    task_id: String,
    pub start: DateTime<Utc>,
    pub stop: Option<DateTime<Utc>>,
}

impl TaskPause {
    fn new(task_id: String) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            task_id,
            start: DateTime::from(Local::now()),
            stop: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Config {
    pub uid: Option<String>,
    pub from: Option<DateTime<Local>>,
    pub to: Option<DateTime<Local>>,
    pub tasks: Vec<String>,
    pub count: usize,
    pub all: bool,
    pub start_time: Option<DateTime<Local>>,
}

pub struct Shift {
    conn: Connection,
}

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

#[derive(Debug)]
pub enum StartError {
    Ongoing(String),
    SqlError(String),
}

impl Error for StartError {}

impl Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("todo")
    }
}

impl Shift {
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let conn = Connection::open(path).expect("could not open database");
        conn.execute_batch(
            "BEGIN;
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                start DATETIME NOT NULL,
                stop DATETIME
            );
            CREATE TABLE IF NOT EXISTS task_pauses (
                id TEXT PRIMARY KEY NOT NULL,
                task_id TEXT NOT NULL,
                start DATETIME NOT NULL,
                stop DATETIME
            );
            COMMIT;
            ",
        )
        .expect("could not start database connection");
        Self { conn }
    }
}

impl Shift {
    // https://serde.rs/custom-date-format.html

    pub fn start(&self, args: &Config) -> Result<Task, StartError> {
        let mut task = Task::new(args.uid.clone().expect("Required to specify task name"));
        if let Some(start_time) = args.start_time {
            task.start = start_time.into()
        }

        match self.conn.execute(
            "INSERT INTO tasks 
             SELECT ?1, ?2, ?3, ?4
             WHERE NOT EXISTS(
                 SELECT * FROM tasks
                 WHERE name = ?2 AND stop IS NULL
             );",
            params![task.id.to_string(), task.name, task.start, task.stop],
        ) {
            Ok(1) => Ok(task),
            Ok(0) => Err(StartError::Ongoing(task.name)),
            Ok(u) => Err(StartError::SqlError(format!(
                "Inserted {} tasks when only expected 1",
                u
            ))),
            Err(e) => Err(StartError::SqlError(e.to_string())),
        }
    }

    // Get curret ongoing task(s)
    pub fn status(&self, _args: &Config) -> anyhow::Result<()> {
        let query = "SELECT * FROM tasks WHERE stop IS NULL";

        let mut stmt = self.conn.prepare(query)?;
        let task_iter = stmt.query_map([], |row| Task::try_from(row))?;
        task_iter.for_each(|t| {
            if let Ok(task) = t {
                println!("{task}");
            }
        });
        Ok(())
    }

    /// Retrieve the tasks from the database
    pub fn tasks(&self, args: &Config) -> anyhow::Result<Vec<Task>> {
        let row_to_task = |row: &Row<'_>| Task::try_from(row);
        let mut stmt;
        let task_iter = match (args.to, args.from) {
            (Some(to_date), Some(from_date)) => {
                let query =
                    "SELECT * FROM tasks WHERE start > ? and start < ? ORDER BY start DESC LIMIT ?";
                stmt = self.conn.prepare(query)?;
                if args.all || !args.tasks.is_empty() {
                    stmt.query_map(params![from_date, to_date, -1], row_to_task)?
                } else {
                    stmt.query_map(params![from_date, to_date, args.count], row_to_task)?
                }
            }
            (None, Some(from_date)) => {
                let query = "SELECT * FROM tasks WHERE start > ? ORDER BY start DESC LIMIT ?";
                stmt = self.conn.prepare(query)?;
                if args.all || !args.tasks.is_empty() {
                    stmt.query_map(params![from_date, -1], row_to_task)?
                } else {
                    stmt.query_map(params![from_date, args.count], row_to_task)?
                }
            }
            (Some(to_date), None) => {
                let query = "SELECT * FROM tasks WHERE start < ? ORDER BY start DESC LIMIT ?";
                stmt = self.conn.prepare(query)?;
                if args.all || !args.tasks.is_empty() {
                    stmt.query_map(params![to_date, -1], row_to_task)?
                } else {
                    stmt.query_map(params![to_date, args.count], row_to_task)?
                }
            }
            (None, None) => {
                let query = "SELECT * FROM tasks ORDER BY start DESC LIMIT ?";
                stmt = self.conn.prepare(query)?;
                if args.all || !args.tasks.is_empty() {
                    stmt.query_map([-1], row_to_task)?
                } else {
                    stmt.query_map([args.count], row_to_task)?
                }
            }
        };

        let iter = task_iter.flatten();
        let res = if !args.tasks.is_empty() {
            let filtered = iter.filter(|t| args.tasks.contains(&t.name));
            if args.all {
                filtered.collect::<Vec<Task>>()
            } else {
                filtered.take(args.count).collect::<Vec<Task>>()
            }
        } else {
            iter.collect::<Vec<Task>>()
        };

        Ok(res)
    }

    /// Update task with stop time
    pub fn stop(&self, args: &Config) -> Result<(), StopError> {
        let query = "SELECT * FROM tasks WHERE stop IS NULL";
        let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
        let tasks = stmt
            .query_map([], |row| Task::try_from(row))
            .expect("No parameters should always bind correctly")
            .flatten()
            .collect::<Vec<Task>>();

        match &args.uid {
            Some(id) => {
                let tasks_name_match = self.get_tasks(id);
                match tasks_name_match.len() {
                    0 => {
                        dbg!(&tasks_name_match);
                        return Err(StopError::NoTasks);
                    }
                    1 => {
                        if let Some(t) = tasks_name_match.first() {
                            return match self.conn.execute(
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

                self.conn
                    .execute(
                        "UPDATE tasks SET stop = ?1 WHERE id LIKE ?2",
                        params![DateTime::<Utc>::from(Local::now()), format!("%{id}")],
                    )
                    .expect("SQL statement is valid");
            }
            None if tasks.len() == 1 || args.all => {
                self.conn
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
}

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

impl Shift {
    pub fn pause(&self, args: &Config) -> Result<(), PauseResumeError> {
        let query = "SELECT * FROM tasks WHERE stop IS NULL";
        let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
        let tasks = stmt
            .query_map([], |row| Task::try_from(row))
            .expect("No parameters should always bind correctly")
            .flatten()
            .collect::<Vec<Task>>();

        match &args.uid {
            Some(id) => {
                let tasks_name_match = self.get_tasks(id);
                match tasks_name_match.len() {
                    0 => return Err(PauseResumeError::NoTasks),
                    1 => {
                        if let Some(t) = tasks_name_match.first() {
                            let pause = TaskPause::new(t.id.clone());
                            return match self.conn.execute(
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
                let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
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
                    self.conn
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

    fn get_tasks(&self, uid: &str) -> Vec<Task> {
        let query = "SELECT * FROM tasks WHERE id LIKE ?1 OR name = ?2";
        let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
        stmt.query_map([format!("%{uid}"), uid.to_string()], |row| {
            Task::try_from(row)
        })
        .expect("No parameters should always bind correctly")
        .flatten()
        .collect::<Vec<Task>>()
    }

    fn get_ongoing_pauses(&self) -> Vec<TaskPause> {
        let query = "SELECT * FROM task_pauses WHERE stop IS NULL";
        let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
        stmt.query_map([], |row| TaskPause::try_from(row))
            .expect("No parameters should always bind correctly")
            .flatten()
            .collect::<Vec<TaskPause>>()
    }

    fn is_paused(&self, uuid: &str) -> Result<TaskPause, PauseResumeError> {
        let query = "SELECT * FROM task_pauses WHERE task_id = ?1 AND stop IS NULL";
        let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
        let task_pauses = stmt
            .query_map([uuid], |row| TaskPause::try_from(row))
            .expect("No parameters should always bind correctly")
            .flatten()
            .collect::<Vec<TaskPause>>();

        match task_pauses.len() {
            // TODO do not clone
            1 => Ok(task_pauses.first().expect("Vec length is 1").clone()),
            0 => Err(PauseResumeError::NoTasks),
            _ => Err(PauseResumeError::MultiplePauses(task_pauses)),
        }
    }

    pub fn resume(&self, args: &Config) -> Result<(), PauseResumeError> {
        // TODO joint tasks and task_pauses so we get the name also from this query
        let task_pauses = self.get_ongoing_pauses();

        match &args.uid {
            // resume task with id (name or uuid)
            Some(id) => {
                let tasks_with_uid = self.get_tasks(id);

                match tasks_with_uid.len() {
                    0 => return Err(PauseResumeError::NoTasks),
                    1 => {
                        if let Some(t) = tasks_with_uid.first() {
                            let pause = self.is_paused(&t.id)?;

                            return match self.conn.execute(
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
                self.conn
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
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, Local};

    use crate::{Config, PauseResumeError, Shift, StopError, Task};

    fn start_with_name(shift: &Shift, s: &str) -> Task {
        let config = Config {
            uid: Some(s.to_string()),
            ..Default::default()
        };
        shift.start(&config).unwrap()
    }

    #[test]
    fn start_time() {
        let s = Shift::new("");

        let time = DateTime::from(Local::now());
        let config = Config {
            uid: Some("task1".to_string()),
            start_time: Some(time),
            ..Default::default()
        };
        s.start(&config).unwrap();

        let config = Config {
            count: 50,
            ..Default::default()
        };
        let tasks = s.tasks(&config);
        assert_eq!(tasks.unwrap()[0].start, time, "Start time not handled");
    }

    #[test]
    fn log_count_limit() {
        let s = Shift::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }
        let config = Config {
            count: 2,
            ..Default::default()
        };

        let tasks = s.tasks(&config);
        assert_eq!(tasks.unwrap().len(), 2);
    }

    #[test]
    fn log_desc() {
        let s = Shift::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 4,
            ..Default::default()
        };
        let tasks = s.tasks(&config);
        assert_eq!(
            tasks
                .unwrap()
                .iter()
                .map(|t| &t.name)
                .collect::<Vec<&String>>(),
            vec!["task99", "task98", "task97", "task96"]
        );
    }

    #[test]
    fn log_all() {
        let s = Shift::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 4,
            all: true,
            ..Default::default()
        };
        let tasks = s.tasks(&config);
        assert_eq!(tasks.unwrap().len(), 100);
    }

    #[test]
    fn log_task() {
        let s = Shift::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 100,
            tasks: vec!["task1".to_string(), "task2".to_string()],
            ..Default::default()
        };
        let tasks = s.tasks(&config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 2);
        assert_eq!(
            tasks.iter().map(|t| &t.name).collect::<Vec<&String>>(),
            vec!["task2", "task1"]
        )
    }

    #[test]
    fn log_task_limit() {
        let s = Shift::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 3,
            tasks: vec![
                "task1".to_string(),
                "task2".to_string(),
                "task3".to_string(),
                "task4".to_string(),
            ],
            ..Default::default()
        };
        let tasks = s.tasks(&config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 3);
        assert_eq!(
            tasks.iter().map(|t| &t.name).collect::<Vec<&String>>(),
            vec!["task4", "task3", "task2"]
        )
    }

    #[test]
    fn stop() {
        let s = Shift::new("");

        start_with_name(&s, "task1");

        let config = Config {
            count: 10,
            ..Default::default()
        };
        s.stop(&config).expect("Should stop without error");
        let tasks = s.tasks(&config).expect("Should get task1");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        assert!(tasks[0].stop != None, "the task stop field was not set")
    }

    #[test]
    fn stop_error_multiple_tasks() {
        let s = Shift::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");

        let config = Config {
            count: 10,
            ..Default::default()
        };
        let a = s.stop(&config).expect_err("Can't stop two tasks");
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
        let s = Shift::new("");

        start_with_name(&s, "task1");
        start_with_name(&s, "task2");

        let config = Config {
            all: true,
            ..Default::default()
        };
        s.stop(&config).expect("Can stop all");
        let tasks = s.tasks(&config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 2, "Didn't get expected amount of tasks");
        for t in tasks {
            assert!(t.stop != None, "the task stop field was not set: {t:?}")
        }
    }

    #[test]
    fn stop_with_name() {
        let s = Shift::new("");

        start_with_name(&s, "task1");

        let config = Config {
            uid: Some("task1".to_string()),
            ..Default::default()
        };

        s.stop(&config).expect("Can stop with name");
        let config = Config {
            all: true,
            ..Default::default()
        };
        let tasks = s.tasks(&config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 1, "Didn't get expected amount of tasks");
        assert!(
            tasks.first().unwrap().stop != None,
            "the task stop field was not set: {:?}",
            tasks.first()
        )
    }

    #[test]
    fn resume() {
        let s = Shift::new("");
        start_with_name(&s, "task1");
        let config = Config {
            ..Default::default()
        };

        s.pause(&config).expect("Can pause task");
        s.resume(&config).expect("Can resume paused task");
        s.stop(&config).expect("Can stop after break");
    }

    #[test]
    fn resume_with_name() {
        let s = Shift::new("");
        start_with_name(&s, "task1");
        start_with_name(&s, "task2");
        let config = Config {
            uid: Some("task2".to_string()),
            ..Default::default()
        };

        s.pause(&config).expect("Can pause task");
        s.resume(&config).expect("Can resume resume task");
        s.stop(&config).expect("Can stop after break");

        let config = Config {
            count: 100,
            ..Default::default()
        };
        let tasks = s.tasks(&config).expect("Should get task1 and task2");
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
        let s = Shift::new("");
        let task1 = start_with_name(&s, "task1");
        start_with_name(&s, "task2");
        let config = Config {
            uid: Some(task1.id.to_string()),
            ..Default::default()
        };

        s.pause(&config).expect("Can pause task");
        s.resume(&config).expect("Can resume resume task");
        s.stop(&config).expect("Can stop after break");

        let config = Config {
            count: 100,
            ..Default::default()
        };
        let tasks = s.tasks(&config).expect("Should get task1 and task2");
        assert_eq!(tasks.len(), 2, "Started 2 tasks");
        assert_eq!(s.get_tasks("task1").len(), 1, "Stopped task1");
    }

    #[test]
    fn resume_all() {
        let s = Shift::new("");
        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }
        let config = Config {
            all: true,
            ..Default::default()
        };
        s.pause(&config).expect("Can pause all task");
        assert_eq!(s.get_ongoing_pauses().len(), 100);
        s.resume(&config).expect("Can resume resume all task");
        assert_eq!(s.get_ongoing_pauses().len(), 0, "Stopped all tasks");
    }

    #[test]
    fn pause_already_paused_task() {
        let s = Shift::new("");
        start_with_name(&s, "t1");
        let config = Config {
            ..Default::default()
        };

        s.pause(&config).expect("Allowed to pause first time");
        assert_eq!(
            s.pause(&config)
                .expect_err("Not allowd to pause a second time"),
            PauseResumeError::NoPauses
        );
    }

    #[test]
    fn resume_already_resumed_task() {
        let s = Shift::new("");
        start_with_name(&s, "t1");
        let config = Config {
            ..Default::default()
        };

        s.pause(&config).expect("Allowed to pause first time");
        s.resume(&config).expect("Allowed to resume first time");
        assert_eq!(
            s.resume(&config)
                .expect_err("Not allowd to resume a second time"),
            PauseResumeError::NoPauses
        );
    }
}
