use std::{collections::HashMap, fmt::Display, path::Path, str::FromStr};

use chrono::{DateTime, Local, TimeDelta};
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

// TODO should this be a pub(crate) type and then expose a type with only public fields?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    // TODO: have Uuid here as type
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) id: String,
    pub name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) session: String,
    pub state: TaskState,
    pub time: DateTime<Local>,
}

impl TaskEvent {
    fn new(
        name: String,
        session: Option<Uuid>,
        time: Option<DateTime<Local>>,
        state: TaskState,
    ) -> Self {
        let session_id = session.map_or(Uuid::now_v7(), |a| a);
        let time = time.map_or(Local::now(), |a| a);
        Self {
            id: Uuid::now_v7().to_string(),
            name,
            session: session_id.to_string(),
            state,
            time: time.into(),
        }
    }
}

impl Display for TaskEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.id.get(self.id.len() - 8..).expect(""),
            self.name,
            self.state,
            self.time
        )?;
        Ok(())
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
    /// Events starting from latest backwards in time to a start event
    pub events: Vec<TaskEvent>,
}

impl TaskSession {
    fn is_paused(&self) -> bool {
        if let Some(e) = self.events.first() {
            if e.state == TaskState::Paused {
                return true;
            }
        }
        false
    }

    fn state(&self) -> &TaskState {
        if let Some(e) = self.events.first() {
            &e.state
        } else {
            &TaskState::Stopped
        }
    }

    // TODO get all time diffs between events and then validate?
    fn get_times(&self) -> (TimeDelta, TimeDelta) {
        let mut elapsed = TimeDelta::zero();
        let mut pause_time = TimeDelta::zero();
        let mut previous: Option<&TaskEvent> = None;

        let mut events = self.events.clone();
        events.reverse();
        for e in &events {
            match e.state {
                TaskState::Started => {
                    // previous can be empty or pause
                    if let Some(p) = previous {
                        match p.state {
                            TaskState::Stopped => {
                                assert_eq!(
                                    self.events.len(),
                                    2,
                                    "Start + Stop event should be exactly two {:?}",
                                    &self
                                );
                                return (p.time.signed_duration_since(e.time), TimeDelta::zero());
                            }
                            TaskState::Paused => {
                                elapsed += p.time.signed_duration_since(e.time);
                            }
                            TaskState::Started => {
                                panic!("Found more than one start event in session: {:?}", &self)
                            }
                            TaskState::Resumed => panic!(
                                "Resume event not possible to be after start event: {:?}",
                                &self
                            ),
                        }
                    } else {
                        return (
                            Local::now().signed_duration_since(e.time),
                            TimeDelta::zero(),
                        );
                    }
                }
                TaskState::Stopped => {
                    assert_eq!(
                        previous, None,
                        "Found more than one stop event in session: {:?}",
                        &self
                    );
                }
                TaskState::Paused => {
                    if let Some(p) = previous {
                        // could be either started or previous pause
                        match p.state {
                            TaskState::Resumed => {
                                pause_time += p.time.signed_duration_since(e.time);
                            }
                            TaskState::Started => {
                                elapsed += p.time.signed_duration_since(e.time);
                            }
                            TaskState::Stopped => {
                                pause_time += p.time.signed_duration_since(e.time);
                            }
                            TaskState::Paused => {
                                panic!("Found two pause events after each other: {:?}", &self)
                            }
                        }
                    } else {
                        pause_time += Local::now().signed_duration_since(e.time);
                    }
                }
                TaskState::Resumed => {
                    if let Some(p) = previous {
                        assert_eq!(
                            p.state,
                            TaskState::Paused,
                            "Resume event only allowed after pause event: {p:?}"
                        );
                        // Pause time not added
                        pause_time += p.time.signed_duration_since(e.time);
                    } else {
                        // add from now to pause start
                        elapsed += Local::now().signed_duration_since(e.time);
                    }
                }
            }
            previous = Some(e);
        }
        (elapsed, pause_time)
    }
}

// TODO cli part should handle this?
impl Display for TaskSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let current_state = self.state();
        let (elapsed_time, pause_time) = self.get_times();
        write!(
            f,
            "{} {} {}h {}min elapsed",
            self.name,
            current_state,
            elapsed_time.num_hours(),
            elapsed_time.num_minutes() % 60
        )?;
        if !pause_time.is_zero() {
            write!(
                f,
                "\t{}h {}min paused",
                pause_time.num_hours(),
                pause_time.num_minutes() % 60
            )?;
        };
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
            ORDER BY time DESC";
        let mut stmt = self.conn.prepare(query).expect("SQL statement is valid");
        let events = stmt
            .query_map([], |row| TaskEvent::try_from(row))
            .expect("No parameters should always bind correctly")
            .map(|e| e.unwrap())
            .collect::<Vec<TaskEvent>>();

        let mut session_events = HashMap::<(String, String), Vec<TaskEvent>>::new();
        for event in events {
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
                .first()
                .unwrap()
                .time
                .cmp(&sb.events.first().unwrap().time)
        });
        sessions
    }
}

#[cfg(test)]
mod test {
    use crate::{
        commands::{
            start::{self, StartOpts},
            stop::{self, StopOpts},
        },
        ShiftDb,
    };

    #[test]
    fn get_ongoing() {
        let s = ShiftDb::new("");
        let config = StartOpts {
            uid: Some("task1".to_string()),
            ..Default::default()
        };
        start::start(&s, &config).unwrap();

        let config = StartOpts {
            uid: Some("task2".to_string()),
            ..Default::default()
        };
        start::start(&s, &config).unwrap();

        let config = StopOpts {
            uid: Some("task2".to_string()),
            ..Default::default()
        };
        stop::stop(&s, &config).unwrap();

        let tasks = s.ongoing_sessions();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks.get(0).unwrap().name, "task1");
    }
}
