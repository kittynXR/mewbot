use crate::storage::StorageClient;
use async_trait::async_trait;
// use rusqlite::params;

#[async_trait]
pub trait StorageClientExt {
    async fn get_recent_messages(&self, limit: usize) -> Result<Vec<String>, rusqlite::Error>;
    async fn get_user_list(&self) -> Result<Vec<String>, rusqlite::Error>;
    // async fn add_message(&self, message: &str) -> Result<(), rusqlite::Error>;
}

#[async_trait]
impl StorageClientExt for StorageClient {
    async fn get_recent_messages(&self, limit: usize) -> Result<Vec<String>, rusqlite::Error> {
        let query = format!(
            "SELECT message FROM messages ORDER BY timestamp DESC LIMIT {}",
            limit
        );
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&query)?;
        let messages = stmt.query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(messages)
    }

    async fn get_user_list(&self) -> Result<Vec<String>, rusqlite::Error> {
        let query = "SELECT DISTINCT username FROM chatters ORDER BY last_seen DESC";
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(query)?;
        let users = stmt.query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(users)
    }
    // async fn add_message(&self, message: &str) -> Result<(), rusqlite::Error> {
    //     let query = "INSERT INTO messages (message, timestamp) VALUES (?, datetime('now'))";
    //     let conn = self.conn.lock().unwrap();
    //     conn.execute(query, params![message])?;
    //     Ok(())
    // }
}
