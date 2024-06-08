use std::{collections::HashMap, str::FromStr};

use rusqlite::{params, Row};
use uuid::Uuid;

use crate::{Config, ShiftDb, TaskEvent, TaskSession};

/// Retrieve the tasks from the database
// TODO change return type from Vec to IntoIterator
pub fn sessions(s: &ShiftDb, args: &Config) -> anyhow::Result<Vec<TaskSession>> {
    let row_to_events = |row: &Row<'_>| TaskEvent::try_from(row);
    let mut stmt;
    let events = match (args.to, args.from) {
        (Some(to_date), Some(from_date)) => {
            let query =
                "SELECT * FROM task_events WHERE time > ?1 and time < ?2 ORDER BY time DESC";
            stmt = s.conn.prepare(query)?;
            if args.all || !args.tasks.is_empty() {
                stmt.query_map(params![from_date, to_date], row_to_events)?
            } else {
                stmt.query_map(params![from_date, to_date], row_to_events)?
            }
        }
        (None, Some(from_date)) => {
            let query = "SELECT * FROM task_events WHERE time > ? ORDER BY time DESC";
            stmt = s.conn.prepare(query)?;
            if args.all || !args.tasks.is_empty() {
                stmt.query_map(params![from_date], row_to_events)?
            } else {
                stmt.query_map(params![from_date], row_to_events)?
            }
        }
        (Some(to_date), None) => {
            let query = "SELECT * FROM task_events WHERE time < ? ORDER BY time DESC";
            stmt = s.conn.prepare(query)?;
            if args.all || !args.tasks.is_empty() {
                stmt.query_map(params![to_date], row_to_events)?
            } else {
                stmt.query_map(params![to_date], row_to_events)?
            }
        }
        (None, None) => {
            let query = "SELECT * FROM task_events ORDER BY time DESC";
            stmt = s.conn.prepare(query)?;
            stmt.query_map([], row_to_events)?
        }
    };

    // get events for all those sessions and insert them into the sesssion structs
    let mut session_map = HashMap::<(String, String), Vec<TaskEvent>>::new();
    for e in events {
        let event = e.expect("Database corrupt, could not parse event from database");
        if let Some(session_events) =
            session_map.get_mut(&(event.name.to_string(), event.session.to_string()))
        {
            session_events.push(event);
        } else {
            session_map.insert(
                (event.name.to_string(), event.session.to_string()),
                vec![event],
            );
        }
    }
    let mut iter = session_map
        .into_iter()
        .map(|((name, id), events)| TaskSession {
            id: Uuid::from_str(&id).expect("Could not deserialize id as an uuid"),
            name,
            events,
        })
        .collect::<Vec<TaskSession>>();
    iter.sort_by(|sa, sb| {
        sb.events
            .first()
            .unwrap()
            .time
            .cmp(&sa.events.first().unwrap().time)
    });

    let res = if !args.tasks.is_empty() {
        let filtered = iter.into_iter().filter(|t| args.tasks.contains(&t.name));
        if args.all {
            filtered.collect()
        } else {
            filtered.take(args.count).collect()
        }
    } else if args.all {
        iter
    } else {
        iter.into_iter().take(args.count).collect()
    };

    Ok(res)
}

#[cfg(test)]
mod test {
    use crate::{
        commands::{sessions::sessions, test::start_with_name},
        Config, ShiftDb,
    };

    #[test]
    fn count_limit() {
        let s = ShiftDb::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }
        let config = Config {
            count: 2,
            ..Default::default()
        };

        let tasks = sessions(&s, &config);
        assert_eq!(tasks.unwrap().len(), 2);
    }

    #[test]
    fn desc() {
        let s = ShiftDb::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 4,
            ..Default::default()
        };
        let tasks = sessions(&s, &config);
        assert_eq!(
            tasks
                .unwrap()
                .iter()
                .map(|t| &t.name)
                .collect::<Vec<&String>>(),
            vec!["task99", "task98", "task97", "task96"]
        );
    }

    #[test]
    fn all() {
        let s = ShiftDb::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 4,
            all: true,
            ..Default::default()
        };
        let tasks = sessions(&s, &config);
        assert_eq!(tasks.unwrap().len(), 100);
    }

    #[test]
    fn filter_by_names() {
        let s = ShiftDb::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 100,
            tasks: vec!["task1".to_string(), "task2".to_string()],
            ..Default::default()
        };
        let tasks = sessions(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 2);
        assert_eq!(
            tasks.iter().map(|t| &t.name).collect::<Vec<&String>>(),
            vec!["task2", "task1"]
        )
    }

    #[test]
    fn limit() {
        let s = ShiftDb::new("");

        for i in 0..100 {
            start_with_name(&s, &format!("task{}", i));
        }

        let config = Config {
            count: 3,
            tasks: vec![
                "task1".to_string(),
                "task2".to_string(),
                "task3".to_string(),
                "task4".to_string(),
            ],
            ..Default::default()
        };
        let tasks = sessions(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 3);
        assert_eq!(
            tasks.iter().map(|t| &t.name).collect::<Vec<&String>>(),
            vec!["task4", "task3", "task2"]
        )
    }
}
