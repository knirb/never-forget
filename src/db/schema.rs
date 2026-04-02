use rusqlite::Connection;

/// Each migration is a function that takes a connection and applies one schema change.
/// Migrations are ordered and each runs exactly once, tracked by schema_version.
const MIGRATIONS: &[fn(&Connection) -> rusqlite::Result<()>] = &[
    v1_initial,
    v2_add_calendars,
];

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );",
    )?;

    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current {
            migration(conn)?;
            conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
            tracing::info!("Applied migration v{version}");
        }
    }

    Ok(())
}

fn v1_initial(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE events (
            id TEXT PRIMARY KEY,
            calendar_id TEXT NOT NULL,
            calendar_title TEXT,
            calendar_color TEXT,
            title TEXT NOT NULL,
            start_time INTEGER NOT NULL,
            end_time INTEGER NOT NULL,
            location TEXT,
            notes TEXT,
            meeting_url TEXT,
            last_synced INTEGER NOT NULL
        );
        CREATE INDEX idx_events_start ON events(start_time);

        CREATE TABLE event_state (
            event_id TEXT PRIMARY KEY,
            dismissed_at INTEGER,
            snoozed_until INTEGER,
            FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
        );

        CREATE TABLE settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        INSERT INTO settings (key, value) VALUES ('notify_minutes_before', '1');
        INSERT INTO settings (key, value) VALUES ('poll_interval_seconds', '30');
        INSERT INTO settings (key, value) VALUES ('enabled', 'true');
        ",
    )
}

fn v2_add_calendars(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE calendars (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            color TEXT,
            enabled INTEGER NOT NULL DEFAULT 1
        );
        ",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_run_without_error() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();

        // Should still have the same version
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
    }

    #[test]
    fn test_version_tracking() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let max_version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(max_version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn test_incremental_migration() {
        let conn = Connection::open_in_memory().unwrap();

        // Simulate running only v1 initially
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
        ).unwrap();
        v1_initial(&conn).unwrap();
        conn.execute("INSERT INTO schema_version (version) VALUES (1)", []).unwrap();

        // Now run all migrations — should only apply v2
        run_migrations(&conn).unwrap();

        let max_version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(max_version, 2);

        // calendars table should exist
        conn.execute(
            "INSERT INTO calendars (id, title, enabled) VALUES ('cal-1', 'Test', 1)",
            [],
        ).unwrap();
    }

    #[test]
    fn test_default_settings_inserted() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let val: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'notify_minutes_before'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(val, "1");

        let val: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'poll_interval_seconds'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(val, "30");
    }

    #[test]
    fn test_events_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO events (id, calendar_id, calendar_title, calendar_color, title, start_time, end_time, location, notes, meeting_url, last_synced)
             VALUES ('test-1', 'cal-1', 'Work', '#FF0000', 'Test Event', 1000, 2000, NULL, NULL, NULL, 999)",
            [],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_calendars_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO calendars (id, title, color, enabled) VALUES ('cal-1', 'Work', '#FF0000', 1)",
            [],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM calendars", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_event_state_foreign_key() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO events (id, calendar_id, title, start_time, end_time, last_synced)
             VALUES ('ev-1', 'cal-1', 'Test', 1000, 2000, 999)",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO event_state (event_id, dismissed_at, snoozed_until) VALUES ('ev-1', 500, NULL)",
            [],
        )
        .unwrap();

        let dismissed: i64 = conn
            .query_row(
                "SELECT dismissed_at FROM event_state WHERE event_id = 'ev-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dismissed, 500);
    }
}
