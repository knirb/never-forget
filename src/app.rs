use std::time::Duration;

use iced::window;
use iced::{Element, Point, Size, Subscription, Task, Theme};

use crate::calendar::eventkit::EventKitStore;
use crate::calendar::sync;
use crate::db;
use crate::db::queries::{self, CalendarEvent};
use crate::notifications;
use crate::overlay;
use crate::settings::Settings;
use crate::tray::Tray;

const SYNC_LOOKBACK_SECS: i64 = 300;
const SYNC_LOOKAHEAD_SECS: i64 = 86400;
const TRAY_EVENT_LIMIT: usize = 10;

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    SyncCalendar,
    Dismiss,
    JoinMeeting,
    Snooze(i64),
    SnoozeUntilEvent,
}

pub struct App {
    conn: rusqlite::Connection,
    settings: Settings,
    store: Option<EventKitStore>,
    tray: Tray,
    overlay_window_ids: Vec<window::Id>,
    current_event: Option<CalendarEvent>,
}

impl App {
    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    fn has_overlay(&self) -> bool {
        !self.overlay_window_ids.is_empty()
    }
}

pub fn run() -> iced::Result {
    iced::daemon("Never Forget", update, view)
        .subscription(subscription)
        .theme(|_state, _window| Theme::Dark)
        .run_with(|| {
            let conn = db::open_connection().expect("Failed to open database");
            let settings = Settings::load(&conn).expect("Failed to load settings");

            let store = EventKitStore::new();
            let access = store.request_access().unwrap_or(false);
            if !access {
                tracing::warn!("Calendar access not granted. Events won't sync.");
            }

            let tray = Tray::new();

            let app = App {
                conn,
                settings,
                store: if access { Some(store) } else { None },
                tray,
                overlay_window_ids: Vec::new(),
                current_event: None,
            };

            (app, Task::done(Message::SyncCalendar))
        })
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Tick => {
            if let Some(event) = crate::tray::poll_menu_event() {
                if event.id.0 == app.tray.quit_id() {
                    std::process::exit(0);
                }
            }

            if !app.has_overlay() {
                let now = App::now();
                let notify_window = app.settings.notify_seconds_before();
                if let Ok(events) = notifications::get_events_to_notify(&app.conn, now, notify_window) {
                    if let Some(event) = events.into_iter().next() {
                        return show_overlay(app, event);
                    }
                }
            }

            Task::none()
        }

        Message::SyncCalendar => {
            if let Some(store) = &app.store {
                if let Err(e) = sync::sync_calendars(&app.conn, store) {
                    tracing::error!("Failed to sync calendars: {e}");
                }

                let now = App::now();
                let from = now - SYNC_LOOKBACK_SECS;
                let to = now + SYNC_LOOKAHEAD_SECS;
                match sync::sync_events(&app.conn, store, from, to) {
                    Ok(count) => tracing::debug!("Synced {count} events"),
                    Err(e) => tracing::error!("Failed to sync events: {e}"),
                }
            }

            let now = App::now();
            if let Ok(events) = queries::get_next_events(&app.conn, now, TRAY_EVENT_LIMIT) {
                app.tray.update_events(&events);
            }

            Task::none()
        }

        Message::Dismiss => {
            if let Some(event) = &app.current_event {
                if let Err(e) = queries::set_dismissed(&app.conn, &event.id, App::now()) {
                    tracing::warn!("Failed to dismiss event: {e}");
                }
            }
            close_overlay(app)
        }

        Message::JoinMeeting => {
            if let Some(event) = &app.current_event {
                if let Some(url) = &event.meeting_url {
                    if let Err(e) = open::that(url) {
                        tracing::warn!("Failed to open meeting URL: {e}");
                    }
                }
                if let Err(e) = queries::set_dismissed(&app.conn, &event.id, App::now()) {
                    tracing::warn!("Failed to dismiss event after join: {e}");
                }
            }
            close_overlay(app)
        }

        Message::Snooze(seconds) => {
            if let Some(event) = &app.current_event {
                let until = App::now() + seconds;
                if let Err(e) = queries::set_snoozed(&app.conn, &event.id, until) {
                    tracing::warn!("Failed to snooze event: {e}");
                }
            }
            close_overlay(app)
        }

        Message::SnoozeUntilEvent => {
            if let Some(event) = &app.current_event {
                if let Err(e) = queries::set_snoozed(&app.conn, &event.id, event.start_time) {
                    tracing::warn!("Failed to snooze event: {e}");
                }
            }
            close_overlay(app)
        }
    }
}

fn view(app: &App, _window: window::Id) -> Element<'_, Message> {
    if let Some(event) = &app.current_event {
        let seconds_until = event.start_time - App::now();
        let countdown = format_countdown(seconds_until);
        overlay::view(event, &countdown)
    } else {
        iced::widget::text("").into()
    }
}

fn subscription(app: &App) -> Subscription<Message> {
    Subscription::batch([
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
        iced::time::every(Duration::from_secs(app.settings.poll_interval_seconds as u64))
            .map(|_| Message::SyncCalendar),
    ])
}

/// Get all screen rects as (x, y, width, height) in top-left origin coordinates.
fn get_screen_rects() -> Vec<(f32, f32, f32, f32)> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    // Safe because iced runs on the main thread
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let screens = NSScreen::screens(mtm);

    // macOS uses bottom-left origin. We need to convert to top-left for iced/winit.
    // The primary screen (index 0) defines the coordinate space.
    let all_screens: Vec<_> = screens.iter().collect();
    let primary_height = all_screens
        .first()
        .map(|s| s.frame().size.height as f32)
        .unwrap_or(1080.0);

    all_screens
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            let x = frame.origin.x as f32;
            // Convert from bottom-left to top-left origin
            let y = primary_height - (frame.origin.y as f32 + frame.size.height as f32);
            let w = frame.size.width as f32;
            let h = frame.size.height as f32;
            (x, y, w, h)
        })
        .collect()
}

fn show_overlay(app: &mut App, event: CalendarEvent) -> Task<Message> {
    if app.has_overlay() {
        return Task::none();
    }

    app.current_event = Some(event);

    let screens = get_screen_rects();
    let mut tasks: Vec<Task<Message>> = Vec::new();

    for (x, y, w, h) in &screens {
        let settings = window::Settings {
            size: Size::new(*w, *h),
            position: window::Position::Specific(Point::new(*x, *y)),
            decorations: false,
            transparent: true,
            level: window::Level::AlwaysOnTop,
            resizable: false,
            exit_on_close_request: false,
            ..Default::default()
        };

        let (id, task) = window::open(settings);
        app.overlay_window_ids.push(id);
        tasks.push(task.discard());
        tasks.push(window::gain_focus(id));
    }

    tracing::debug!("Opening overlay on {} screen(s)", screens.len());
    Task::batch(tasks)
}

fn close_overlay(app: &mut App) -> Task<Message> {
    app.current_event = None;
    let ids: Vec<window::Id> = app.overlay_window_ids.drain(..).collect();
    if ids.is_empty() {
        Task::none()
    } else {
        Task::batch(ids.into_iter().map(window::close))
    }
}

fn format_countdown(seconds_until: i64) -> String {
    if seconds_until > 60 {
        let minutes = seconds_until / 60;
        if minutes == 1 {
            "The event will start in 1 minute".to_string()
        } else {
            format!("The event will start in {minutes} minutes")
        }
    } else if seconds_until > 0 {
        format!("The event will start in {seconds_until} seconds")
    } else if seconds_until > -60 {
        "The event is starting now!".to_string()
    } else {
        let minutes_ago = (-seconds_until) / 60;
        if minutes_ago == 1 {
            "The event started 1 minute ago".to_string()
        } else {
            format!("The event started {minutes_ago} minutes ago")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_countdown_minutes() {
        assert_eq!(format_countdown(120), "The event will start in 2 minutes");
        assert_eq!(format_countdown(61), "The event will start in 1 minute");
    }

    #[test]
    fn test_format_countdown_seconds() {
        assert_eq!(format_countdown(30), "The event will start in 30 seconds");
    }

    #[test]
    fn test_format_countdown_now() {
        assert_eq!(format_countdown(0), "The event is starting now!");
        assert_eq!(format_countdown(-30), "The event is starting now!");
    }

    #[test]
    fn test_format_countdown_past() {
        assert_eq!(format_countdown(-120), "The event started 2 minutes ago");
        assert_eq!(format_countdown(-60), "The event started 1 minute ago");
    }
}
