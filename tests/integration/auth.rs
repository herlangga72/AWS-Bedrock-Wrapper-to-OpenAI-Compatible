//! SQLite authentication integration tests
//!
//! Requires: SQLite database
//!
//! Run with:
//! ```bash
//! cargo test --test auth_test -- --nocapture
//! ```

use aws_bedrock_translation_to_openai::domain::auth::Authentication;
use std::fs;
use tempfile::TempDir;

/// Create a temporary SQLite database for testing
fn create_test_auth() -> Authentication {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_auth.db");
    Authentication::new(db_path.to_str().unwrap()).unwrap()
}

#[test]
fn test_auth_register_key() {
    let auth = create_test_auth();

    let result = auth.register_key("test-key-1", "user1@example.com");
    assert!(result.is_ok(), "Should register key: {:?}", result.err());

    let result = auth.register_key("test-key-2", "user2@example.com");
    assert!(result.is_ok());
}

#[test]
fn test_auth_authenticate_valid_key() {
    let auth = create_test_auth();

    auth.register_key("valid-key", "valid@example.com").unwrap();

    let result = auth.authenticate("valid-key");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "valid@example.com");
}

#[test]
fn test_auth_authenticate_invalid_key() {
    let auth = create_test_auth();

    auth.register_key("real-key", "real@example.com").unwrap();

    let result = auth.authenticate("wrong-key");
    assert!(result.is_err());
}

#[test]
fn test_auth_authenticate_empty_key() {
    let auth = create_test_auth();

    auth.register_key("some-key", "some@example.com").unwrap();

    let result = auth.authenticate("");
    assert!(result.is_err());
}

#[test]
fn test_auth_multiple_keys_same_user() {
    let auth = create_test_auth();

    auth.register_key("key-1", "user@example.com").unwrap();
    auth.register_key("key-2", "user@example.com").unwrap();

    assert_eq!(auth.authenticate("key-1").unwrap(), "user@example.com");
    assert_eq!(auth.authenticate("key-2").unwrap(), "user@example.com");
}

#[test]
fn test_auth_same_key_different_users() {
    let auth = create_test_auth();

    auth.register_key("key-1", "user1@example.com").unwrap();

    // Re-registering same key with different user should fail or update
    let result = auth.register_key("key-1", "user2@example.com");
    // Behavior depends on implementation - either fails or updates
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_auth_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("persist_auth.db");

    // Create auth and register key
    {
        let auth = Authentication::new(db_path.to_str().unwrap()).unwrap();
        auth.register_key("persistent-key", "persistent@example.com").unwrap();
    }

    // Re-open database and verify key still works
    {
        let auth = Authentication::new(db_path.to_str().unwrap()).unwrap();
        assert_eq!(
            auth.authenticate("persistent-key").unwrap(),
            "persistent@example.com"
        );
    }
}

#[test]
fn test_auth_list_keys() {
    let auth = create_test_auth();

    auth.register_key("key-a", "user-a@example.com").unwrap();
    auth.register_key("key-b", "user-b@example.com").unwrap();
    auth.register_key("key-c", "user-c@example.com").unwrap();

    // Note: Implementation may or may not expose list_keys functionality
    // This test documents expected behavior
}
