# Architecture

Never Forget is a native macOS menu bar app written in Rust that surfaces calendar events as a fullscreen, hard-to-miss overlay shortly before they begin. It reads from the system Calendar via EventKit, caches data in a local SQLite database, and renders the overlay with [`iced`](https://github.com/iced-rs/iced).

## High-level flow

```
EventKit (macOS) ──► sync ──► SQLite ──► notification query ──► iced overlay
        │                                                            │
        └── change notification ──► reactive resync                  └── Join / Dismiss / Snooze ──► event_state
```

A single iced `daemon` owns the entire app state. A 1 Hz `Tick` drives the trigger check and tray polling; a configurable poll interval (default 30s) drives calendar sync. EventKit change notifications trigger an immediate resync so external edits propagate without waiting for the next tick.

## Module layout (`src/`)

| Module | Responsibility |
|---|---|
| `main.rs` | Entry point. Initializes tracing and hands off to `app::run()`. |
| `app.rs` | iced daemon: state, `Message` enum, `update`/`view`/`subscription`, and multi-monitor overlay window management. |
| `calendar/eventkit.rs` | Safe wrapper around `EKEventStore`. Handles permission requests, calendar/event fetching, change-notification observer, and conversion of `EKEvent` → `CalendarEvent`. |
| `calendar/sync.rs` | Reconciles EventKit data into SQLite: upserts calendars (preserving the user's enabled flag) and events, and purges stale events. |
| `db/mod.rs` | Opens the SQLite connection at `~/Library/Application Support/neverforget/events.db` (WAL, foreign keys on); also exposes an in-memory connection for tests. |
| `db/schema.rs` | Versioned migration runner. Each `vN_*` function is appended to the `MIGRATIONS` slice and tracked in `schema_version`. |
| `db/queries.rs` | All SQL access: domain types (`CalendarEvent`, `Calendar`, `EventState`), upserts, lookups, and per-event state mutations. |
| `notifications.rs` | The trigger query: which events are due to fire *right now*, joining `events`, `event_state`, and `calendars` to filter dismissed/snoozed/disabled/declined events. |
| `overlay.rs` | iced view for the fullscreen overlay: title, time range, location (when set), countdown, Join/Dismiss buttons, and 1m/5m/until-event snooze row. |
| `blur.rs` | Inserts an `NSVisualEffectView` behind iced's content view via objc2 to give the overlay native macOS vibrancy without breaking the Metal renderer. |
| `tray.rs` | Menu bar icon and dropdown listing the next upcoming events plus a Quit item. Debug builds also expose a "Show Next Overlay" item that bypasses the notify window for verifying overlay UI changes. |
| `meeting_url.rs` | Extracts Zoom / Google Meet / Teams / Webex URLs from an event's location or notes via regex. |
| `settings.rs` | Typed wrapper over the `settings` key/value table (`notify_minutes_before`, `poll_interval_seconds`, `enabled`). |

## State and data model

Three SQLite tables, evolved through three migrations:

- **`events`** — cached calendar events keyed by EventKit identifier. Stores title, start/end, calendar metadata (id, title, color), location, notes, extracted `meeting_url`, `last_synced`, and `attendee_status` (raw `EKParticipantStatus`).
- **`event_state`** — per-event UI state (`dismissed_at`, `snoozed_until`). Foreign-keyed to `events` with `ON DELETE CASCADE`, so purging stale events also clears their state.
- **`calendars`** — list of available calendars with a user-controlled `enabled` flag. Sync upserts title/color but never overwrites `enabled`, so the user's selection survives resyncs.
- **`settings`** — flat key/value config, surfaced through the `Settings` struct.
- **`schema_version`** — append-only migration log.

In-memory app state (`App`) holds the DB connection, loaded `Settings`, optional `EventKitStore` (absent if access was denied), the `Tray`, the open overlay window IDs (one per screen), and the currently shown event.

## The notification trigger

`notifications::get_events_to_notify` is the single source of truth for "should we show an overlay right now?". An event qualifies when:

1. `start_time` falls inside `[now, now + notify_seconds_before]`.
2. Its calendar is enabled.
3. The current user has accepted it, or has no attendee status (i.e. a personal event with no invitee list). Declined / tentative / pending events are filtered out.
4. It has no `event_state` row, or the row has neither a `dismissed_at` nor an active `snoozed_until > now`.

The `Tick` handler in `app.rs` runs this query every second when no overlay is open and shows the first match.

## Overlay rendering

When an event fires, `show_overlay` enumerates all `NSScreen`s, converts macOS bottom-left coords to iced's top-left, and opens one transparent, decoration-less, always-on-top window per screen. Each window gets a `NSVisualEffectView` inserted via `blur.rs` for the frosted background, then takes focus. The view shows the event card; keyboard shortcuts are wired in `update`:

- `Enter` → join meeting if a URL was extracted, else dismiss
- `Escape` → dismiss
- `1` / `5` → snooze for 1 or 5 minutes

Closing the overlay drains all window IDs and closes them in a batch task.

## Sync loop

`Message::SyncCalendar`, fired once at startup and then on the poll-interval timer:

1. `sync_calendars` upserts every EventKit calendar (preserving `enabled`).
2. `sync_events` fetches events in `[now - 5m, now + 24h]` filtered to enabled calendars, upserts them, and deletes events whose `end_time` is older than 1h before the window start.
3. The next 10 events are pushed into the tray menu.

In addition, `EventKitStore` registers an `EKEventStoreChangedNotification` observer that flips an `AtomicBool`. Every tick checks/clears that flag and dispatches a `SyncCalendar` immediately, so changes made in Calendar.app appear without waiting for the poll.

## Permissions and platform integration

- Calendar access is requested on first launch via `requestFullAccessToEventsWithCompletion`. If denied, the app still runs but `App.store` is `None` and no syncing happens.
- All EventKit, AppKit, and notification-center calls go through `objc2` / `objc2-foundation` / `objc2-app-kit` / `objc2-event-kit` bindings.
- The tray icon is generated procedurally (a 16×16 orange disc) via `tray-icon` + `muda`.

## Testing

Each module ships with `#[cfg(test)]` unit tests against an in-memory SQLite database (`db::open_in_memory`). Coverage focuses on the parts where bugs would actually hurt: migrations (idempotency, incremental application), the notification filter (dismissed / snoozed / disabled-calendar / attendee-status combinations), URL extraction across the four supported providers, calendar enable-flag preservation across resyncs, and the countdown formatter's edge cases around the start time.
