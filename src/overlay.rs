use iced::widget::{button, center, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};

use crate::app::Message;
use crate::db::queries::CalendarEvent;

const BG_COLOR: Color = Color::from_rgba(0.08, 0.08, 0.10, 0.45);
const CARD_BG: Color = Color::from_rgba(0.14, 0.14, 0.16, 0.85);
const ACCENT: Color = Color::from_rgb(1.0, 0.584, 0.0); // #FF9500 orange
const TEXT_COLOR: Color = Color::WHITE;
const MUTED_TEXT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.7);

pub fn view<'a>(event: &CalendarEvent, countdown_text: &str) -> Element<'a, Message> {
    let title = text(event.title.clone())
        .size(32)
        .color(TEXT_COLOR);

    let start = format_time(event.start_time);
    let end = format_time(event.end_time);
    let time_range = text(format!("{start} \u{2013} {end}"))
        .size(18)
        .color(MUTED_TEXT);

    let countdown = text(countdown_text.to_string())
        .size(16)
        .color(MUTED_TEXT);

    let join_btn = if event.meeting_url.is_some() {
        column![
            styled_button("Join", Message::JoinMeeting, true),
        ]
    } else {
        column![]
    };

    let dismiss_btn = styled_button("Dismiss", Message::Dismiss, false);

    let snooze_label = row![
        text("Snooze").size(14).color(MUTED_TEXT),
    ]
    .align_y(Alignment::Center);

    let snooze_buttons = row![
        styled_button_sized("1 minute", Message::Snooze(60), false, 120, 13),
        styled_button_sized("5 minutes", Message::Snooze(300), false, 120, 13),
        styled_button_sized("Until Event", Message::SnoozeUntilEvent, false, 120, 13),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let card_content = column![
        title,
        time_range,
        Space::with_height(8),
        countdown,
        Space::with_height(20),
        join_btn,
        dismiss_btn,
        Space::with_height(16),
        snooze_label,
        Space::with_height(4),
        snooze_buttons,
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .padding(30)
    .width(Length::Shrink);

    // Use a thick left border as the calendar color indicator
    let card = container(card_content)
        .width(Length::Shrink)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(CARD_BG)),
            border: Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    container(center(card))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn styled_button(label: &str, msg: Message, primary: bool) -> Element<'_, Message> {
    styled_button_sized(label, msg, primary, 280, 16)
}

fn styled_button_sized(label: &str, msg: Message, primary: bool, width: u16, font_size: u16) -> Element<'_, Message> {
    let bg = if primary { ACCENT } else { Color::TRANSPARENT };
    let text_color = if primary { Color::BLACK } else { ACCENT };

    button(
        text(label)
            .size(font_size)
            .color(text_color)
            .align_x(Alignment::Center)
    )
    .on_press(msg)
    .width(width)
    .padding([8, 12])
    .style(move |_: &Theme, _status| button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: ACCENT,
            width: if primary { 1.5 } else { 1.0 },
            radius: if primary { 8.0 } else { 6.0 }.into(),
        },
        ..Default::default()
    })
    .into()
}

fn format_time(timestamp: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_else(|| "??:??".to_string())
}

fn parse_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(149);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        Color::from_rgb8(r, g, b)
    } else {
        ACCENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_valid_hex() {
        let c = parse_color("#FF5733");
        assert_eq!(c, Color::from_rgb8(255, 87, 51));
    }

    #[test]
    fn test_parse_color_no_hash() {
        let c = parse_color("00FF00");
        assert_eq!(c, Color::from_rgb8(0, 255, 0));
    }

    #[test]
    fn test_parse_color_invalid() {
        let c = parse_color("xyz");
        assert_eq!(c, ACCENT);
    }

    #[test]
    fn test_format_time() {
        let result = format_time(1700000000);
        assert!(result.contains(':'));
    }
}
