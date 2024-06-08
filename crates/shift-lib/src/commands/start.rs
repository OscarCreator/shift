use std::{error::Error, fmt::Display};

use chrono::{DateTime, Local};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{ShiftDb, TaskEvent, TaskState};

#[derive(Debug)]
pub enum StartError {
    Ongoing(String),
    SqlError(String),
}

impl Error for StartError {}

impl Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StartOpts {
    pub uid: Option<String>,
    pub start_time: Option<DateTime<Local>>,
}

pub fn start(s: &ShiftDb, args: &StartOpts) -> Result<TaskEvent, StartError> {
    let name = args.uid.clone().expect("Required to specify task name");
    let ongoing = s.ongoing_sessions().into_iter().filter(|s| s.name == name);
    let mut event = TaskEvent::new(name.to_string(), None, None, TaskState::Started);
    if let Some(start_time) = args.start_time {
        event.time = start_time.into()
    }

    if ongoing.count() > 0 {
        return Err(StartError::Ongoing(event.name));
    }
    match s.conn.execute(
        "INSERT INTO task_events VALUES (?1, ?2, ?3, ?4, ?5);",
        params![event.id, event.name, event.session, event.state, event.time],
    ) {
        Ok(1) => Ok(event),
        Ok(u) => Err(StartError::SqlError(format!(
            "Inserted {} tasks when only expected 1",
            u
        ))),
        Err(e) => Err(StartError::SqlError(e.to_string())),
    }
}

#[cfg(test)]
mod test {
    use chrono::Local;

    use crate::commands::sessions::sessions;
    use crate::{commands::start::StartOpts, Config, ShiftDb};

    use super::start;

    #[test]
    fn start_time() {
        let s = ShiftDb::new("");

        let time = Local::now();
        let config = StartOpts {
            uid: Some("task1".to_string()),
            start_time: Some(time),
            ..Default::default()
        };
        start(&s, &config).unwrap();
        assert_eq!(s.ongoing_sessions().len(), 1);

        let config = Config {
            count: 50,
            ..Default::default()
        };
        let tasks = sessions(&s, &config);
        assert_eq!(
            tasks.unwrap()[0].events[0].time,
            time,
            "Start time not handled"
        );
    }
}
