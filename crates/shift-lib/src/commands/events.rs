use std::str::FromStr;

use chrono::{DateTime, Local};
use rusqlite::{params, version, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{ShiftDb, TaskEvent, TaskSession, TaskState};

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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EventStatOpts {
    pub from: DateTime<Local>,
    pub to: DateTime<Local>,
}

// Summarise
pub fn event_stats(mut events: Vec<TaskEvent>, opts: &EventStatOpts) -> Vec<TaskSession> {
    let mut partial_sessions: Vec<TaskSession> = Vec::new();
    let mut sessions: Vec<TaskSession> = Vec::new();
    // TODO events are given in backward order
    events.reverse();
    for event in events {
        match event.state {
            TaskState::Started => {
                assert_eq!(
                    partial_sessions
                        .iter_mut()
                        .find(|e| e.id.to_string() == event.session),
                    None,
                    "Invalid state, session with id {} already been started",
                    event.session
                );
                partial_sessions.push(TaskSession {
                    id: Uuid::from_str(&event.session)
                        .expect("Could not deserialize id as an uuid"),
                    name: event.name.to_string(),
                    events: vec![event],
                });
            }
            TaskState::Paused | TaskState::Resumed => {
                if let Some(session) = partial_sessions
                    .iter_mut()
                    .find(|e| e.id.to_string() == event.session)
                {
                    session.events.push(event);
                } else {
                    partial_sessions.push(TaskSession {
                        id: Uuid::from_str(&event.session)
                            .expect("Could not deserialize session id as an uuid"),
                        name: event.name.to_string(),
                        events: vec![event],
                    })
                }
            }
            TaskState::Stopped => {
                let position = partial_sessions
                    .iter()
                    .position(|s| s.id.to_string() == event.session);
                match position {
                    None => {
                        sessions.push(TaskSession {
                            id: Uuid::from_str(&event.session)
                                .expect("Could not deserialize session id as an uuid"),
                            name: event.name.to_string(),
                            events: vec![
                                TaskEvent::new(
                                    Uuid::now_v7().to_string(),
                                    Some(
                                        Uuid::from_str(&event.session)
                                            .expect("Could not deserialize session id as an uuid"),
                                    ),
                                    Some(opts.from),
                                    TaskState::Started,
                                ),
                                event,
                            ],
                        });
                    }
                    Some(pos) => {
                        let mut session = partial_sessions.swap_remove(pos);
                        session.events.push(event);
                        sessions.push(session);
                    }
                }
            }
        }
    }

    loop {
        if let Some(mut s) = partial_sessions.pop() {
            s.events.push(TaskEvent::new(
                Uuid::now_v7().to_string(),
                Some(
                    Uuid::from_str(
                        &s.events
                            .first()
                            .expect("Should have atleast one event in session")
                            .session,
                    )
                    .expect("Could not deserialize session id as an uuid"),
                ),
                Some(opts.to),
                TaskState::Stopped,
            ));
            sessions.push(s);
        } else {
            break;
        }
    }

    sessions
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
