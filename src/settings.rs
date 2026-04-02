use rusqlite::Connection;

use crate::db::queries::{get_setting, set_setting};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub notify_minutes_before: u32,
    pub poll_interval_seconds: u32,
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notify_minutes_before: 1,
            poll_interval_seconds: 30,
            enabled: true,
        }
    }
}

impl Settings {
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let notify = get_setting(conn, "notify_minutes_before")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let poll = get_setting(conn, "poll_interval_seconds")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let enabled = get_setting(conn, "enabled")?
            .map(|v| v == "true")
            .unwrap_or(true);

        Ok(Self {
            notify_minutes_before: notify,
            poll_interval_seconds: poll,
            enabled,
        })
    }

    pub fn save(&self, conn: &Connection) -> rusqlite::Result<()> {
        set_setting(conn, "notify_minutes_before", &self.notify_minutes_before.to_string())?;
        set_setting(conn, "poll_interval_seconds", &self.poll_interval_seconds.to_string())?;
        set_setting(conn, "enabled", if self.enabled { "true" } else { "false" })?;
        Ok(())
    }

    pub fn notify_seconds_before(&self) -> i64 {
        self.notify_minutes_before as i64 * 60
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.notify_minutes_before, 1);
        assert_eq!(s.poll_interval_seconds, 30);
        assert!(s.enabled);
    }

    #[test]
    fn test_load_default_from_db() {
        let conn = db::open_in_memory().unwrap();
        let s = Settings::load(&conn).unwrap();
        assert_eq!(s.notify_minutes_before, 1);
        assert_eq!(s.poll_interval_seconds, 30);
        assert!(s.enabled);
    }

    #[test]
    fn test_save_and_load() {
        let conn = db::open_in_memory().unwrap();
        let s = Settings {
            notify_minutes_before: 5,
            poll_interval_seconds: 60,
            enabled: false,
        };
        s.save(&conn).unwrap();

        let loaded = Settings::load(&conn).unwrap();
        assert_eq!(loaded, s);
    }

    #[test]
    fn test_notify_seconds_before() {
        let s = Settings { notify_minutes_before: 3, ..Default::default() };
        assert_eq!(s.notify_seconds_before(), 180);
    }
}
