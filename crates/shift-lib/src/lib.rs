use std::{fmt::Display, path::Path};

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use commands::pause::PauseResumeError;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod commands;

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

pub struct ShiftDb {
    conn: Connection,
}

impl ShiftDb {
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

impl ShiftDb {
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
}
