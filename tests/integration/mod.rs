//! Integration tests module
//!
//! These tests require running infrastructure (ClickHouse, LocalStack)
//! Use: `docker-compose up integration-tests`
//!
//! Or run manually with:
//! ```bash
//! docker-compose up -d clickhouse localstack
//! cargo test --test '*_test'
//! ```

pub mod logging;
pub mod auth;
