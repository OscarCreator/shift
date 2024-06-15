use chrono::{DateTime, Local};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ShiftDb, TaskEvent};

#[derive(Debug, Error)]
pub enum Error {
    #[error("TODO")]
    A,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Opts {
    pub from: Option<DateTime<Local>>,
    pub to: Option<DateTime<Local>>,
    pub count: Option<usize>,
    pub tasks: Vec<String>,
}

pub fn events(s: &ShiftDb, opts: &Opts) -> Result<Vec<TaskEvent>, Error> {
    let row_to_events = |row: &Row<'_>| TaskEvent::try_from(row);
    let mut stmt;
    let events = match (opts.to, opts.from) {
        (Some(to_date), Some(from_date)) => {
            let query =
                "SELECT * FROM task_events WHERE time > ?1 and time < ?2 ORDER BY time DESC LIMIT ?3";
            stmt = s.conn.prepare(query).expect("SQL statement is correct");
            if opts.count.is_none() || !opts.tasks.is_empty() {
                stmt.query_map(params![from_date, to_date, -1], row_to_events)
                    .expect("Parameters should always bind correctly")
            } else {
                stmt.query_map(params![from_date, to_date, opts.count], row_to_events)
                    .expect("Parameters should always bind correctly")
            }
        }
        (None, Some(from_date)) => {
            let query = "SELECT * FROM task_events WHERE time > ?1 ORDER BY time DESC LIMIT ?2";
            stmt = s.conn.prepare(query).expect("SQL statement is correct");
            if opts.count.is_none() || !opts.tasks.is_empty() {
                stmt.query_map(params![from_date, -1], row_to_events)
                    .expect("Parameters should always bind correctly")
            } else {
                stmt.query_map(params![from_date, opts.count], row_to_events)
                    .expect("Parameters should always bind correctly")
            }
        }
        (Some(to_date), None) => {
            let query = "SELECT * FROM task_events WHERE time < ?1 ORDER BY time DESC LIMIT ?2";
            stmt = s.conn.prepare(query).expect("SQL statement is correct");
            if opts.count.is_none() || !opts.tasks.is_empty() {
                stmt.query_map(params![to_date, -1], row_to_events)
                    .expect("Parameters should always bind correctly")
            } else {
                stmt.query_map(params![to_date, opts.count], row_to_events)
                    .expect("Parameters should always bind correctly")
            }
        }
        (None, None) => {
            let query = "SELECT * FROM task_events ORDER BY time DESC LIMIT ?1";
            stmt = s.conn.prepare(query).expect("SQL statement is correct");
            if opts.count.is_none() || !opts.tasks.is_empty() {
                stmt.query_map([-1], row_to_events)
                    .expect("Parameters should always bind correctly")
            } else {
                stmt.query_map([opts.count], row_to_events)
                    .expect("Parameters should always bind correctly")
            }
        }
    };
    let parsed_events =
        events.map(|e| e.expect("Database corrupt, could not parse event from database"));

    let res = if !opts.tasks.is_empty() {
        let filtered = parsed_events
            .into_iter()
            .filter(|t| opts.tasks.contains(&t.name));
        if let Some(count) = opts.count {
            filtered.take(count).collect()
        } else {
            filtered.collect()
        }
    } else {
        parsed_events.collect()
    };

    Ok(res)
}
