use std::{error::Error, fmt::Display};

use rusqlite::params;

use crate::{Config, ShiftDb, Task};

#[derive(Debug)]
pub enum StartError {
    Ongoing(String),
    SqlError(String),
}

impl Error for StartError {}

impl Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("todo")
    }
}

pub fn start(s: &ShiftDb, args: &Config) -> Result<Task, StartError> {
    let mut task = Task::new(args.uid.clone().expect("Required to specify task name"));
    if let Some(start_time) = args.start_time {
        task.start = start_time.into()
    }

    match s.conn.execute(
        "INSERT INTO tasks 
             SELECT ?1, ?2, ?3, ?4
             WHERE NOT EXISTS(
                 SELECT * FROM tasks
                 WHERE name = ?2 AND stop IS NULL
             );",
        params![task.id.to_string(), task.name, task.start, task.stop],
    ) {
        Ok(1) => Ok(task),
        Ok(0) => Err(StartError::Ongoing(task.name)),
        Ok(u) => Err(StartError::SqlError(format!(
            "Inserted {} tasks when only expected 1",
            u
        ))),
        Err(e) => Err(StartError::SqlError(e.to_string())),
    }
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, Local};

    use crate::{commands::tasks::tasks, Config, ShiftDb, Task};

    use super::start;

    #[test]
    fn start_time() {
        let s = ShiftDb::new("");

        let time = DateTime::from(Local::now());
        let config = Config {
            uid: Some("task1".to_string()),
            start_time: Some(time),
            ..Default::default()
        };
        start(&s, &config).unwrap();

        let config = Config {
            count: 50,
            ..Default::default()
        };
        let tasks = tasks(&s, &config);
        assert_eq!(tasks.unwrap()[0].start, time, "Start time not handled");
    }
}
