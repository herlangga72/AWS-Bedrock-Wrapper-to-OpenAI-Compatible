# Add Axum headers support
cargo add axum-extra --features "typed-header"

# Add Reqwest with blocking support (needed for your logger)
cargo add reqwest --features "blocking"

# Add Utilities for streams and async operations
cargo add futures-util
cargo add async-stream

# Add UUID with Serde support (fixes the E0277 Serialize error)
cargo add uuid --features "serde v4"