//! Anthropic-to-Bedrock translation layer
//!
//! Translates Anthropic native /v1/messages requests to AWS Bedrock Converse API calls.

pub mod request;

pub use request::*;
