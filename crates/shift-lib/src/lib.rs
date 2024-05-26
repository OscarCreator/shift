use std::{collections::HashMap, fmt::Display, path::Path, str::FromStr};

use chrono::{DateTime, Local, Utc};
use rusqlite::{
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
    Connection, Row, ToSql,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod commands;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Started,
    Stopped,
    Paused,
    Resumed,
}

impl Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Started => write!(f, "Started"),
            TaskState::Stopped => write!(f, "Stopped"),
            TaskState::Paused => write!(f, "Paused"),
            TaskState::Resumed => write!(f, "Resumed"),
        }
    }
}

impl ToSql for TaskState {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl FromSql for TaskState {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "Started" => Ok(TaskState::Started),
            "Stopped" => Ok(TaskState::Stopped),
            "Paused" => Ok(TaskState::Paused),
            "Resumed" => Ok(TaskState::Resumed),
            _ => unreachable!("couldn't parse TaskState from string"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub(crate) id: String,
    pub name: String,
    pub(crate) session: String,
    pub state: TaskState,
    pub time: DateTime<Utc>,
}

impl TaskEvent {
    fn new(name: String, session: Option<Uuid>, state: TaskState) -> Self {
        let session_id = session.map_or(Uuid::now_v7(), |a| a);
        Self {
            id: Uuid::now_v7().to_string(),
            name,
            session: session_id.to_string(),
            state,
            time: DateTime::from(Local::now()),
        }
    }
}

impl Display for TaskEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl<'a> TryFrom<&Row<'a>> for TaskEvent {
    type Error = rusqlite::Error;

    fn try_from(value: &Row<'a>) -> Result<Self, Self::Error> {
        Ok(TaskEvent {
            id: value.get(0)?,
            name: value.get(1)?,
            session: value.get(2)?,
            state: value.get(3)?,
            time: value.get(4)?,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TaskSession {
    pub(crate) id: Uuid,
    pub name: String,
    // Should be events starting from latest start and onwards
    pub events: Vec<TaskEvent>,
}

impl TaskSession {
    fn new(name: String, uuid: Uuid) -> Self {
        Self {
            id: uuid,
            name,
            events: vec![],
        }
    }

    fn is_completed(&self) -> bool {
        self.events
            .iter()
            .filter(|e| e.state == TaskState::Stopped)
            .count()
            == 1
    }

    fn is_paused(&self) -> bool {
        if let Some(e) = self.events.first() {
            if e.state == TaskState::Paused {
                return true;
            }
        }
        false
    }
}

// TODO cli part should handle this
impl Display for TaskSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        //f.write_str(
        //    self.id
        //        .get(self.id.len() - 7..)
        //        .expect("Could not get subslice of uuid"),
        //)?;
        //f.write_str("  ")?;
        //self.start.naive_local().date().fmt(f)?;
        //f.write_str(" ")?;
        //self.start.naive_local().format("%H:%M:%S").fmt(f)?;
        //if let Some(stop_time) = self.stop {
        //    f.write_str(" to ")?;
        //    if self.start.naive_local().date() != stop_time.naive_local().date() {
        //        stop_time.naive_local().date().fmt(f)?;
        //        f.write_str(" ")?;
        //    }
        //    stop_time.naive_local().format("%H:%M:%S").fmt(f)?;
        //    f.write_str("  ")?;
        //    let duration = stop_time - self.start;
        //    if duration.num_hours() != 0 {
        //        f.write_fmt(format_args!("{}h ", duration.num_hours()))?;
        //    }
        //    if duration.num_minutes() % 60 != 0 || duration.num_hours() != 0 {
        //        f.write_fmt(format_args!("{}m ", duration.num_minutes() % 60))?;
        //    }
        //    f.write_fmt(format_args!("{}s", duration.num_seconds() % 60))?;
        //}

        //f.write_str("  ")?;
        //f.write_str(&self.name)?;
        Ok(())
    }
}

// TODO remove and use on argument config per function
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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS task_events (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                session TEXT NOT NULL,
                state TEXT NOT NULL,
                time DATETIME NOT NULL
            )",
            [],
        )
        .expect("could not start database connection");
        Self { conn }
    }
}

impl ShiftDb {
    fn ongoing_sessions(&self) -> Vec<TaskSession> {
        let query = "SELECT * FROM task_events event
            WHERE NOT EXISTS (
                SELECT 1 FROM task_events
                WHERE session == event.session
                AND state == 'Stopped'
            )
            ORDER BY datetime(time) DESC";
        let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
        let mut events = stmt
            .query_map([], |row| TaskEvent::try_from(row))
            .expect("No parameters should always bind correctly")
            .map(|e| e.unwrap())
            .collect::<Vec<TaskEvent>>();

        let mut session_events = HashMap::<(String, String), Vec<TaskEvent>>::new();
        while let Some(event) = events.pop() {
            if let Some(event_vec) =
                session_events.get_mut(&(event.name.to_string(), event.session.to_string()))
            {
                event_vec.push(event);
            } else {
                session_events.insert(
                    (event.name.to_string(), event.session.to_string()),
                    vec![event],
                );
            }
        }
        let mut sessions = session_events
            .into_iter()
            .map(|((name, session), events)| TaskSession {
                id: Uuid::from_str(&session).expect("Could not deserialize id as an uuid"),
                name,
                events,
            })
            .collect::<Vec<TaskSession>>();
        sessions.sort_by(|sa, sb| {
            sa.events
                .last()
                .unwrap()
                .time
                .cmp(&sb.events.last().unwrap().time)
        });
        sessions
    }
}

#[cfg(test)]
mod test {
    use crate::{
        commands::{start, stop},
        Config, ShiftDb,
    };

    #[test]
    fn get_ongoing() {
        let s = ShiftDb::new("");
        let config = Config {
            uid: Some("task1".to_string()),
            ..Default::default()
        };
        start::start(&s, &config).unwrap();

        let config = Config {
            uid: Some("task2".to_string()),
            ..Default::default()
        };
        start::start(&s, &config).unwrap();

        stop::stop(&s, &config).unwrap();

        let tasks = s.ongoing_sessions();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks.get(0).unwrap().name, "task1");
    }
}
