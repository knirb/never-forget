use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

use crate::db::queries::CalendarEvent;

pub struct Tray {
    menu: Menu,
    quit_id: String,
    event_items: Vec<MenuItem>,
}

impl Tray {
    pub fn new() -> Self {
        let menu = Menu::new();

        let no_events = MenuItem::new("No upcoming events", false, None);
        menu.append(&no_events).expect("Failed to add menu item");
        menu.append(&PredefinedMenuItem::separator()).expect("Failed to add separator");

        let quit_item = MenuItem::new("Quit Never Forget", true, None);
        let quit_id = quit_item.id().0.clone();
        menu.append(&quit_item).expect("Failed to add quit item");

        let icon_data = create_icon_data();
        let icon = Icon::from_rgba(icon_data, 16, 16).expect("Failed to create icon");

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("Never Forget - Calendar Notifications")
            .with_icon(icon)
            .with_title("NF")
            .build()
            .expect("Failed to create tray icon");

        Box::leak(Box::new(tray));

        Self {
            menu,
            quit_id,
            event_items: vec![no_events],
        }
    }

    pub fn quit_id(&self) -> &str {
        &self.quit_id
    }

    pub fn update_events(&mut self, events: &[CalendarEvent]) {
        // Remove old event items
        for item in self.event_items.drain(..) {
            let _ = self.menu.remove(&item);
        }

        if events.is_empty() {
            let no_events = MenuItem::new("No upcoming events", false, None);
            let _ = self.menu.prepend(&no_events);
            self.event_items.push(no_events);
        } else {
            // Insert in reverse so they end up in correct order via prepend
            for event in events.iter().rev() {
                let time = format_event_time(event.start_time);
                let label = format!("{time}  {}", event.title);
                let item = MenuItem::new(label, false, None);
                let _ = self.menu.prepend(&item);
                self.event_items.push(item);
            }
        }
    }
}

pub fn poll_menu_event() -> Option<MenuEvent> {
    MenuEvent::receiver().try_recv().ok()
}

fn format_event_time(timestamp: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| {
            let now = Local::now();
            if dt.date_naive() == now.date_naive() {
                dt.format("%H:%M").to_string()
            } else {
                dt.format("%a %H:%M").to_string()
            }
        })
        .unwrap_or_else(|| "??:??".to_string())
}

fn create_icon_data() -> Vec<u8> {
    let mut data = vec![0u8; 16 * 16 * 4];
    let center = 7.5_f32;
    let radius = 6.0_f32;

    for y in 0..16 {
        for x in 0..16 {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = (y * 16 + x) * 4;
            if dist <= radius {
                data[idx] = 0xFF;     // R
                data[idx + 1] = 0x95; // G
                data[idx + 2] = 0x00; // B
                data[idx + 3] = 0xFF; // A
            }
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_event_time_today() {
        let now = chrono::Local::now().timestamp();
        let result = format_event_time(now);
        assert!(result.contains(':'));
        // Today's events show just HH:MM (no day name)
        assert!(!result.contains("Mon"));
        assert!(!result.contains("Tue"));
    }

    #[test]
    fn test_format_event_time_future() {
        // A week from now should show day name
        let future = chrono::Local::now().timestamp() + 7 * 86400;
        let result = format_event_time(future);
        assert!(result.contains(':'));
    }
}
