# AWS Bedrock Proxy - Architecture Documentation

**Last Updated:** 2026-04-11

---

## Overview

This is an OpenAI-compatible API proxy for AWS Bedrock and Cloudflare Workers AI. It translates OpenAI-format requests into provider-specific API calls.

```
Client Request (OpenAI format)
       │
       ▼
┌─────────────────────────────────┐
│ Interface Layer (HTTP Handlers) │
│  - chat_handler                 │
│  - thinking_handler             │
│  - reasoning_handler            │
│  - embedding_handler            │
└-────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│   Domain Layer (Business Logic) │
│ - capabilities.rs (model reg)   │
│ - types.rs (domain models)      │
│ - auth (API key validation)     │
│ - logging (usage tracking)      │
└─────────────────────────────────┘
       │
       ▼
┌───────────────────────────────────┐
│Infrastructure (External Services) │
│- bedrock/ (AWS Bedrock)           │
│- cloudflare/ (Cloudflare AI)      │
│- cache/ (file-based cache)        │
│- persistence/ (SQLite, ClickHouse)│
└───────────────────────────────────┘
```

---

## Directory Structure

```
src/
├── main.rs              # Application entry point
├── lib.rs               # Library root (for testing)
├── domain/              # Business logic & entities
│   ├── chat/
│   │   ├── mod.rs       # Chat domain exports
│   │   ├── types.rs     # ChatRequest, Message, Content, etc.
│   │   ├── errors.rs    # ChatError enum
│   │   └── capabilities.rs  # Model registry & param mapping
│   ├── embedding/
│   │   ├── mod.rs
│   │   └── types.rs     # OpenAiEmbeddingRequest, Nova types
│   ├── auth/
│   │   ├── mod.rs
│   │   └── types.rs     # Authentication service (SQLite)
│   └── logging/
│       ├── mod.rs
│       └── types.rs     # ClickHouseLogger
├── interface/           # HTTP handlers
│   ├── chat/
│   │   ├── mod.rs
│   │   ├── chat_handler.rs      # Standard chat (Converse API)
│   │   ├── thinking_handler.rs   # Claude extended thinking
│   │   └── reasoning_handler.rs # DeepSeek R1 reasoning
│   ├── embedding/
│   │   ├── mod.rs
│   │   └── embedding_handler.rs  # Nova embeddings
│   └── models/
│       ├── mod.rs
│       └── models_handler.rs     # List available models
├── infrastructure/      # External integrations
│   ├── bedrock/
│   │   ├── mod.rs
│   │   ├── converse.rs  # Converse API builder
│   │   └── invoke.rs    # Invoke API (thinking)
│   ├── cloudflare/
│   │   ├── mod.rs
│   │   └── client.rs    # Cloudflare Workers AI client
│   └── cache/
│       ├── mod.rs
│       └── file_cache.rs # Model list caching
├── shared/              # Shared utilities
│   ├── mod.rs
│   ├── app_state.rs    # AppState struct
│   └── extractors.rs   # HTTP extractors
└── application/         # Use case orchestration
    ├── mod.rs
    ├── chat/
    └── embedding/
```

---

## File Documentation

### Root Files

#### `main.rs`
**Purpose:** Application entry point and router setup

**Key Responsibilities:**
- Initialize AWS config and clients
- Initialize auth service (SQLite)
- Initialize ClickHouse logger
- Initialize Cloudflare client (if configured)
- Register default API key from env
- Start cache monitor background task
- Setup Axum router with routes

**Routes:**
- `POST /v1/chat/completions` → `chat_with_thinking_handler`
- `GET /v1/models` → `list_models_handler`
- `POST /v1/embeddings` → `handle_embeddings`

---

### Domain Layer

#### `domain/chat/types.rs`
**Purpose:** OpenAI-compatible chat domain types

**Key Types:**
- `ChatRequest` - Incoming chat request (OpenAI format)
- `Message` - Chat message with role and content
- `Content` - Text or blocks content
- `ContentBlock` - Multi-modal content block
- `ThinkingRequest` - Claude extended thinking config
- `ReasoningContent` - DeepSeek reasoning content

**Test Coverage:** 22 tests (serialization, deserialization, edge cases)

#### `domain/chat/capabilities.rs`
**Purpose:** Model capability registry and parameter mapping

**Key Types:**
- `Vendor` - Enum (AwsBedrock, Cloudflare)
- `ModelCapabilities` - Model configuration
- `BaseParams` - Common params (max_tokens, temperature, top_p)
- `ModelSpecificParams` - Provider-specific params (top_k, penalties)
- `ThinkingConfig` - Claude thinking budget

**Key Functions:**
- `get_model_capabilities(model_id)` - Lookup model config
- `map_openai_params(...)` - Map OpenAI params to provider format

**Supported Models:**
- Claude (anthropic.claude-*)
- DeepSeek (deepseek.r1, deepseek.chat)
- Cohere (cohere.command-r)
- AI21 (ai21.j2, ai21.jurassic)
- Mistral (mistral.*)
- Llama (meta.llama*, llama*)
- Amazon Titan (amazon.titan*)
- Amazon Nova (amazon.nova*)
- Cloudflare (@cf/*)

**Test Coverage:** 31 tests (model matching, param mapping, vendor detection)

#### `domain/chat/errors.rs`
**Purpose:** Chat-specific error types

**Types:**
- `ChatError` - Error enum for chat operations

**Note:** Not used in current implementation (uses String errors)

#### `domain/embedding/types.rs`
**Purpose:** Embedding domain types

**Key Types:**
- `OpenAiEmbeddingRequest` - OpenAI-compatible input format
- `NovaRequest` - AWS Nova embedding request builder
- `NovaResponse` - AWS Nova response format
- `OpenAiEmbeddingResponse` - OpenAI-compatible output
- `OpenAiEmbeddingData` - Individual embedding result

**Test Coverage:** 9 tests (serialization, deserialization, builder)

#### `domain/auth/types.rs`
**Purpose:** API key authentication via SQLite

**Key Types:**
- `Authentication` - Auth service
- `AuthError` - Auth failure types

**Note:** Not tested (requires SQLite)

#### `domain/logging/types.rs`
**Purpose:** Usage logging to ClickHouse

**Key Types:**
- `ClickHouseLogger` - Async ClickHouse client
- `LogEntry` - Usage log record

**Note:** Not tested (requires ClickHouse)

---

### Interface Layer (HTTP Handlers)

#### `interface/chat/chat_handler.rs`
**Purpose:** Standard chat completion handler using AWS Bedrock Converse API

**Key Functions:**
- `chat_handler()` - Main entry point, routes to Bedrock or Cloudflare
- `stream_converse()` - Streaming response handler for Bedrock
- `non_stream()` - Non-streaming response handler for Bedrock
- `stream_cloudflare()` - Streaming handler for Cloudflare
- `non_stream_cloudflare()` - Non-streaming handler for Cloudflare

**Routing Logic:**
```
if model.starts_with("@cf/") → Cloudflare
else → AWS Bedrock (Converse API)
```

**Model Name Normalization:**
- Cloudflare: `@cf/provider/model` → `cloudflare/provider/model`
- Bedrock: `bedrock/model` → `aws/bedrock/model`

**Note:** Not tested (requires AWS clients)

#### `interface/chat/thinking_handler.rs`
**Purpose:** Claude extended thinking handler

**Key Functions:**
- `chat_with_thinking_handler()` - Entry point (routes to thinking or standard)
- `thinking_handler()` - Detects thinking-enabled models
- `non_stream_thinking()` - Claude thinking API handler

**Note:** Not tested (requires AWS clients)

#### `interface/chat/reasoning_handler.rs`
**Purpose:** DeepSeek R1 reasoning handler

**Key Functions:**
- `chat_with_reasoning_handler()` - Entry point
- `reasoning_handler()` - Routes to reasoning or standard
- `non_stream_reasoning()` - DeepSeek R1 handler

**Note:** Not tested (requires AWS clients)

#### `interface/embedding/embedding_handler.rs`
**Purpose:** Nova embedding handler

**Key Functions:**
- `handle_embeddings()` - Routes to Nova

**Note:** Not tested (requires AWS clients)

#### `interface/models/models_handler.rs`
**Purpose:** List available models

**Key Functions:**
- `list_models_handler()` - Returns cached model list

**Note:** Not tested

---

### Infrastructure Layer

#### `infrastructure/bedrock/converse.rs`
**Purpose:** AWS Bedrock Converse API builder

**Key Functions:**
- `build_converse_payload()` - Build Converse API request
- `extract_text_from_content()` - Parse response text

**Types:**
- `ConversePayload` - Request payload structure

**Note:** Not tested (requires AWS clients)

#### `infrastructure/bedrock/invoke.rs`
**Purpose:** AWS Bedrock Invoke API for Claude thinking

**Key Functions:**
- `build_thinking_request()` - Build thinking request
- `invoke_thinking_model()` - Call Invoke API
- `parse_thinking_params()` - Parse thinking config

**Types:**
- `ThinkingRequestBody` - Request body for thinking
- `ThinkingResponse` - Response with thinking blocks

**Note:** Not tested (requires AWS clients)

#### `infrastructure/cloudflare/client.rs`
**Purpose:** Cloudflare Workers AI API client

**Key Types:**
- `CloudflareClient` - HTTP client for Cloudflare API
- `CloudflareRequest` - Request format
- `CloudflareResponse` - Response format
- `CfMessage`, `CfUsage`, `CfResult` - Response parts
- `OpenAiChatResponse` - Converted OpenAI format
- `CloudflareClientBuilder` - Builder pattern

**Key Functions:**
- `CloudflareClient::builder()` - Create client with builder
- `is_cloudflare_model()` - Detect @cf/ models
- `chat()` - Non-streaming chat
- `chat_streaming()` - Streaming chat
- `to_openai_response()` - Convert to OpenAI format

**API Endpoint:** `POST https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/{model}`

**Test Coverage:** Partial (builder and detection only)

#### `infrastructure/cache/file_cache.rs`
**Purpose:** File-based cache for model list

**Key Functions:**
- `refresh_models_cache()` - Fetch and cache model list
- `run_cache_monitor()` - Background refresh task (hourly)

**Note:** Not tested

---

### Shared Utilities

#### `shared/app_state.rs`
**Purpose:** Application state shared across handlers

**Fields:**
- `client` - AWS Bedrock runtime client
- `mgmt_client` - AWS Bedrock management client
- `logger` - ClickHouse logger
- `file_cache` - Model list cache
- `auth` - Authentication service
- `cloudflare_client` - Optional Cloudflare client

#### `shared/extractors.rs`
**Purpose:** HTTP header extractors

**Functions:**
- `extract_bearer_token()` - Extract Bearer token from Authorization header

**Note:** Not used in current implementation

---

## Request/Response Flow

### Chat Completion Flow

```
1. Client sends POST /v1/chat/completions
          │
          ▼
2. chat_handler receives ChatRequest
          │
          ▼
3. Check model prefix (@cf/ → Cloudflare, else → Bedrock)
          │
     ┌────┴────┐
     ▼         ▼
Cloudflare   Bedrock
     │         │
     ▼         ▼
4a. Call      4b. Call
Cloudflare   build_converse_payload()
API          │
     │         ▼
     │    5. Call Bedrock
     │    Converse API
     │         │
     └────┬────┘
          ▼
6. Normalize response to OpenAI format
          │
          ▼
7. Log usage to ClickHouse
          │
          ▼
8. Return JSON response
```

### Model Routing

| Model Prefix | Provider | Handler |
|--------------|----------|---------|
| `@cf/` | Cloudflare Workers AI | cloudflare client |
| `anthropic.claude-*` | AWS Bedrock | converse API |
| `deepseek.r1-*` | AWS Bedrock | invoke API (reasoning) |
| `cohere.command-*` | AWS Bedrock | invoke API |
| `ai21.j2-*` | AWS Bedrock | invoke API |
| `mistral.*` | AWS Bedrock | invoke API |
| `meta.llama-*` | AWS Bedrock | invoke API |
| `amazon.titan-*` | AWS Bedrock | converse API |
| `amazon.nova-*` | AWS Bedrock | converse API |

---

## Environment Variables

| Variable | Required | Default | Purpose |
|----------|----------|---------|---------|
| `AWS_REGION` | No | us-east-1 | AWS region |
| `DEFAULT_API_KEY` | Yes | - | API authentication |
| `DB_API_KEY_LOCATION_SQLITE` | No | api_keys.db | SQLite database path |
| `CLOUDFLARE_ACCOUNT_ID` | No | - | Cloudflare account |
| `CLOUDFLARE_API_TOKEN` | No | - | Cloudflare API token |
| `SERVER_HOST` | No | 0.0.0.0 | Bind address |
| `SERVER_PORT` | No | 3001 | Bind port |
| `CLICKHOUSE_URL` | No | http://127.0.0.1:8123 | ClickHouse URL |
| `CLICKHOUSE_USER` | No | default | ClickHouse user |
| `CLICKHOUSE_PASSWORD` | No | password | ClickHouse password |
| `CLICKHOUSE_DB` | No | default | ClickHouse database |

---

## Dependencies

### Runtime

| Dependency | Purpose |
|------------|---------|
| axum | HTTP framework |
| tokio | Async runtime |
| serde/serde_json | Serialization |
| aws-sdk-bedrock | AWS Bedrock management |
| aws-sdk-bedrockruntime | AWS Bedrock runtime |
| reqwest | HTTP client (Cloudflare) |
| clickhouse | ClickHouse client |
| rusqlite | SQLite client |
| arc-swap | Thread-safe shared state |
| chrono | Date/time |
| uuid | Request ID generation |
| tracing | Logging |

### Development

| Dependency | Purpose |
|------------|---------|
| mockito | HTTP mocking for tests |
| tokio-test | Async test utilities |
| cargo-tarpaulin | Code coverage |
