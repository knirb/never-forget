use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub calendar_title: Option<String>,
    pub calendar_color: Option<String>,
    pub title: String,
    pub start_time: i64,
    pub end_time: i64,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub meeting_url: Option<String>,
    pub last_synced: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calendar {
    pub id: String,
    pub title: String,
    pub color: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventState {
    pub event_id: String,
    pub dismissed_at: Option<i64>,
    pub snoozed_until: Option<i64>,
}

pub fn upsert_event(conn: &Connection, event: &CalendarEvent) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO events (id, calendar_id, calendar_title, calendar_color, title, start_time, end_time, location, notes, meeting_url, last_synced)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            calendar_id = excluded.calendar_id,
            calendar_title = excluded.calendar_title,
            calendar_color = excluded.calendar_color,
            title = excluded.title,
            start_time = excluded.start_time,
            end_time = excluded.end_time,
            location = excluded.location,
            notes = excluded.notes,
            meeting_url = excluded.meeting_url,
            last_synced = excluded.last_synced",
        params![
            event.id,
            event.calendar_id,
            event.calendar_title,
            event.calendar_color,
            event.title,
            event.start_time,
            event.end_time,
            event.location,
            event.notes,
            event.meeting_url,
            event.last_synced,
        ],
    )?;
    Ok(())
}

pub fn get_upcoming_events(conn: &Connection, now: i64, within_seconds: i64) -> rusqlite::Result<Vec<CalendarEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, calendar_id, calendar_title, calendar_color, title, start_time, end_time, location, notes, meeting_url, last_synced
         FROM events
         WHERE start_time >= ?1 AND start_time <= ?2
         ORDER BY start_time ASC",
    )?;
    let events = stmt
        .query_map(params![now, now + within_seconds], |row| {
            Ok(CalendarEvent {
                id: row.get(0)?,
                calendar_id: row.get(1)?,
                calendar_title: row.get(2)?,
                calendar_color: row.get(3)?,
                title: row.get(4)?,
                start_time: row.get(5)?,
                end_time: row.get(6)?,
                location: row.get(7)?,
                notes: row.get(8)?,
                meeting_url: row.get(9)?,
                last_synced: row.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(events)
}

pub fn get_next_events(conn: &Connection, now: i64, limit: usize) -> rusqlite::Result<Vec<CalendarEvent>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.calendar_id, e.calendar_title, e.calendar_color, e.title,
                e.start_time, e.end_time, e.location, e.notes, e.meeting_url, e.last_synced
         FROM events e
         INNER JOIN calendars c ON e.calendar_id = c.id AND c.enabled = 1
         WHERE e.start_time >= ?1
         ORDER BY e.start_time ASC
         LIMIT ?2",
    )?;
    let events = stmt
        .query_map(params![now, limit as i64], |row| {
            Ok(CalendarEvent {
                id: row.get(0)?,
                calendar_id: row.get(1)?,
                calendar_title: row.get(2)?,
                calendar_color: row.get(3)?,
                title: row.get(4)?,
                start_time: row.get(5)?,
                end_time: row.get(6)?,
                location: row.get(7)?,
                notes: row.get(8)?,
                meeting_url: row.get(9)?,
                last_synced: row.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(events)
}

pub fn get_event_state(conn: &Connection, event_id: &str) -> rusqlite::Result<Option<EventState>> {
    conn.query_row(
        "SELECT event_id, dismissed_at, snoozed_until FROM event_state WHERE event_id = ?1",
        params![event_id],
        |row| {
            Ok(EventState {
                event_id: row.get(0)?,
                dismissed_at: row.get(1)?,
                snoozed_until: row.get(2)?,
            })
        },
    )
    .optional()
}

pub fn set_dismissed(conn: &Connection, event_id: &str, now: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO event_state (event_id, dismissed_at, snoozed_until)
         VALUES (?1, ?2, NULL)
         ON CONFLICT(event_id) DO UPDATE SET dismissed_at = excluded.dismissed_at, snoozed_until = NULL",
        params![event_id, now],
    )?;
    Ok(())
}

pub fn set_snoozed(conn: &Connection, event_id: &str, until: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO event_state (event_id, dismissed_at, snoozed_until)
         VALUES (?1, NULL, ?2)
         ON CONFLICT(event_id) DO UPDATE SET snoozed_until = excluded.snoozed_until",
        params![event_id, until],
    )?;
    Ok(())
}

pub fn clear_event_state(conn: &Connection, event_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM event_state WHERE event_id = ?1", params![event_id])?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn upsert_calendar(conn: &Connection, cal: &Calendar) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO calendars (id, title, color, enabled)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            color = excluded.color",
        params![cal.id, cal.title, cal.color, cal.enabled as i32],
    )?;
    Ok(())
}

pub fn get_all_calendars(conn: &Connection) -> rusqlite::Result<Vec<Calendar>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, color, enabled FROM calendars ORDER BY title ASC",
    )?;
    let cals = stmt
        .query_map([], |row| {
            Ok(Calendar {
                id: row.get(0)?,
                title: row.get(1)?,
                color: row.get(2)?,
                enabled: row.get::<_, i32>(3)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(cals)
}

pub fn get_enabled_calendar_ids(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM calendars WHERE enabled = 1",
    )?;
    let ids = stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

pub fn set_calendar_enabled(conn: &Connection, calendar_id: &str, enabled: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE calendars SET enabled = ?1 WHERE id = ?2",
        params![enabled as i32, calendar_id],
    )?;
    Ok(())
}

pub fn delete_stale_events(conn: &Connection, before_timestamp: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM events WHERE end_time < ?1",
        params![before_timestamp],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        crate::db::open_in_memory().unwrap()
    }

    fn make_event(id: &str, title: &str, start: i64, end: i64) -> CalendarEvent {
        CalendarEvent {
            id: id.to_string(),
            calendar_id: "cal-1".to_string(),
            calendar_title: Some("Work".to_string()),
            calendar_color: Some("#FF5733".to_string()),
            title: title.to_string(),
            start_time: start,
            end_time: end,
            location: None,
            notes: None,
            meeting_url: None,
            last_synced: 100,
        }
    }

    #[test]
    fn test_upsert_and_query_event() {
        let conn = setup();
        let event = make_event("ev-1", "Standup", 1000, 2000);
        upsert_event(&conn, &event).unwrap();

        let events = get_upcoming_events(&conn, 500, 1000).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Standup");
        assert_eq!(events[0].calendar_color, Some("#FF5733".to_string()));
    }

    #[test]
    fn test_upsert_updates_existing() {
        let conn = setup();
        let mut event = make_event("ev-1", "Standup", 1000, 2000);
        upsert_event(&conn, &event).unwrap();

        event.title = "Updated Standup".to_string();
        upsert_event(&conn, &event).unwrap();

        let events = get_upcoming_events(&conn, 500, 1000).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Updated Standup");
    }

    #[test]
    fn test_upcoming_events_filters_by_time() {
        let conn = setup();
        upsert_event(&conn, &make_event("ev-1", "Soon", 1000, 1500)).unwrap();
        upsert_event(&conn, &make_event("ev-2", "Later", 5000, 6000)).unwrap();
        upsert_event(&conn, &make_event("ev-3", "Past", 100, 200)).unwrap();

        let events = get_upcoming_events(&conn, 900, 200).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Soon");
    }

    #[test]
    fn test_dismiss_event() {
        let conn = setup();
        upsert_event(&conn, &make_event("ev-1", "Test", 1000, 2000)).unwrap();
        set_dismissed(&conn, "ev-1", 900).unwrap();

        let state = get_event_state(&conn, "ev-1").unwrap().unwrap();
        assert_eq!(state.dismissed_at, Some(900));
        assert_eq!(state.snoozed_until, None);
    }

    #[test]
    fn test_snooze_event() {
        let conn = setup();
        upsert_event(&conn, &make_event("ev-1", "Test", 1000, 2000)).unwrap();
        set_snoozed(&conn, "ev-1", 950).unwrap();

        let state = get_event_state(&conn, "ev-1").unwrap().unwrap();
        assert_eq!(state.snoozed_until, Some(950));
        assert_eq!(state.dismissed_at, None);
    }

    #[test]
    fn test_dismiss_clears_snooze() {
        let conn = setup();
        upsert_event(&conn, &make_event("ev-1", "Test", 1000, 2000)).unwrap();
        set_snoozed(&conn, "ev-1", 950).unwrap();
        set_dismissed(&conn, "ev-1", 960).unwrap();

        let state = get_event_state(&conn, "ev-1").unwrap().unwrap();
        assert_eq!(state.dismissed_at, Some(960));
        assert_eq!(state.snoozed_until, None);
    }

    #[test]
    fn test_clear_event_state() {
        let conn = setup();
        upsert_event(&conn, &make_event("ev-1", "Test", 1000, 2000)).unwrap();
        set_dismissed(&conn, "ev-1", 900).unwrap();
        clear_event_state(&conn, "ev-1").unwrap();

        let state = get_event_state(&conn, "ev-1").unwrap();
        assert!(state.is_none());
    }

    #[test]
    fn test_no_event_state_returns_none() {
        let conn = setup();
        let state = get_event_state(&conn, "nonexistent").unwrap();
        assert!(state.is_none());
    }

    #[test]
    fn test_settings_crud() {
        let conn = setup();
        let val = get_setting(&conn, "notify_minutes_before").unwrap().unwrap();
        assert_eq!(val, "1");

        set_setting(&conn, "notify_minutes_before", "5").unwrap();
        let val = get_setting(&conn, "notify_minutes_before").unwrap().unwrap();
        assert_eq!(val, "5");
    }

    #[test]
    fn test_get_nonexistent_setting() {
        let conn = setup();
        let val = get_setting(&conn, "nonexistent").unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn test_delete_stale_events() {
        let conn = setup();
        upsert_event(&conn, &make_event("ev-1", "Old", 100, 200)).unwrap();
        upsert_event(&conn, &make_event("ev-2", "Current", 1000, 2000)).unwrap();

        let deleted = delete_stale_events(&conn, 500).unwrap();
        assert_eq!(deleted, 1);

        let events = get_upcoming_events(&conn, 0, 10000).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Current");
    }

    fn make_calendar(id: &str, title: &str, enabled: bool) -> Calendar {
        Calendar {
            id: id.to_string(),
            title: title.to_string(),
            color: Some("#FF0000".to_string()),
            enabled,
        }
    }

    #[test]
    fn test_upsert_and_list_calendars() {
        let conn = setup();
        upsert_calendar(&conn, &make_calendar("cal-1", "Work", true)).unwrap();
        upsert_calendar(&conn, &make_calendar("cal-2", "Personal", true)).unwrap();

        let cals = get_all_calendars(&conn).unwrap();
        assert_eq!(cals.len(), 2);
        let titles: Vec<&str> = cals.iter().map(|c| c.title.as_str()).collect();
        assert!(titles.contains(&"Work"));
        assert!(titles.contains(&"Personal"));
    }

    #[test]
    fn test_upsert_calendar_preserves_enabled_on_update() {
        let conn = setup();
        upsert_calendar(&conn, &make_calendar("cal-1", "Work", true)).unwrap();
        set_calendar_enabled(&conn, "cal-1", false).unwrap();

        // Re-upsert with title change — enabled should stay false
        // because upsert_calendar only updates title and color
        upsert_calendar(&conn, &make_calendar("cal-1", "Work Updated", true)).unwrap();
        let cals = get_all_calendars(&conn).unwrap();
        assert_eq!(cals[0].title, "Work Updated");
        assert!(!cals[0].enabled);
    }

    #[test]
    fn test_get_enabled_calendar_ids() {
        let conn = setup();
        upsert_calendar(&conn, &make_calendar("cal-1", "Work", true)).unwrap();
        upsert_calendar(&conn, &make_calendar("cal-2", "Personal", true)).unwrap();
        set_calendar_enabled(&conn, "cal-2", false).unwrap();

        let ids = get_enabled_calendar_ids(&conn).unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], "cal-1");
    }

    #[test]
    fn test_set_calendar_enabled() {
        let conn = setup();
        upsert_calendar(&conn, &make_calendar("cal-1", "Work", true)).unwrap();

        set_calendar_enabled(&conn, "cal-1", false).unwrap();
        let cals = get_all_calendars(&conn).unwrap();
        assert!(!cals[0].enabled);

        set_calendar_enabled(&conn, "cal-1", true).unwrap();
        let cals = get_all_calendars(&conn).unwrap();
        assert!(cals[0].enabled);
    }
}
