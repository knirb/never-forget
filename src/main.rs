mod app;
mod calendar;
mod db;
mod meeting_url;
mod notifications;
mod overlay;
mod settings;
mod tray;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    tracing::info!("Never Forget starting...");
    app::run()
}
