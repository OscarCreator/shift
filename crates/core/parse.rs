use chrono::{offset::LocalResult, DateTime, Local, NaiveDateTime, NaiveTime, TimeZone};

pub fn to_date(s: &str) -> anyhow::Result<DateTime<Local>> {
    let time_formats = vec!["%H:%M", "%H:%M:%S"];
    for f in time_formats {
        if let Ok(nt) = NaiveTime::parse_from_str(s, f) {
            if let LocalResult::Single(d) = Local::now().with_time(nt) {
                return Ok(d);
            }
        }
    }
    let date_formats = vec!["%Y-%m-%d %H:%M", "%Y-%m-%d %H:%M:%S"];
    for f in date_formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            if let LocalResult::Single(d) = Local.from_local_datetime(&dt) {
                return Ok(d);
            }
        }
    }

    Err(anyhow::anyhow!("could not parse time"))
}
