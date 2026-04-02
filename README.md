# Never Forget

An aggressive calendar notification app for macOS that makes sure you never miss a meeting.

When an event is about to start, a full-screen overlay takes over your screen displaying the event details, countdown timer, and action buttons. No more "I didn't see the notification" excuses.

## Features

- **Full-screen overlay notifications** — impossible to miss, covers your entire screen
- **Live countdown timer** — shows exactly how long until your event starts
- **Join button** — automatically extracts meeting URLs (Zoom, Google Meet, Teams, Webex) from your events and opens them with one click
- **Snooze** — snooze for 1 minute, 5 minutes, or until the event starts
- **Dismiss** — dismiss the notification if you don't need it
- **macOS Calendar sync** — reads directly from your macOS Calendar app via EventKit
- **Calendar color support** — shows the calendar color indicator for each event
- **Menu bar app** — lives quietly in your menu bar, only appears when needed
- **Configurable timing** — set how many minutes before an event you want to be notified (default: 1 minute)
- **Lightweight** — native Rust app, minimal resource usage
- **Local SQLite database** — all data stored locally, no cloud dependency

## Requirements

- macOS 14.0+
- Calendar access permission (prompted on first launch)

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run
```

On first launch, macOS will prompt you to grant calendar access. The app will then sync your calendar events and show overlay notifications before events start.

## Configuration

Settings are stored in a local SQLite database at:
```
~/Library/Application Support/neverforget/events.db
```

Default settings:
- **Notification timing**: 1 minute before event
- **Sync interval**: every 30 seconds

## Tech Stack

- **Rust** — systems programming language
- **iced** — cross-platform GUI framework
- **EventKit** (via objc2) — macOS calendar integration
- **SQLite** (via rusqlite) — local event storage
- **tray-icon** — menu bar integration

## Contributing

This project uses [Conventional Commits](https://www.conventionalcommits.org/) for all commit messages.

## License

MIT
