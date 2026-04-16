# Architecture Documentation

**Last Updated:** 2026-04-15

---

## Table of Contents

1. [Module Structure](#1-module-structure)
2. [Flame Graphs](#2-flame-graphs)
3. [Function Reference](#3-function-reference)
4. [Route → Handler Map](#4-route--handler-map)

---

## 1. Module Structure

```
src/
├── main.rs                              # Entry point, Router setup
├── lib.rs                               # Library root exports
│
├── domain/                              # Business logic
│   ├── auth/
│   │   ├── service.rs                   # Authentication impl
│   │   └── types.rs                    # ApiKey, AuthError
│   ├── chat/
│   │   ├── types.rs                    # ChatRequest, Message, Content, ToolChoice
│   │   ├── anthropic_types.rs          # AnthropicMessagesRequest, AnthropicResponse
│   │   └── capabilities.rs             # ModelCapabilities registry
│   ├── embedding/
│   │   ├── builder.rs                  # NovaRequest builder
│   │   └── types.rs                    # OpenAiEmbeddingRequest/Response
│   └── logging/
│       ├── logger.rs                   # ClickHouseLogger
│       └── types.rs                    # LogEntry
│
├── infrastructure/                      # External integrations
│   ├── aws/bedrock/
│   │   ├── converse.rs                 # build_converse_payload, build_tool_config
│   │   ├── invoke.rs                   # invoke_thinking_model, build_thinking_request
│   │   └── anthropic_translator.rs     # anthropic_model_to_bedrock, AnthropicConversePayload
│   ├── cache/
│   │   └── file_cache.rs               # refresh_models_cache, run_cache_monitor
│   └── cloudflare/
│       └── client.rs                   # CloudflareClient, chat, chat_streaming
│
├── interface/                           # HTTP handlers
│   ├── openai/
│   │   ├── completions.rs              # openai_chat_handler
│   │   └── chat/
│   │       ├── chat_handler.rs         # chat_handler, stream_converse, non_stream
│   │       ├── thinking_handler.rs      # chat_with_thinking_handler, thinking_handler
│   │       └── reasoning_handler.rs    # chat_with_reasoning_handler, reasoning_handler
│   ├── anthropic/
│   │   └── messages_handler.rs         # claude_messages_handler
│   └── common/
│       ├── models/mod.rs               # list_models_handler
│       └── embedding/mod.rs             # handle_embeddings
│
└── shared/
    ├── app_state.rs                    # AppState struct
    ├── constants.rs                    # ALL_* constants
    ├── errors.rs                       # error_response(), sse_error()
    └── logging.rs                      # spawn_log()
```

---

## 2. Flame Graphs

### `POST /v1/chat/completions` (OpenAI)

```
openai_chat_handler (completions.rs:22)
└── chat_with_thinking_handler (thinking_handler.rs:63)
        │
        ├── [reasoning model?]──→ chat_with_reasoning_handler (reasoning_handler.rs:72)
        │                            └── reasoning_handler (reasoning_handler.rs:81)
        │                                ├── build_converse_payload (converse.rs:19)
        │                                │       └── map_openai_params (capabilities.rs:391)
        │                                └── client.converse_stream() / client.converse()
        │
        ├── [thinking model + enabled?]──→ thinking_handler (thinking_handler.rs:112)
        │                                     ├── parse_thinking_params (invoke.rs)
        │                                     ├── build_thinking_request (invoke.rs)
        │                                     └── invoke_thinking_model (invoke.rs)
        │                                             └── client.invoke_model() → Blob
        │
        └── [standard model]──→ chat_handler (chat_handler.rs:148)
                                  │
                                  ├── [Cloudflare @cf/?]──→ stream_cloudflare / non_stream_cloudflare
                                  │                            └── CloudflareClient::chat() / chat_streaming()
                                  │                                    └── HTTP POST to Cloudflare API
                                  │
                                  └── [Bedrock]──→ stream_converse / non_stream (chat_handler.rs:234/363)
                                           ├── build_converse_payload (converse.rs:19)
                                           │       └── map_openai_params (capabilities.rs:391)
                                           └── client.converse_stream() / client.converse()
                                                   └── aws_sdk_bedrockruntime

spawn_log (on completion)
└── ClickHouseLogger::log_usage
        └── tx.try_send(LogEntry) → async batch → clickhouse insert
```

---

### `POST /claude/v1/messages` (Anthropic Native)

```
claude_messages_handler (messages_handler.rs:49)
├── state.auth.authenticate(bearer_token)
│       └── Authentication::authenticate (auth/service.rs:167)
│
├── anthropic_model_to_bedrock (anthropic_translator.rs:39)
│       └── ANTHROPIC_MODEL_CACHE lookup
│
├── [thinking enabled?]──→ nonstream_thinking / stream_thinking
│       ├── build_thinking_request_from_anthropic (anthropic_translator.rs:194)
│       └── client.invoke_model() → aws_sdk_bedrockruntime
│               └── Blob::new(body_json)
│
└── [standard]──→ nonstream / stream_converse
        └── AnthropicConversePayload::from_anthropic_request (anthropic_translator.rs:77)
                └── InferenceConfiguration::builder()
                        └── client.converse() / client.converse_stream()
```

---

### `GET /v1/models`

```
list_models_handler (common/models/mod.rs:10)
└── state.file_cache.load()
        └── ArcSwap::load() → bytes.clone() → Response

Cache miss (background):
└── refresh_models_cache (cache/file_cache.rs)
        ├── list_foundation_models() → aws_sdk_bedrock
        └── ArcSwap::store()
```

---

### Authentication Flow (All Protected Endpoints)

```
Handler (any)
├── TypedHeader(Authorization<Bearer>)
│       └── bearer.token() → "sk-xxx..."
│
└── state.auth.authenticate(bearer_token)
        │
        ├── key.len() < 8? → AuthError::Forbidden immediately
        │
        ├── hash_key() → Sha256::new() → 64-char hex
        │
        ├── extract_prefix() → hash[..8]
        │
        ├── SQLite: SELECT email, is_active
        │       FROM api_keys
        │       WHERE key_hash=? AND key_prefix=? AND is_active=1
        │
        └── [success] → UPDATE last_used = datetime('now')

spawn_log (on completion)
└── ClickHouseLogger::log_usage
        └── tx.try_send(LogEntry)
                └── async batch → ClickHouse insert
```

---

## 3. Function Reference

### Interface Layer

| File | Function | Line | Description |
|------|----------|------|-------------|
| `openai/completions.rs` | `openai_chat_handler` | 22 | Entry point — delegates to thinking handler |
| `openai/chat/chat_handler.rs` | `chat_handler` | 148 | Routes Cloudflare vs Bedrock, auth, validation |
| | `validate_chat_request` | 44 | Validates model, messages, temperature, roles |
| | `normalize_model_name` | 78 | Rewrites `@cf/` and `bedrock/` prefixes |
| | `stream_converse` | 234 | Bedrock SSE streaming |
| | `non_stream` | 363 | Bedrock non-streaming |
| | `stream_cloudflare` | 462 | Cloudflare SSE streaming |
| | `non_stream_cloudflare` | 534 | Cloudflare non-streaming |
| `openai/chat/thinking_handler.rs` | `chat_with_thinking_handler` | 63 | Routes reasoning/thinking/standard |
| | `thinking_handler` | 112 | Claude extended thinking handler |
| | `non_stream_thinking` | 153 | Non-stream thinking response |
| | `non_stream_as_stream` | 250 | SSE wrapper for thinking |
| `openai/chat/reasoning_handler.rs` | `chat_with_reasoning_handler` | 72 | DeepSeek R1 routing |
| | `reasoning_handler` | 81 | Reasoning handler logic |
| | `non_stream_reasoning` | 143 | Non-stream reasoning |
| | `stream_reasoning` | 276 | Stream reasoning |
| `anthropic/messages_handler.rs` | `claude_messages_handler` | 49 | `/claude/v1/messages` entry |
| | `non_stream` | 130 | Non-stream Converse |
| | `stream_converse` | 193 | Stream Converse |
| | `non_stream_thinking` | 273 | Non-stream thinking (Invoke API) |
| | `stream_thinking` | 378 | Stream thinking (Invoke API) |
| | `build_anthropic_response` | 494 | Build Anthropic response format |
| `common/models/mod.rs` | `list_models_handler` | 10 | `GET /v1/models` |

### Domain Layer

| File | Function | Line | Description |
|------|----------|------|-------------|
| `auth/service.rs` | `Authentication::new` | 15 | Initialize SQLite |
| | `register_key` | 139 | Add new API key |
| | `register_key_with_name` | 144 | Add key with name |
| | `authenticate` | 167 | Validate key — returns email or error |
| | `list_keys` | 197 | List all keys |
| | `deactivate` | 227 | Disable key by email |
| | `reactivate` | 239 | Re-enable key |
| | `delete` | 251 | Delete key |
| | `hash_key` | 127 | SHA256 of API key |
| | `extract_prefix` | 134 | First 8 chars of hash |
| `chat/capabilities.rs` | `get_model_capabilities` | 95 | Lookup model config |
| | `map_openai_params` | 391 | Map OpenAI params to provider format |
| `chat/types.rs` | `ChatRequest`, `Message`, `Content`, `ToolChoice` | — | Request/response types |
| `embedding/builder.rs` | `NovaRequest::new` | 32 | Build Nova embedding request |
| `logging/logger.rs` | `ClickHouseLogger::new` | 17 | Initialize logger |
| | `log_usage` | 82 | Fire-and-forget usage log |

### Infrastructure Layer

| File | Function | Line | Description |
|------|----------|------|-------------|
| `aws/bedrock/converse.rs` | `build_converse_payload` | 19 | Build Converse API payload |
| | `build_tool_config` | 120 | Format tools for system prompt |
| | `extract_text_from_content` | 158 | Extract text from Content enum |
| `aws/bedrock/invoke.rs` | `invoke_thinking_model` | — | Invoke API for thinking |
| | `build_thinking_request` | — | Build Invoke API body |
| `aws/bedrock/anthropic_translator.rs` | `anthropic_model_to_bedrock` | 39 | Model name translation |
| | `ConversePayload::from_anthropic_request` | 77 | Anthropic → Bedrock payload |
| | `build_thinking_request_from_anthropic` | 194 | Anthropic → Invoke body |
| `cloudflare/client.rs` | `CloudflareClient::builder` | 75 | Builder pattern |
| | `CloudflareClient::chat` | 91 | Non-streaming chat |
| | `CloudflareClient::chat_streaming` | 129 | Streaming chat |
| `cache/file_cache.rs` | `refresh_models_cache` | — | Refresh model list |
| | `run_cache_monitor` | — | Background cache monitor |

### Shared Layer

| File | Function | Line | Description |
|------|----------|------|-------------|
| `errors.rs` | `error_response` | — | HTTP error response |
| | `sse_error` | — | SSE error event |
| `logging.rs` | `spawn_log` | — | Fire-and-forget logger |
| `app_state.rs` | `AppState` | — | Shared state struct |

---

## 4. Route → Handler Map

| Model ID Pattern | Vendor | Handler | API |
|-----------------|--------|---------|-----|
| `@cf/*` | Cloudflare | `chat_handler` → `stream_cloudflare` | HTTP REST |
| `anthropic.claude*` + thinking | Bedrock | `thinking_handler` → `invoke_thinking_model` | Invoke |
| `anthropic.claude*` | Bedrock | `chat_handler` → `stream_converse` | Converse |
| `deepseek.r1*` | Bedrock | `reasoning_handler` → `stream_converse` | Converse |
| `cohere.command*` | Bedrock | `chat_handler` → `stream_converse` | Invoke |
| `ai21.j2*` | Bedrock | `chat_handler` → `stream_converse` | Invoke |
| `mistral*` | Bedrock | `chat_handler` → `stream_converse` | Invoke |
| `meta.llama*` | Bedrock | `chat_handler` → `stream_converse` | Invoke |
| `amazon.titan*` | Bedrock | `chat_handler` → `stream_converse` | Converse |
| `amazon.nova*` | Bedrock | `chat_handler` → `stream_converse` | Converse |
| `claude-3-5-sonnet-*` etc. | Bedrock | `claude_messages_handler` | Converse/Invoke |

---

## Supported Models

### AWS Bedrock

| Provider | Model Prefix | API | Tools | Thinking | Reasoning |
|----------|-------------|-----|-------|----------|-----------|
| **Anthropic Claude** | `anthropic.claude-*` | Converse | ✅ | ✅ | ❌ |
| **DeepSeek** | `deepseek.r1*` | Converse | ❌ | ❌ | ✅ |
| **Cohere** | `cohere.command-*` | Invoke | ✅ | ❌ | ❌ |
| **AI21** | `ai21.j2-*` | Invoke | ✅ | ❌ | ❌ |
| **Mistral** | `mistral.*` | Invoke | ✅ | ❌ | ❌ |
| **Meta Llama** | `meta.llama*` | Invoke | ✅ | ❌ | ❌ |
| **Amazon Titan** | `amazon.titan-*` | Converse | ❌ | ❌ | ❌ |
| **Amazon Nova** | `amazon.nova-*` | Converse | ❌ | ❌ | ❌ |

### Cloudflare Workers AI

| Provider | Model Prefix | Tools | Max Tokens |
|----------|-------------|-------|------------|
| **Meta** | `@cf/meta/*` | ✅ | 256 |
| **DeepSeek** | `@cf/deepseek-ai/*` | ✅ | 256 |
| **Mistral** | `@cf/mistral/*` | ✅ | 256 |
| **Google** | `@cf/google/*` | ✅ | 256 |

---

## Parameter Support

### AWS Bedrock

| Parameter | Claude | DeepSeek | Cohere | AI21 | Mistral | Llama | Titan | Nova |
|-----------|--------|----------|--------|------|---------|-------|-------|------|
| `temperature` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `top_p` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `max_tokens` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `stop_sequences` | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `top_k` | ✅ | ❌ | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `frequency_penalty` | ❌ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `presence_penalty` | ❌ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `tools` | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `tool_choice` | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `thinking` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

### Cloudflare

| Parameter | Support |
|-----------|---------|
| `temperature` | ✅ |
| `top_p` | ✅ |
| `max_tokens` | ✅ |
| `top_k` | ❌ |
| `frequency_penalty` | ❌ |
| `presence_penalty` | ❌ |
| `tools` | ✅ |
| `tool_choice` | ✅ |

---

## Default Values

### AWS Bedrock

| Model | temperature | top_p | max_tokens | top_k |
|-------|-------------|-------|------------|-------|
| Claude | 1.0 | 0.9 | 4096 | 250 |
| DeepSeek | 0.7 | 0.9 | 8192 | — |
| Cohere | 0.3 | 0.75 | 2048 | 250 |
| AI21 | 0.5 | 0.5 | 2048 | — |
| Mistral | 0.5 | 0.9 | 4096 | 50 |
| Llama | 0.5 | 0.9 | 512 | — |
| Titan | 0.5 | 0.9 | 2048 | — |
| Nova | 0.7 | 0.9 | 4096 | — |

### Cloudflare

| Model | temperature | top_p | max_tokens |
|-------|-------------|-------|------------|
| All `@cf/*` | 0.7 | 0.9 | 256 |

---

## Anthropic Model Name Mapping

| Anthropic Name | Bedrock Model ID |
|----------------|------------------|
| `claude-3-5-sonnet-20240620` | `anthropic.claude-3-5-sonnet-v1:0` |
| `claude-3-5-sonnet-20241022` | `anthropic.claude-3-5-sonnet-v2:0` |
| `claude-3-5-haiku-20241022` | `anthropic.claude-3-5-haiku-v1:0` |
| `claude-3-7-sonnet-20250620` | `anthropic.claude-3-7-sonnet-v1:0` |
| `claude-sonnet-4-5-20250929` | `anthropic.claude-sonnet-4-5-20250929-v1:0` |
| `claude-sonnet-4-6-20250514` | `anthropic.claude-sonnet-4-6-v1:0` |
| `claude-opus-4-5-20261111` | `anthropic.claude-opus-4-5-20261111-v1:0` |
| `claude-opus-4-6-20250514` | `anthropic.claude-opus-4-6-v1:0` |
| `claude-haiku-4-5-20251001` | `anthropic.claude-haiku-4-5-20251001-v1:0` |
| `claude-haiku-4-6-20250514` | `anthropic.claude-haiku-4-6-v1:0` |
