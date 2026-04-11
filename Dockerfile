FROM rust:1.77-slim as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy main.rs for dependency caching
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Build dependencies only (for caching)
RUN cargo build --release && rm -rf src

# Copy actual source
COPY src ./src
COPY tests ./tests
COPY Makefile ./

# Build the application
RUN touch src/main.rs src/lib.rs && cargo build --release

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/aws-bedrock-translation-to-openai /app/server
COPY .env.example /app/.env

EXPOSE 3001

CMD ["/app/server"]
