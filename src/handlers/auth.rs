use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use std::fmt;

#[derive(Debug)]
pub enum AuthError {
    DbError(rusqlite::Error),
    Forbidden,
    LockError,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthError::DbError(e) => write!(f, "Internal Database Error: {}", e),
            AuthError::Forbidden => write!(f, "403 Forbidden: Invalid API Key"),
            AuthError::LockError => write!(f, "Internal State Contention"),
        }
    }
}

#[derive(Clone)]
pub struct Authentication {
    conn: Arc<Mutex<Connection>>,
}

impl Authentication {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS api_keys (
                id INTEGER PRIMARY KEY,
                api_key TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL
            )",
            [],
        )?;

        Ok(Self { 
            conn: Arc::new(Mutex::new(conn)) 
        })
    }

    pub fn register_key(&self, api_key: &str, email: &str) -> Result<(), AuthError> {
        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;
        conn.execute(
            "INSERT OR IGNORE INTO api_keys (api_key, email) VALUES (?1, ?2)",
            params![api_key, email],
        ).map_err(AuthError::DbError)?;
        Ok(())
    }

    pub fn authenticate(&self, provided_key: &str) -> std::result::Result<String, AuthError> {
        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;
        
        let result = conn.query_row(
            "SELECT email FROM api_keys WHERE api_key = ?1",
            params![provided_key],
            |row| row.get::<_, String>(0)
        );

        match result {
            Ok(email) => Ok(email),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(AuthError::Forbidden),
            Err(e) => Err(AuthError::DbError(e)),
        }
    }
}