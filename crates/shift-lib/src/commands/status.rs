use crate::{Config, ShiftDb};

// Get curret ongoing task(s)
pub fn status(s: &ShiftDb, _args: &Config) -> anyhow::Result<()> {
    for session in s.ongoing_sessions() {
        println!("{session}");
    }
    Ok(())
}
