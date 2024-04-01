use std::{
    error::Error,
    fmt::{Display, Write},
    path::Path,
};

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    id: String,
    name: String,

    start: DateTime<Utc>,
    stop: Option<DateTime<Utc>>,
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
        f.write_str(&self.id.to_string())?;
        f.write_char(',')?;
        f.write_str(&self.name)?;
        f.write_char(',')?;
        f.write_fmt(format_args!("{}", &self.start))?;
        if let Some(stop) = &self.stop {
            f.write_char(',')?;
            f.write_fmt(format_args!("{stop}"))?;
        };
        Ok(())
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
}

impl Error for StopError {}

impl Display for StopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("hi")
    }
}

impl Shift {
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let conn = Connection::open(path).expect("could not open database");
        conn.execute(
            "
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                start DATETIME NOT NULL,
                stop DATETIME
            )
            ",
            (),
        )
        .expect("could not start database connection");
        Self { conn }
    }

    // https://serde.rs/custom-date-format.html

    pub fn start(&self, args: &Config) -> anyhow::Result<()> {
        let mut task = Task::new(args.uid.clone().expect("Required to specify task name"));
        if let Some(start_time) = args.start_time {
            task.start = start_time.into()
        }

        self.conn.execute(
            "INSERT INTO tasks VALUES (?1, ?2, ?3, ?4)",
            params![task.id.to_string(), task.name, task.start, task.stop],
        )?;
        Ok(())
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
            None => {
                return Err(StopError::MultipleTasks(tasks));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, Local, Utc};

    use crate::{Config, Shift, StopError};

    fn start_with_name(shift: &Shift, s: String) {
        let config = Config {
            uid: Some(s),
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
            start_with_name(&s, format!("task{}", i));
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
            start_with_name(&s, format!("task{}", i));
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
            start_with_name(&s, format!("task{}", i));
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
            start_with_name(&s, format!("task{}", i));
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
            start_with_name(&s, format!("task{}", i));
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

        start_with_name(&s, "task1".to_string());

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

        start_with_name(&s, "task1".to_string());
        start_with_name(&s, "task2".to_string());

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
        }
    }

    #[test]
    fn stop_all() {
        let s = Shift::new("");

        start_with_name(&s, "task1".to_string());
        start_with_name(&s, "task2".to_string());

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
}
