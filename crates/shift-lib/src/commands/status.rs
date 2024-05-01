use crate::{Config, ShiftDb, Task};

// Get curret ongoing task(s)
pub fn status(s: &ShiftDb, _args: &Config) -> anyhow::Result<()> {
    let query = "SELECT * FROM tasks WHERE stop IS NULL";

    let mut stmt = s.conn.prepare(query)?;
    let task_iter = stmt.query_map([], |row| Task::try_from(row))?;
    task_iter.for_each(|t| {
        if let Ok(task) = t {
            println!("{task}");
        }
    });
    Ok(())
}
