use std::{
    fmt::{Display, Write},
    path::Path,
};

use anyhow::anyhow;
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    id: Uuid,
    name: String,

    start: DateTime<Utc>,
    // https://serde.rs/field-attrs.html
    stop: Option<DateTime<Utc>>,
}

impl Task {
    fn new(name: String) -> Self {
        Self {
            id: Uuid::now_v7(),
            name,
            start: DateTime::from(Local::now()),
            stop: None,
        }
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

struct ShiftDb {}

impl ShiftDb {
    // for test's use inmemory datebase
    // use ~/.local/share/shift/ for database?
    fn connection() -> anyhow::Result<Connection> {
        let conn = Connection::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("tasks.db"))?;
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
        )?;
        Ok(conn)
    }
}

// https://serde.rs/custom-date-format.html

pub fn start(task_name: &str) -> anyhow::Result<()> {
    let task = Task::new(task_name.to_string());
    let conn = ShiftDb::connection()?;

    conn.execute(
        "INSERT INTO tasks VALUES (?1, ?2, ?3, ?4)",
        params![task.id.to_string(), task.name, task.start, task.stop],
    )?;
    Ok(())
}

pub struct Config {
    pub json: bool,
    pub uuid: Option<String>,
}

// Get curret ongoing task(s)
pub fn status(_args: &Config) -> anyhow::Result<()> {
    let conn = ShiftDb::connection()?;
    let query = "SELECT * FROM tasks WHERE stop IS NULL";

    let mut stmt = conn.prepare(query)?;
    let task_iter = stmt.query_map([], |row| {
        Ok(Task {
            id: Uuid::parse_str(&row.get::<usize, String>(0)?).unwrap(),
            name: row.get(1)?,
            start: row.get(2)?,
            stop: row.get(3)?,
        })
    })?;
    task_iter.for_each(|t| {
        if let Ok(task) = t {
            println!("{task}");
        }
    });
    Ok(())
}

// TODO add options to function
pub fn log() -> anyhow::Result<()> {
    // show all task

    let conn = ShiftDb::connection()?;
    let query = "SELECT * FROM tasks";
    let mut stmt = conn.prepare(query)?;
    let task_iter = stmt.query_map([], |row| {
        Ok(Task {
            id: Uuid::parse_str(&row.get::<usize, String>(0)?).unwrap(),
            name: row.get(1)?,
            start: row.get(2)?,
            stop: row.get(3)?,
        })
    })?;

    // should never contain errors
    for task in task_iter.flatten() {
        println!("{task}");
    }
    Ok(())
}

// TODO stop task, e.g update database
pub fn stop(args: &Config) -> anyhow::Result<()> {
    let conn = ShiftDb::connection()?;
    let query = "SELECT * FROM tasks WHERE stop IS NULL";
    let mut stmt = conn.prepare(query)?;
    let task_iter = stmt.query_map([], |row| {
        Ok(Task {
            id: Uuid::parse_str(&row.get::<usize, String>(0)?).unwrap(),
            name: row.get(1)?,
            start: row.get(2)?,
            stop: row.get(3)?,
        })
    })?;

    match &args.uuid {
        Some(id) => {
            conn.execute(
                "
                UPDATE tasks 
                SET stop = ?1
                WHERE id LIKE ?2
                ",
                params![DateTime::<Utc>::from(Local::now()), format!("%{id}")],
            )?;
        }
        None if task_iter.count() == 1 => {
            conn.execute(
                "UPDATE tasks SET stop = ?1 WHERE stop IS NULL",
                params![DateTime::<Utc>::from(Local::now())],
            )?;
        }
        // TODO other kinds of error types which maps to cli options?
        None => return Err(anyhow!("Need to specify id")),
    }

    Ok(())
}
