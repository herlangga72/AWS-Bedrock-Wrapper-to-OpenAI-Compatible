//! Authentication service - API key validation with SQLite backend

use crate::domain::auth::types::AuthError;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

/// Authentication service for API key validation
#[derive(Clone)]
pub struct Authentication {
    conn: Arc<Mutex<Connection>>,
}

impl Authentication {
    pub fn new(path: &str) -> Result<Self, AuthError> {
        let conn = Connection::open(path).map_err(|e| AuthError::DbError(e.to_string()))?;

        Self::init_schema(&conn)?;
        Self::run_migrations(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_schema(conn: &Connection) -> Result<(), AuthError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS api_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key_hash TEXT NOT NULL UNIQUE,
                key_prefix TEXT NOT NULL,
                email TEXT NOT NULL,
                name TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_used TEXT,
                is_active INTEGER NOT NULL DEFAULT 1
            )",
            [],
        )
        .map_err(|e| AuthError::DbError(e.to_string()))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_api_keys_key_prefix ON api_keys(key_prefix)",
            [],
        )
        .map_err(|e| AuthError::DbError(e.to_string()))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_api_keys_email ON api_keys(email)",
            [],
        )
        .map_err(|e| AuthError::DbError(e.to_string()))?;

        Ok(())
    }

    fn run_migrations(conn: &Connection) -> Result<(), AuthError> {
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='api_keys'",
                [],
                |row| row.get::<_, i32>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);

        if !table_exists {
            return Ok(());
        }

        let has_key_hash: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('api_keys') WHERE name='key_hash'",
                [],
                |row| row.get::<_, i32>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);

        if !has_key_hash {
            tracing::info!("Migrating api_keys table to new schema...");
            conn.execute("ALTER TABLE api_keys RENAME TO api_keys_old", [])
                .map_err(|e| AuthError::DbError(e.to_string()))?;

            Self::init_schema(conn)?;

            let old_exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='api_keys_old'",
                    [],
                    |row| row.get::<_, i32>(0),
                )
                .unwrap_or(0)
                > 0;

            if old_exists {
                let mut stmt = conn
                    .prepare("SELECT id, api_key, email FROM api_keys_old")
                    .map_err(|e| AuthError::DbError(e.to_string()))?;

                let old_keys: Vec<(i64, String, String)> = stmt
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                    .map_err(|e| AuthError::DbError(e.to_string()))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| AuthError::DbError(e.to_string()))?;

                for (_old_id, api_key, email) in old_keys {
                    let key_hash = Self::hash_key(&api_key);
                    let key_prefix = Self::extract_prefix(&key_hash);
                    conn.execute(
                        "INSERT OR IGNORE INTO api_keys (key_hash, key_prefix, email) VALUES (?1, ?2, ?3)",
                        params![key_hash, key_prefix, email],
                    )
                    .map_err(|e| AuthError::DbError(e.to_string()))?;
                }

                conn.execute("DROP TABLE api_keys_old", [])
                    .map_err(|e| AuthError::DbError(e.to_string()))?;
                tracing::info!("Migration complete");
            }
        }

        Ok(())
    }

    /// Hash API key for storage
    fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Extract prefix for faster lookups (first 8 chars of hash)
    fn extract_prefix(hash: &str) -> String {
        hash.chars().take(8).collect()
    }

    /// Register a new API key
    pub fn register_key(&self, api_key: &str, email: &str) -> Result<(), AuthError> {
        self.register_key_with_name(api_key, email, None)
    }

    /// Register a new API key with optional name
    pub fn register_key_with_name(
        &self,
        api_key: &str,
        email: &str,
        name: Option<&str>,
    ) -> Result<(), AuthError> {
        if api_key.len() < 8 {
            return Err(AuthError::InvalidKeyFormat);
        }

        let key_hash = Self::hash_key(api_key);
        let key_prefix = Self::extract_prefix(&key_hash);

        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;
        conn.execute(
            "INSERT OR IGNORE INTO api_keys (key_hash, key_prefix, email, name) VALUES (?1, ?2, ?3, ?4)",
            params![key_hash, key_prefix, email, name],
        )
        .map_err(|e| AuthError::DbError(e.to_string()))?;
        Ok(())
    }

    /// Authenticate and return email if valid
    pub fn authenticate(&self, provided_key: &str) -> Result<String, AuthError> {
        if provided_key.len() < 8 {
            return Err(AuthError::Forbidden);
        }

        let key_hash = Self::hash_key(provided_key);
        let key_prefix = Self::extract_prefix(&key_hash);

        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;

        let result = conn.query_row(
            "SELECT email, is_active FROM api_keys WHERE key_hash = ?1 AND key_prefix = ?2 AND is_active = 1",
            params![key_hash, key_prefix],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        );

        match result {
            Ok((email, _)) => {
                let _ = conn.execute(
                    "UPDATE api_keys SET last_used = datetime('now') WHERE key_hash = ?1",
                    params![key_hash],
                );
                Ok(email)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(AuthError::Forbidden),
            Err(e) => Err(AuthError::DbError(e.to_string())),
        }
    }

    /// List all API keys (without revealing the actual keys)
    pub fn list_keys(&self) -> Result<Vec<crate::domain::auth::types::ApiKey>, AuthError> {
        use crate::domain::auth::types::ApiKey;

        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, key_hash, email, name, created_at, last_used, is_active FROM api_keys ORDER BY created_at DESC",
            )
            .map_err(|e| AuthError::DbError(e.to_string()))?;

        let keys = stmt
            .query_map([], |row| {
                Ok(ApiKey {
                    id: row.get(0)?,
                    key_hash: row.get(1)?,
                    email: row.get(2)?,
                    name: row.get(3)?,
                    created_at: row.get(4)?,
                    last_used: row.get(5)?,
                    is_active: row.get::<_, i32>(6)? == 1,
                })
            })
            .map_err(|e| AuthError::DbError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AuthError::DbError(e.to_string()))?;

        Ok(keys)
    }

    /// Deactivate an API key
    pub fn deactivate(&self, email: &str) -> Result<bool, AuthError> {
        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;
        let affected = conn
            .execute(
                "UPDATE api_keys SET is_active = 0 WHERE email = ?1",
                params![email],
            )
            .map_err(|e| AuthError::DbError(e.to_string()))?;
        Ok(affected > 0)
    }

    /// Reactivate an API key
    pub fn reactivate(&self, email: &str) -> Result<bool, AuthError> {
        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;
        let affected = conn
            .execute(
                "UPDATE api_keys SET is_active = 1 WHERE email = ?1",
                params![email],
            )
            .map_err(|e| AuthError::DbError(e.to_string()))?;
        Ok(affected > 0)
    }

    /// Delete an API key
    pub fn delete(&self, email: &str) -> Result<bool, AuthError> {
        let conn = self.conn.lock().map_err(|_| AuthError::LockError)?;
        let affected = conn
            .execute("DELETE FROM api_keys WHERE email = ?1", params![email])
            .map_err(|e| AuthError::DbError(e.to_string()))?;
        Ok(affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_auth() -> Authentication {
        Authentication::new(":memory:").unwrap()
    }

    #[test]
    fn test_register_and_authenticate() {
        let auth = create_test_auth();
        auth.register_key("test-key-12345", "test@example.com")
            .unwrap();
        assert_eq!(
            auth.authenticate("test-key-12345").unwrap(),
            "test@example.com"
        );
    }

    #[test]
    fn test_wrong_key() {
        let auth = create_test_auth();
        assert!(auth.authenticate("wrong-key-12345").is_err());
    }

    #[test]
    fn test_short_key() {
        let auth = create_test_auth();
        assert!(auth.authenticate("short").is_err());
    }

    #[test]
    fn test_hash_consistency() {
        let hash1 = Authentication::hash_key("test-key");
        let hash2 = Authentication::hash_key("test-key");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_prefix_extraction() {
        let hash = "abcd12345678";
        assert_eq!(Authentication::extract_prefix(hash), "abcd1234");
    }

    #[test]
    fn test_deactivate() {
        let auth = create_test_auth();
        auth.register_key("test-key-12345", "test@example.com")
            .unwrap();
        assert!(auth.deactivate("test@example.com").unwrap());
        assert!(auth.authenticate("test-key-12345").is_err());
    }

    #[test]
    fn test_reactivate() {
        let auth = create_test_auth();
        auth.register_key("test-key-12345", "test@example.com")
            .unwrap();
        auth.deactivate("test@example.com").unwrap();
        assert!(auth.reactivate("test@example.com").unwrap());
        assert!(auth.authenticate("test-key-12345").is_ok());
    }

    #[test]
    fn test_delete() {
        let auth = create_test_auth();
        auth.register_key("test-key-12345", "test@example.com")
            .unwrap();
        assert!(auth.delete("test@example.com").unwrap());
        assert!(auth.authenticate("test-key-12345").is_err());
    }

    #[test]
    fn test_multiple_keys_same_email() {
        let auth = create_test_auth();
        auth.register_key("test-key-12345", "test@example.com")
            .unwrap();
        auth.register_key("another-key-12345", "test@example.com")
            .unwrap();
        assert_eq!(
            auth.authenticate("test-key-12345").unwrap(),
            "test@example.com"
        );
        assert_eq!(
            auth.authenticate("another-key-12345").unwrap(),
            "test@example.com"
        );
    }
}
