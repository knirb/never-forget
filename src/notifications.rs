use crate::db::queries::{
    CalendarEvent, ATTENDEE_STATUS_ACCEPTED, ATTENDEE_STATUS_PENDING, ATTENDEE_STATUS_TENTATIVE,
    ATTENDEE_STATUS_UNKNOWN,
};
use rusqlite::{Connection, params};

pub fn get_events_to_notify(conn: &Connection, now: i64, notify_seconds_before: i64) -> rusqlite::Result<Vec<CalendarEvent>> {
    // Notify for events the user accepted, answered "maybe" to, or hasn't
    // responded to. NULL status means the copy doesn't know the user's
    // response (personal events, shared team calendars).
    //
    // The NOT EXISTS clause dedupes copies of the same meeting (same title
    // and start) across calendars: a copy that knows the user's response
    // outranks one that doesn't — so declining on your own calendar also
    // silences the shared-calendar copy — and equally-informed copies fall
    // back to the lowest id so only one notifies.
    let sql = format!(
        "SELECT e.id, e.calendar_id, e.calendar_title, e.calendar_color, e.title,
                e.start_time, e.end_time, e.location, e.notes, e.meeting_url, e.last_synced,
                e.attendee_status
         FROM events e
         LEFT JOIN event_state es ON e.id = es.event_id
         INNER JOIN calendars c ON e.calendar_id = c.id AND c.enabled = 1
         WHERE e.start_time >= ?1 AND e.start_time <= ?2
           AND (e.attendee_status IS NULL
                OR e.attendee_status IN ({unknown}, {pending}, {accepted}, {tentative}))
           AND NOT EXISTS (
                SELECT 1 FROM events dup
                INNER JOIN calendars dc ON dup.calendar_id = dc.id AND dc.enabled = 1
                WHERE dup.id != e.id
                  AND dup.title = e.title
                  AND dup.start_time = e.start_time
                  AND ((dup.attendee_status IS NOT NULL AND e.attendee_status IS NULL)
                       OR ((dup.attendee_status IS NULL) = (e.attendee_status IS NULL)
                           AND dup.id < e.id)))
           AND (es.event_id IS NULL
                OR (es.dismissed_at IS NULL
                    AND (es.snoozed_until IS NULL OR es.snoozed_until <= ?3)))
         ORDER BY e.start_time ASC",
        unknown = ATTENDEE_STATUS_UNKNOWN,
        pending = ATTENDEE_STATUS_PENDING,
        accepted = ATTENDEE_STATUS_ACCEPTED,
        tentative = ATTENDEE_STATUS_TENTATIVE,
    );
    let mut stmt = conn.prepare(&sql)?;
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
                attendee_status: row.get(11)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::queries::{upsert_event, upsert_calendar, set_dismissed, set_snoozed, Calendar, CalendarEvent, ATTENDEE_STATUS_DECLINED};

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
            attendee_status: None,
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

    fn make_event_with_status(id: &str, title: &str, start: i64, end: i64, status: Option<i64>) -> CalendarEvent {
        let mut event = make_event(id, title, start, end);
        event.attendee_status = status;
        event
    }

    #[test]
    fn test_accepted_event_returned() {
        let conn = setup_with_calendar();
        upsert_event(&conn, &make_event_with_status("ev-1", "Accepted", 1000, 1500, Some(ATTENDEE_STATUS_ACCEPTED))).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Accepted");
    }

    #[test]
    fn test_no_attendee_status_returned() {
        // Events without attendees (personal events) have null status — show them.
        let conn = setup_with_calendar();
        upsert_event(&conn, &make_event_with_status("ev-1", "Personal", 1000, 1500, None)).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Personal");
    }

    #[test]
    fn test_declined_event_filtered() {
        let conn = setup_with_calendar();
        upsert_event(&conn, &make_event_with_status("ev-1", "Declined", 1000, 1500, Some(ATTENDEE_STATUS_DECLINED))).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_tentative_event_returned() {
        let conn = setup_with_calendar();
        // EKParticipantStatusTentative = 4 ("maybe") — show it.
        upsert_event(&conn, &make_event_with_status("ev-1", "Tentative", 1000, 1500, Some(4))).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Tentative");
    }

    #[test]
    fn test_pending_event_returned() {
        let conn = setup_with_calendar();
        // EKParticipantStatusPending = 1 (not responded yet) — show it.
        upsert_event(&conn, &make_event_with_status("ev-1", "Pending", 1000, 1500, Some(1))).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Pending");
    }

    #[test]
    fn test_unknown_status_event_returned() {
        let conn = setup_with_calendar();
        // EKParticipantStatusUnknown = 0 — treat like not responded.
        upsert_event(&conn, &make_event_with_status("ev-1", "Unknown", 1000, 1500, Some(0))).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_delegated_event_filtered() {
        let conn = setup_with_calendar();
        // EKParticipantStatusDelegated = 5
        upsert_event(&conn, &make_event_with_status("ev-1", "Delegated", 1000, 1500, Some(5))).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert!(events.is_empty());
    }

    fn make_duplicate_on_calendar(id: &str, calendar_id: &str, title: &str, start: i64, end: i64, status: Option<i64>) -> CalendarEvent {
        let mut event = make_event_with_status(id, title, start, end, status);
        event.calendar_id = calendar_id.to_string();
        event
    }

    fn setup_with_shared_calendar() -> rusqlite::Connection {
        let conn = setup_with_calendar();
        upsert_calendar(&conn, &Calendar {
            id: "cal-shared".to_string(),
            title: "Team Shared".to_string(),
            color: None,
            enabled: true,
        }).unwrap();
        conn
    }

    #[test]
    fn test_declined_event_duplicate_on_shared_calendar_filtered() {
        // The same meeting often exists both on the user's own calendar (which
        // knows their response) and on a shared team calendar (which doesn't).
        // A decline on the informed copy must suppress the uninformed copy too.
        let conn = setup_with_shared_calendar();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-own", "cal-1", "Standup", 1000, 1500, Some(3))).unwrap();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-shared", "cal-shared", "Standup", 1000, 1500, None)).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_accepted_event_duplicate_notifies_once() {
        let conn = setup_with_shared_calendar();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-own", "cal-1", "Standup", 1000, 1500, Some(ATTENDEE_STATUS_ACCEPTED))).unwrap();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-shared", "cal-shared", "Standup", 1000, 1500, None)).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "ev-own");
    }

    #[test]
    fn test_status_less_duplicates_notify_once() {
        // Two copies from shared calendars, neither knowing the user's
        // response: only one should notify, and dismissing it silences both.
        let conn = setup_with_shared_calendar();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-a", "cal-1", "Standup", 1000, 1500, None)).unwrap();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-b", "cal-shared", "Standup", 1000, 1500, None)).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 1);

        set_dismissed(&conn, &events[0].id, 950).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_different_events_same_start_both_notify() {
        let conn = setup_with_shared_calendar();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-a", "cal-1", "Standup", 1000, 1500, None)).unwrap();
        upsert_event(&conn, &make_duplicate_on_calendar("ev-b", "cal-shared", "Other Meeting", 1000, 1500, None)).unwrap();
        let events = get_events_to_notify(&conn, 940, 120).unwrap();
        assert_eq!(events.len(), 2);
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
