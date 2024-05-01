use rusqlite::{params, Row};

use crate::{Config, ShiftDb, Task};

/// Retrieve the tasks from the database
pub fn tasks(s: &ShiftDb, args: &Config) -> anyhow::Result<Vec<Task>> {
    let row_to_task = |row: &Row<'_>| Task::try_from(row);
    let mut stmt;
    let task_iter = match (args.to, args.from) {
        (Some(to_date), Some(from_date)) => {
            let query =
                "SELECT * FROM tasks WHERE start > ? and start < ? ORDER BY start DESC LIMIT ?";
            stmt = s.conn.prepare(query)?;
            if args.all || !args.tasks.is_empty() {
                stmt.query_map(params![from_date, to_date, -1], row_to_task)?
            } else {
                stmt.query_map(params![from_date, to_date, args.count], row_to_task)?
            }
        }
        (None, Some(from_date)) => {
            let query = "SELECT * FROM tasks WHERE start > ? ORDER BY start DESC LIMIT ?";
            stmt = s.conn.prepare(query)?;
            if args.all || !args.tasks.is_empty() {
                stmt.query_map(params![from_date, -1], row_to_task)?
            } else {
                stmt.query_map(params![from_date, args.count], row_to_task)?
            }
        }
        (Some(to_date), None) => {
            let query = "SELECT * FROM tasks WHERE start < ? ORDER BY start DESC LIMIT ?";
            stmt = s.conn.prepare(query)?;
            if args.all || !args.tasks.is_empty() {
                stmt.query_map(params![to_date, -1], row_to_task)?
            } else {
                stmt.query_map(params![to_date, args.count], row_to_task)?
            }
        }
        (None, None) => {
            let query = "SELECT * FROM tasks ORDER BY start DESC LIMIT ?";
            stmt = s.conn.prepare(query)?;
            if args.all || !args.tasks.is_empty() {
                stmt.query_map([-1], row_to_task)?
            } else {
                stmt.query_map([args.count], row_to_task)?
            }
        }
    };

    let iter = task_iter.flatten();
    let res = if !args.tasks.is_empty() {
        let filtered = iter.filter(|t| args.tasks.contains(&t.name));
        if args.all {
            filtered.collect::<Vec<Task>>()
        } else {
            filtered.take(args.count).collect::<Vec<Task>>()
        }
    } else {
        iter.collect::<Vec<Task>>()
    };

    Ok(res)
}

#[cfg(test)]
mod test {
    use crate::{
        commands::{tasks::tasks, test::start_with_name},
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

        let tasks = tasks(&s, &config);
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
        let tasks = tasks(&s, &config);
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
        let tasks = tasks(&s, &config);
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
        let tasks = tasks(&s, &config).expect("Should get task1 and task2");

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
        let tasks = tasks(&s, &config).expect("Should get task1 and task2");

        assert_eq!(tasks.len(), 3);
        assert_eq!(
            tasks.iter().map(|t| &t.name).collect::<Vec<&String>>(),
            vec!["task4", "task3", "task2"]
        )
    }
}
