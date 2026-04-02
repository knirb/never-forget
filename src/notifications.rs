use crate::db::queries::CalendarEvent;
use rusqlite::{Connection, params};

pub fn get_events_to_notify(conn: &Connection, now: i64, notify_seconds_before: i64) -> rusqlite::Result<Vec<CalendarEvent>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.calendar_id, e.calendar_title, e.calendar_color, e.title,
                e.start_time, e.end_time, e.location, e.notes, e.meeting_url, e.last_synced
         FROM events e
         LEFT JOIN event_state es ON e.id = es.event_id
         INNER JOIN calendars c ON e.calendar_id = c.id AND c.enabled = 1
         WHERE e.start_time >= ?1 AND e.start_time <= ?2
           AND (es.event_id IS NULL
                OR (es.dismissed_at IS NULL
                    AND (es.snoozed_until IS NULL OR es.snoozed_until <= ?3)))
         ORDER BY e.start_time ASC",
    )?;
    let events = stmt
        .query_map(params![now, now + notify_seconds_before, now], |row| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::queries::{upsert_event, upsert_calendar, set_dismissed, set_snoozed, Calendar, CalendarEvent};

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

    fn setup_with_calendar() -> rusqlite::Connection {
        let conn = db::open_in_memory().unwrap();
        upsert_calendar(&conn, &Calendar {
            id: "cal-1".to_string(),
            title: "Work".to_string(),
            color: Some("#FF5733".to_string()),
            enabled: true,
        }).unwrap();
        conn
    }

    #[test]
    fn test_no_events_returns_empty() {
        let conn = setup_with_calendar();
        let events = get_events_to_notify(&conn, 1000, 120).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_upcoming_event_returned() {
        let conn = setup_with_calendar();
        upsert_event(&conn, &make_event("ev-1", "Soon", 1000, 1500)).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Soon");
    }

    #[test]
    fn test_dismissed_event_filtered() {
        let conn = setup_with_calendar();
        upsert_event(&conn, &make_event("ev-1", "Dismissed", 1000, 1500)).unwrap();
        set_dismissed(&conn, "ev-1", 900).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_active_snooze_filtered() {
        let conn = setup_with_calendar();
        upsert_event(&conn, &make_event("ev-1", "Snoozed", 1000, 1500)).unwrap();
        set_snoozed(&conn, "ev-1", 2000).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_expired_snooze_returned() {
        let conn = setup_with_calendar();
        upsert_event(&conn, &make_event("ev-1", "Snooze Done", 1000, 1500)).unwrap();
        set_snoozed(&conn, "ev-1", 900).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Snooze Done");
    }

    #[test]
    fn test_disabled_calendar_events_filtered() {
        let conn = db::open_in_memory().unwrap();
        upsert_calendar(&conn, &Calendar {
            id: "cal-1".to_string(),
            title: "Work".to_string(),
            color: None,
            enabled: false,
        }).unwrap();
        upsert_event(&conn, &make_event("ev-1", "Hidden", 1000, 1500)).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_filters_correctly_mixed() {
        let conn = setup_with_calendar();

        upsert_event(&conn, &make_event("ev-1", "Soon", 1000, 1500)).unwrap();
        upsert_event(&conn, &make_event("ev-2", "Dismissed", 1050, 1600)).unwrap();
        set_dismissed(&conn, "ev-2", 900).unwrap();
        upsert_event(&conn, &make_event("ev-3", "Snoozed", 1020, 1520)).unwrap();
        set_snoozed(&conn, "ev-3", 2000).unwrap();
        upsert_event(&conn, &make_event("ev-4", "Snooze Done", 1030, 1530)).unwrap();
        set_snoozed(&conn, "ev-4", 900).unwrap();

        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        let titles: Vec<&str> = events.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.contains(&"Soon"));
        assert!(!titles.contains(&"Dismissed"));
        assert!(!titles.contains(&"Snoozed"));
        assert!(titles.contains(&"Snooze Done"));
    }
}
