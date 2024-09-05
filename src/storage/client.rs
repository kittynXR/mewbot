use rusqlite::{Connection, Result, params};
use crate::storage::models::ChatterData;
use std::path::Path;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};
use log::info;
use parking_lot::RwLock;
use lru_cache::LruCache;
use crate::twitch::roles::UserRole;

pub struct StorageClient {
    pub(crate) conn: Arc<Mutex<Connection>>,
    statement_cache: Arc<RwLock<LruCache<String, String>>>,
}

impl StorageClient {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Open a new connection (this will create a new file)
        let conn = Connection::open(&path)?;

        // Enable foreign key support
        conn.execute("PRAGMA foreign_keys = ON", [])?;

        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chatters (
                user_id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                is_streamer BOOLEAN NOT NULL,
                chatter_type TEXT NOT NULL,
                sentiment REAL NOT NULL,
                content_summary TEXT,
                custom_notes TEXT,
                last_seen INTEGER NOT NULL,
                role TEXT NOT NULL DEFAULT 'Viewer'
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY,
                user_id TEXT NOT NULL,
                message TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES chatters(user_id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS stream_data (
                id INTEGER PRIMARY KEY,
                user_id TEXT NOT NULL,
                title TEXT NOT NULL,
                category TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES chatters(user_id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS verification_codes (
                user_id TEXT PRIMARY KEY,
                code INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        println!("Database schema created or updated successfully");

        Ok(StorageClient {
            conn: Arc::new(Mutex::new(conn)),
            statement_cache: Arc::new(RwLock::new(LruCache::new(100))),
        })
    }

    pub async fn close(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Closing StorageClient...");
        // Implement logic to close database connections
        Ok(())
    }

    pub async fn store_verification_code(&self, user_id: &str, code: u32) -> Result<(), rusqlite::Error> {
        let query = "INSERT OR REPLACE INTO verification_codes (user_id, code, created_at) VALUES (?1, ?2, ?3)";
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(query)?;
        stmt.execute(params![user_id, code, Utc::now().timestamp()])?;
        Ok(())
    }

    pub async fn get_verification_code(&self, user_id: &str) -> Result<Option<u32>, rusqlite::Error> {
        let query = "SELECT code FROM verification_codes WHERE user_id = ?1";
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(query)?;
        let result = stmt.query_row([user_id], |row| row.get(0));
        match result {
            Ok(code) => Ok(Some(code)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn remove_verification_code(&self, user_id: &str) -> Result<(), rusqlite::Error> {
        let query = "DELETE FROM verification_codes WHERE user_id = ?1";
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(query)?;
        stmt.execute([user_id])?;
        Ok(())
    }

    pub fn add_message(&self, user_id: &str, message: &str) -> Result<()> {
        let query = "INSERT INTO messages (user_id, message, timestamp) VALUES (?1, ?2, ?3)";

        let conn = self.conn.lock().unwrap();

        // First, ensure the user exists in the chatters table
        self.ensure_chatter_exists(user_id, &conn)?;

        let mut stmt = conn.prepare_cached(query)?;
        stmt.execute(params![user_id, message, Utc::now().timestamp()])?;

        Ok(())
    }

    pub fn add_stream_data(&self, user_id: &str, title: &str, category: &str) -> Result<()> {
        let query = "INSERT INTO stream_data (user_id, title, category, timestamp) VALUES (?1, ?2, ?3, ?4)";

        let conn = self.conn.lock().unwrap();

        // First, ensure the user exists in the chatters table
        self.ensure_chatter_exists(user_id, &conn)?;

        let mut stmt = conn.prepare_cached(query)?;
        stmt.execute(params![user_id, title, category, Utc::now().timestamp()])?;

        Ok(())
    }

    fn ensure_chatter_exists(&self, user_id: &str, conn: &Connection) -> Result<()> {
        let query = "INSERT OR IGNORE INTO chatters (user_id, username, is_streamer, chatter_type, sentiment, last_seen)
                     VALUES (?1, ?1, 0, 'new', 0.0, ?2)";

        let mut stmt = conn.prepare_cached(query)?;
        stmt.execute(params![user_id, Utc::now().timestamp()])?;

        Ok(())
    }

    pub fn get_chatter_data(&self, user_id: &str) -> Result<Option<ChatterData>> {
        let query = "SELECT username, is_streamer, chatter_type, sentiment, content_summary, custom_notes, last_seen, role FROM chatters WHERE user_id = ?1";

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(query)?;
        let result = stmt.query_row([user_id], |row| {
            Ok(ChatterData {
                user_id: user_id.to_string(),
                username: row.get(0)?,
                messages: vec![], // We'll fetch messages separately if needed
                sentiment: row.get(3)?,
                chatter_type: row.get(2)?,
                is_streamer: row.get(1)?,
                stream_titles: None, // We'll fetch stream data separately if needed
                stream_categories: None,
                content_summary: row.get(4)?,
                custom_notes: row.get(5)?,
                last_seen: DateTime::from_timestamp(row.get::<_, i64>(6)?, 0)
                    .unwrap_or_else(|| Utc::now()),
                role: UserRole::from_str(&row.get::<_, String>(7)?).unwrap_or(UserRole::Viewer),
            })
        });

        match result {
            Ok(chatter_data) => Ok(Some(chatter_data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn upsert_chatter(&self, data: &ChatterData) -> Result<()> {
        println!("Upserting chatter data for user_id: {}", data.user_id);
        let query = "INSERT OR REPLACE INTO chatters (user_id, username, is_streamer, chatter_type, sentiment, content_summary, custom_notes, last_seen, role)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(query)?;
        let result = stmt.execute(params![
        data.user_id,
        data.username,
        data.is_streamer,
        data.chatter_type,
        data.sentiment,
        data.content_summary,
        data.custom_notes,
        data.last_seen.timestamp(),
        data.role.to_string(),
    ]);

        match &result {
            Ok(_) => println!("Successfully upserted chatter data"),
            Err(e) => println!("Error upserting chatter data: {:?}", e),
        }

        result.map(|_| ())
    }
}

unsafe impl Send for StorageClient {}
unsafe impl Sync for StorageClient {}