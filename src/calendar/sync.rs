use rusqlite::Connection;

use crate::calendar::eventkit::EventKitStore;
use crate::db::queries;

/// Discover calendars from EventKit and upsert them into the DB.
/// New calendars default to enabled; existing calendars keep their enabled state.
pub fn sync_calendars(conn: &Connection, store: &EventKitStore) -> Result<usize, Box<dyn std::error::Error>> {
    let calendars = store.fetch_calendars();
    let count = calendars.len();

    for cal in &calendars {
        // Check if calendar already exists to preserve enabled state
        let existing = queries::get_all_calendars(conn)?;
        let already_exists = existing.iter().any(|c| c.id == cal.id);
        if !already_exists {
            if let Err(e) = queries::upsert_calendar(conn, cal) {
                tracing::warn!("Failed to upsert calendar '{}': {e}", cal.id);
            }
        } else {
            // Update title/color but preserve enabled state
            if let Err(e) = queries::upsert_calendar(conn, cal) {
                tracing::warn!("Failed to update calendar '{}': {e}", cal.id);
            }
        }
    }

    Ok(count)
}

/// Sync events from EventKit to SQLite, filtered by enabled calendars.
/// Returns the number of events synced.
pub fn sync_events(conn: &Connection, store: &EventKitStore, from: i64, to: i64) -> Result<usize, Box<dyn std::error::Error>> {
    let enabled_ids = queries::get_enabled_calendar_ids(conn)?;
    let events = store.fetch_events(from, to, &enabled_ids)?;
    let count = events.len();

    for event in &events {
        if let Err(e) = queries::upsert_event(conn, event) {
            tracing::warn!("Failed to upsert event '{}': {e}", event.id);
        }
    }

    let one_hour_ago = from - 3600;
    if let Err(e) = queries::delete_stale_events(conn, one_hour_ago) {
        tracing::warn!("Failed to delete stale events: {e}");
    }

    Ok(count)
}
