# Helper script for contributors: add common dependencies for the async logger setup

# Add Axum headers support
cargo add axum-extra --features "typed-header"

# Add async Reqwest client (matches logger's async reqwest::Client usage)
cargo add reqwest

# Add Utilities for streams and async operations
cargo add futures-util
cargo add async-stream

# Add UUID with Serde support (fixes the E0277 Serialize error)
cargo add uuid --features "serde v4"