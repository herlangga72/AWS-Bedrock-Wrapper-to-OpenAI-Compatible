# API Specification

OpenAI-compatible chat completions API wrapper for AWS Bedrock and Cloudflare Workers AI.

**Base URL:** `http://localhost:8080`

---

## Table of Contents

1. [Endpoints](#1-endpoints)
2. [Request/Response Formats](#2-requestresponse-formats)
3. [Features](#3-features)

---

## 1. Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `POST` | `/v1/chat/completions` | `openai_chat_handler` | OpenAI-compatible chat |
| `POST` | `/openai/v1/chat/completions` | `openai_chat_handler` | OpenAI-compatible chat (alt) |
| `POST` | `/claude/v1/messages` | `claude_messages_handler` | Anthropic-native chat |
| `GET` | `/v1/models` | `list_models_handler` | List available models |
| `POST` | `/v1/embeddings` | `handle_embeddings` | Text embeddings |

---

## 2. Request/Response Formats

### `POST /v1/chat/completions`

OpenAI-compatible chat completions.

#### Request Headers

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | `application/json` |
| `Authorization` | Yes | `Bearer <api_key>` |
| `x-openwebui-thinking` | No | Enable thinking (`true`/`1`) |
| `x-openwebui-reasoning` | No | Include reasoning (`true`/`1`) |
| `x-openwebui-user-email` | No | User email override |
| `x-openwebui-message-id` | No | Message ID override |

#### Request Body

```json
{
  "model": "anthropic.claude-sonnet-4-5-20250929-v1:0",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello"}
  ],
  "stream": false,
  "temperature": 1.0,
  "top_p": 0.9,
  "max_tokens": 4096,
  "top_k": 250,
  "frequency_penalty": 0.0,
  "presence_penalty": 0.0,
  "tools": [...],
  "tool_choice": "auto",
  "thinking": {"enabled": true, "budget_tokens": 8000}
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `model` | string | **Yes** | — | Model ID |
| `messages` | array | **Yes** | — | Array of message objects |
| `stream` | boolean | No | `false` | Enable SSE streaming |
| `temperature` | float | No | model-dependent | Sampling temperature (0.0–2.0) |
| `top_p` | float | No | model-dependent | Nucleus sampling |
| `max_tokens` | integer | No | model-dependent | Max tokens to generate |
| `top_k` | integer | No | — | Top-k sampling (Claude, Cohere, Mistral) |
| `frequency_penalty` | float | No | — | (Cohere, AI21) |
| `presence_penalty` | float | No | — | (Cohere, AI21) |
| `tools` | array | No | — | Tool definitions |
| `tool_choice` | string/object | No | `"auto"` | Tool choice policy |
| `thinking` | object | No | — | Claude extended thinking |
| `stop_sequences` | string | No | — | Stop sequence (non-Converse models) |

#### Messages

```json
{"role": "user", "content": "Hello"}
{"role": "assistant", "content": "Hi"}
{"role": "system", "content": "You are helpful"}
{"role": "tool", "content": "{}", "tool_call_id": "tool_001"}
```

Roles: `user`, `assistant`, `system`, `tool`

#### Response (Non-Streaming)

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "anthropic.claude-sonnet-4-5-20250929-v1:0",
  "choices": [{
    "index": 0,
    "message": {"role": "assistant", "content": "Hello! How can I help you?"},
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30,
    "ttft_ms": 50,
    "latency_ms": 150,
    "tokens_per_second": 13.33
  }
}
```

#### Response (Streaming)

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"}}]}
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"!"}}]}
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","choices":[],"usage":{"input_tokens":10,"output_tokens":20,"total_tokens":30}}
data: [DONE]
```

---

### `POST /claude/v1/messages`

Anthropic-native messages endpoint.

#### Request Headers

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | `application/json` |
| `Authorization` | Yes | `Bearer <api_key>` |
| `anthropic-version` | Yes | `2023-06-01` |

#### Request Body

```json
{
  "model": "claude-3-5-sonnet-20241022",
  "messages": [{"role": "user", "content": [{"type": "text", "text": "Hello"}]}],
  "max_tokens": 4096,
  "stream": false,
  "temperature": 1.0,
  "top_p": 0.9,
  "system": "You are helpful",
  "thinking": {"type": "enabled", "budget_tokens": 8000}
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | **Yes** | Anthropic model name (e.g. `claude-3-5-sonnet-20241022`) |
| `messages` | array | **Yes** | Array of messages |
| `max_tokens` | integer | **Yes** | Max tokens to generate |
| `stream` | boolean | No | Streaming (default: `false`) |
| `system` | string | No | System prompt |
| `temperature` | float | No | Sampling temperature |
| `top_p` | float | No | Nucleus sampling |
| `stop_sequences` | array | No | Stop sequences |
| `thinking` | object | No | Thinking configuration |

#### Response (Non-Streaming)

```json
{
  "id": "msg_abc123",
  "type": "message",
  "role": "assistant",
  "content": [{"type": "text", "text": "Hello!"}],
  "model": "claude-3-5-sonnet-20241022",
  "stop_reason": "end_turn",
  "usage": {"input_tokens": 10, "output_tokens": 20}
}
```

#### Response (Streaming)

```
event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":20,"total_tokens":20}}
```

---

### `GET /v1/models`

List available models.

```json
{
  "object": "list",
  "data": [
    {"id": "anthropic.claude-sonnet-4-5-20250929-v1:0", "object": "model", "created": 0, "owned_by": "anthropic"},
    {"id": "@cf/meta/llama-3.1-8b-instruct", "object": "model", "created": 0, "owned_by": "cloudflare"}
  ]
}
```

---

## 3. Features

### Tools / Function Calling

#### Tool Definition

```json
{
  "tools": [{
    "type": "function",
    "function": {
      "name": "get_weather",
      "description": "Get weather for a city",
      "parameters": {
        "type": "object",
        "properties": {"city": {"type": "string"}},
        "required": ["city"]
      }
    }
  }]
}
```

#### Tool Choice

| Value | Description |
|-------|-------------|
| `"auto"` | Model decides |
| `"none"` | Disable tools |
| `{"function": {"name": "get_weather"}}` | Force specific function |

#### Tool Call Response

```json
{
  "role": "assistant",
  "content": "Let me check...",
  "tool_calls": [{
    "id": "tool_001",
    "type": "function",
    "function": {"name": "get_weather", "arguments": "{\"city\": \"Tokyo\"}"}
  }]
}
```

Tool result:
```json
{"role": "tool", "content": "{\"temperature\": 22}", "tool_call_id": "tool_001"}
```

---

### Claude Extended Thinking

**Supported models:** Claude Opus 4.x, Sonnet 4.x, Haiku 4.x, 3.5 Sonnet, 3.7 Sonnet

```json
{"thinking": {"enabled": true, "budget_tokens": 8000}}
```

| Field | Type | Description |
|-------|------|-------------|
| `enabled` | boolean | Enable/disable |
| `budget_tokens` | integer | Tokens for thinking (1024–200000) |

**OpenAI format response:**
```json
{"content": [
  {"type": "thinking", "thinking": "Let me work through this..."},
  {"type": "text", "text": "The answer is 42."}
]}
```

**Anthropic format response:**
```json
{"content": [
  {"type": "thinking", "thinking": "Let me work through this..."},
  {"type": "text", "text": "The answer is 42."}
]}
```

---

### DeepSeek R1 Reasoning

**Supported models:** DeepSeek R1 series

```json
{"content": [
  {"type": "reasoning", "reasoning_text": "Let me think..."},
  {"type": "text", "text": "The answer is 42."}
]}
```

---

### Authentication

API keys stored in SQLite with SHA256 hashing.

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer <api_key>" \
  -H "Content-Type: application/json" \
  -d '{"model": "...", "messages": [...]}'
```

---

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AWS_REGION` | `us-east-1` | AWS region for Bedrock |
| `SERVER_PORT` | `8080` | HTTP server port |
| `DEFAULT_API_KEY` | — | Default API key (required) |
| `DB_API_KEY_LOCATION_SQLITE` | `api_keys.db` | SQLite database path |
| `CLICKHOUSE_URL` | `localhost:8123` | ClickHouse server URL |
| `CLICKHOUSE_USER` | `default` | ClickHouse user |
| `CLICKHOUSE_PASSWORD` | `""` | ClickHouse password |
| `CLICKHOUSE_DB` | `default` | ClickHouse database |
| `CLICKHOUSE_BATCH_SIZE` | `100` | Batch size for logging |
| `CLICKHOUSE_FLUSH_INTERVAL_SECS` | `5` | Flush interval seconds |
| `CLOUDFLARE_ACCOUNT_ID` | — | Cloudflare account ID |
| `CLOUDFLARE_API_TOKEN` | — | Cloudflare API token |
| `REQUEST_TIMEOUT_CHAT` | `120` | Chat timeout (seconds) |
| `REQUEST_TIMEOUT_THINKING` | `300` | Thinking timeout |
| `REQUEST_TIMEOUT_CLOUDFLARE` | `60` | Cloudflare timeout |
