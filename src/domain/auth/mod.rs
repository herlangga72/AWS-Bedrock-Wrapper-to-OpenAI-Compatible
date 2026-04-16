//! Auth domain - API key authentication

pub mod service;
pub mod types;

pub use service::Authentication;
pub use types::{ApiKey, AuthError};
