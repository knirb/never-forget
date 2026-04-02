pub mod queries;
pub mod schema;

use rusqlite::Connection;
use std::path::PathBuf;

pub fn db_path() -> PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neverforget");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
    data_dir.join("events.db")
}

pub fn open_connection() -> rusqlite::Result<Connection> {
    let path = db_path();
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    schema::run_migrations(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?; // WAL not needed for in-memory
    schema::run_migrations(&conn)?;
    Ok(conn)
}
