use std::{
    fmt::{Display, Write},
    path::Path,
};

use anyhow::anyhow;
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    id: String,
    name: String,

    start: DateTime<Utc>,
    // https://serde.rs/field-attrs.html
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
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub tasks: Vec<String>,
    pub count: usize,
}

pub struct Shift {
    conn: Connection,
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

    pub fn start(&self, task_name: &str) -> anyhow::Result<()> {
        let task = Task::new(task_name.to_string());

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

    // TODO add options to function
    pub fn log(&self, args: &Config) -> anyhow::Result<Vec<Task>> {
        // show all task

        let query = "SELECT * FROM tasks ORDER BY start DESC LIMIT ?";
        let mut stmt = self.conn.prepare(query)?;
        let task_iter = stmt.query_map([args.count], |row| Task::try_from(row))?;

        // should never contain errors
        //for task in task_iter.flatten() {
        //    println!("{task}");
        //}
        Ok(task_iter.flatten().collect::<Vec<Task>>())
    }

    // TODO stop task, e.g update database
    pub fn stop(&self, args: &Config) -> anyhow::Result<()> {
        let query = "SELECT * FROM tasks WHERE stop IS NULL";
        let mut stmt = self.conn.prepare(query)?;
        let task_iter = stmt.query_map([], |row| Task::try_from(row))?;

        match &args.uid {
            Some(id) => {
                self.conn.execute(
                    "
                    UPDATE tasks 
                    SET stop = ?1
                    WHERE id LIKE ?2
                    ",
                    params![DateTime::<Utc>::from(Local::now()), format!("%{id}")],
                )?;
            }
            None if task_iter.count() == 1 => {
                self.conn.execute(
                    "UPDATE tasks SET stop = ?1 WHERE stop IS NULL",
                    params![DateTime::<Utc>::from(Local::now())],
                )?;
            }
            // TODO other kinds of error types which maps to cli options?
            None => return Err(anyhow!("Need to specify id")),
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::{Config, Shift};

    #[test]
    fn log_count_limit() {
        let s = Shift::new("");

        for i in 0..100 {
            s.start(&format!("task{}", i)).unwrap();
        }

        let config = Config {
            count: 2,
            ..Default::default()
        };
        let tasks = s.log(&config);
        assert_eq!(tasks.unwrap().len(), 2);
    }

    #[test]
    fn log_desc() {
        let s = Shift::new("");

        for i in 0..100 {
            s.start(&format!("task{}", i)).unwrap();
        }

        let config = Config {
            count: 4,
            ..Default::default()
        };
        let tasks = s.log(&config);
        assert_eq!(
            tasks
                .unwrap()
                .iter()
                .map(|t| &t.name)
                .collect::<Vec<&String>>(),
            vec!["task99", "task98", "task97", "task96"]
        );
    }
}
