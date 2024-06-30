use rusqlite::params;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ShiftDb, TaskEvent};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not find any event")]
    NoEventFound,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Opts {
    pub uid: Option<String>,
}

pub fn event(s: &ShiftDb, opts: &Opts) -> Result<TaskEvent, Error> {
    if let Some(uid) = &opts.uid {
        let query = "SELECT * FROM task_events WHERE id LIKE ?1 LIMIT 1";
        s.conn
            .query_row(&query, params![format!("%{uid}")], |row| {
                TaskEvent::try_from(row)
            })
            .map_or_else(
                |err| {
                    dbg!(err);
                    Err(Error::NoEventFound)
                },
                |e| Ok(e),
            )
    } else {
        let query = "SELECT * FROM task_events ORDER BY time DESC LIMIT 1";
        s.conn
            .query_row(&query, [], |row| TaskEvent::try_from(row))
            .map_or_else(|_| Err(Error::NoEventFound), |e| Ok(e))
    }
}

#[derive(Debug, Error)]
pub enum UpdateEventError {
    #[error("Could not update event with id {0}")]
    NotUpdated(String),
}

pub fn update(
    s: &ShiftDb,
    event: TaskEvent,
    updated_event: TaskEvent,
) -> Result<(), UpdateEventError> {
    let query = "UPDATE task_events SET name = ?1, state = ?2, time = ?3 WHERE id = ?4";
    match s
        .conn
        .execute(
            query,
            params![
                updated_event.name,
                updated_event.state,
                updated_event.time,
                event.id
            ],
        )
        .expect("SQL statement is valid")
    {
        0 => Err(UpdateEventError::NotUpdated(event.id)),

        1 => Ok(()),
        _ => unreachable!("id in task_events is primary key"),
    }
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, Local};

    use crate::commands::event::{update, Opts};
    use crate::commands::pause::{self};
    use crate::commands::test::start_with_name;
    use crate::{Config, ShiftDb, TaskEvent};

    use super::event;

    #[test]
    fn event_last() {
        let s = ShiftDb::new("");

        start_with_name(&s, "task1");
        let started_event = start_with_name(&s, "task2");

        let opts = Opts::default();

        let retreived_event = event(&s, &opts).expect("Should be able to get last event");
        assert_eq!(retreived_event, started_event);
    }

    #[test]
    fn event_and_update_by_uid() {
        let s = ShiftDb::new("");

        let started_event = start_with_name(&s, "task1");
        pause::pause(
            &s,
            &Config {
                ..Default::default()
            },
        )
        .unwrap();

        let opts = Opts {
            uid: Some(started_event.id.to_owned()),
        };

        let retreived_event = event(&s, &opts).expect("Should be able to get last event");
        assert_eq!(retreived_event, started_event);

        let new_event = TaskEvent {
            id: retreived_event.id.to_string(),
            name: retreived_event.name.to_string(),
            session: retreived_event.session.to_string(),
            state: retreived_event.state.clone(),
            time: Local::now(),
        };
        update(&s, retreived_event, new_event.clone()).unwrap();
        let updated = event(&s, &opts).expect("Should be able to get last event");
        assert_eq!(updated, new_event);
    }
}
