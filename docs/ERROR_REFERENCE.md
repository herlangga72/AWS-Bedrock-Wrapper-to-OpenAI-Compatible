# Error Reference

Find errors fast by message or HTTP status, then trace to source file:line.

---

## Quick Lookup

### By Error Message

| Error Message (substring) | HTTP Status | Likely Source | See |
|-------------------------|-------------|---------------|-----|
| `Invalid API key` | 401 | `auth/service.rs:authenticate()` | [Trace](#401-invalid-api-key) |
| `Missing API Key` | 401 | Handler (no `Authorization` header) | [Trace](#401-missing-api-key) |
| `Forbidden` | 403 | `auth/service.rs:authenticate()` | [Trace](#403-forbidden) |
| `model field cannot be empty` | 400 | `chat_handler.rs:44` | [Trace](#400-model-field-cannot-be-empty) |
| `messages array cannot be empty` | 400 | `chat_handler.rs:49` | [Trace](#400-messages-array-cannot-be-empty) |
| `temperature must be between` | 400 | `chat_handler.rs:53-59` | [Trace](#400-temperature-out-of-range) |
| `role must be` | 400 | `chat_handler.rs:66` | [Trace](#400-unrecognized-role) |
| `Bedrock Error:` | 500 | `chat_handler.rs:387` (non-stream) | [Trace](#500-bedrock-error) |
| `Stream failed` | 500 | `chat_handler.rs:263` (stream) | [Trace](#500-stream-failed) |
| `Request timeout` | 500 | `chat_handler.rs:389` | [Trace](#500-request-timeout) |
| `Thinking request failed` | 500 | `messages_handler.rs:296` | [Trace](#500-thinking-request-failed) |
| `Cloudflare API error` | 500 | `cloudflare/client.rs:120` | [Trace](#500-cloudflare-api-error) |
| `Model is currently unavailable` | 503 | `cache/file_cache.rs` | [Trace](#503-model-is-currently-unavailable) |

### By HTTP Status

| Status | Code | Source | See |
|--------|------|--------|-----|
| `400` | `missing_field` | `chat_handler.rs:44-74` | [Trace](#400-errors) |
| `401` | `invalid_api_key` | `auth/service.rs:167` | [Trace](#401-invalid-api-key) |
| `401` | `missing_api_key` | Handler | [Trace](#401-missing-api-key) |
| `403` | `forbidden` | `auth/service.rs:167` | [Trace](#403-forbidden) |
| `500` | `backend_error` | `chat_handler.rs:387` | [Trace](#500-bedrock-error) |
| `500` | `internal_error` | `error_response()` | [Trace](#500-internal-error) |
| `503` | `service_unavailable` | `cache/file_cache.rs` | [Trace](#503-model-is-currently-unavailable) |

---

## Trace Guides

### `401 Invalid API Key`

**Source:** `auth/service.rs:167` — `Authentication::authenticate()`

```
Handler
└── state.auth.authenticate(bearer_token)
        │
        ├── key.len() < 8? → AuthError::Forbidden immediately
        │
        ├── hash_key() → SHA256(key) → 64-char hex
        │
        ├── extract_prefix() → hash[..8]
        │
        └── SQLite: SELECT email, is_active
                FROM api_keys
                WHERE key_hash = ? AND key_prefix = ? AND is_active = 1

Failure modes:
- No rows → AuthError::Forbidden (401)
- Hash mismatch → AuthError::Forbidden (401)
- is_active = 0 → AuthError::Forbidden (401)
- DB error → AuthError::DbError
```

**Debug tip:** Add to `auth/service.rs:175`:
```rust
tracing::debug!("Auth: prefix={}, hash={}", key_prefix, key_hash);
```

**Common causes:**
1. Wrong API key format (should be `sk-xxx...` with 8+ chars)
2. Key not registered in SQLite
3. Key deactivated (`is_active = 0`)

---

### `401 Missing API Key`

**Source:** Handler (no `Authorization` header present)

```
Handler
└── TypedHeader(Authorization<Bearer>) → None → return 401

Fix: Include header: Authorization: Bearer <api_key>
```

---

### `403 Forbidden`

**Source:** `auth/service.rs:167` — same as 401, just returned differently

**Check:** Is `is_active = 0` for this API key in `api_keys` table?

```sql
SELECT email, is_active FROM api_keys WHERE email = '<email>';
```

---

### `400` Errors

#### "model field cannot be empty"

**Source:** `chat_handler.rs:44-47`
```rust
if req.model.is_empty() {
    return Err("model field cannot be empty".to_string());
}
```

**Fix:** Provide non-empty `model` field in request.

---

#### "messages array cannot be empty"

**Source:** `chat_handler.rs:49-51`
```rust
if req.messages.is_empty() {
    return Err("messages array cannot be empty".to_string());
}
```

**Fix:** Provide at least one message in `messages` array.

---

#### "temperature must be between 0.0 and 2.0"

**Source:** `chat_handler.rs:53-59`

**Valid range:** OpenAI spec requires 0.0 – 2.0

**Check:** `shared/constants.rs` for `MIN_TEMPERATURE` and `MAX_TEMPERATURE`

---

#### "messages[N].role must be 'user', 'assistant', or 'system'"

**Source:** `chat_handler.rs:66`

**Valid roles:** `user`, `assistant`, `system`, `tool`

**Note:** `tool` role is accepted but skipped during Bedrock Converse payload building (see `converse.rs:37`).

---

### `500 Bedrock Error`

**Source:** `chat_handler.rs:387` (non-stream), `chat_handler.rs:263` (stream)

#### Non-Stream Path

```
client.converse()
├── timeout (REQUEST_TIMEOUT_CHAT = 120s) → "Request timeout"
├── ValidationException → "Bedrock Error: ValidationException: ..."
├── AccessDeniedException → "Bedrock Error: AccessDeniedException: ..."
├── ResourceNotFoundException → "Bedrock Error: ResourceNotFoundException: ..."
├── ThrottlingException → "Bedrock Error: ThrottlingException: ..."
└── InternalServerException → "Bedrock Error: InternalServerException: ..."
```

**Check AWS:**
1. IAM permissions for `bedrock:InvokeModel` / `bedrock:Converse`
2. Model available in region (`aws bedrock list-foundation-models`)
3. Request not too large (input tokens)

#### Stream Path

```
client.converse_stream()
├── timeout → yield sse_error("Stream failed")
├── SDK error → yield sse_error("Stream failed")
└── success → SSE yield loop
```

---

### `500 Request Timeout`

**Source:** `chat_handler.rs:389` (non-stream)

**Timeout:** `REQUEST_TIMEOUT_CHAT` (default 120s in `shared/constants.rs`)

**Check:**
1. Bedrock service latency (check AWS status)
2. Input prompt too long
3. `max_tokens` too high
4. Network connectivity to AWS

---

### `500 Thinking Request Failed`

**Source:** `messages_handler.rs:296` (Anthropic `/claude/v1/messages` with thinking)

**Path:** `non_stream_thinking()` → `client.invoke_model()`

**Check:**
1. Model supports thinking (Claude 3.5+/4.x only)
2. `budget_tokens` not too high (max ~200000)
3. AWS permissions for `bedrock:InvokeModel`

---

### `500 Cloudflare API Error`

**Source:** `cloudflare/client.rs:120`

```
client.chat() / client.chat_streaming()
├── HTTP status non-2xx → format "Cloudflare API error {status}: {body}"
├── JSON parse fail → "error deserializing response"
└── reqwest error → "error making request: {e}"
```

**Common Cloudflare errors:**

| Status | Cause | Fix |
|--------|-------|-----|
| `400` | Invalid request body or model | Check request format |
| `403` | Bad `CLOUDFLARE_API_TOKEN` | Verify env var |
| `403` | Account ID mismatch | Check `CLOUDFLARE_ACCOUNT_ID` |
| `404` | Model not available | Check `@cf/` model name |
| `429` | Rate limited | Implement backoff |

**Debug:**
```bash
echo $CLOUDFLARE_ACCOUNT_ID
echo $CLOUDFLARE_API_TOKEN  # Should exist and be valid
```

---

### `500 Internal Error`

**Source:** Various `error_response()` calls with `StatusCode::INTERNAL_SERVER_ERROR`

**Causes:**
1. JSON serialization failure in response building
2. Unexpected `None` values when building responses
3. Panics (should be caught by Axum)

**Debug:** Check server logs for stack trace.

---

### `503 Model Is Currently Unavailable`

**Source:** `cache/file_cache.rs` — model list cache empty

```
list_models_handler
└── state.file_cache.load()
        └── cache.get("bedrock_models") → None → 503

Cache population (background monitor):
└── refresh_models_cache()
        ├── list_foundation_models() → AWS API call
        └── ArcSwap::store() → update cache
```

**Check:**
1. Is Bedrock client initialized? (`AWS_REGION`, `AWS_PROFILE`, credentials)
2. Is `run_cache_monitor()` running? (check for "Starting cache monitor" log)
3. Are AWS credentials valid? (`aws sts get-caller-identity`)

**Manual refresh:** Not exposed via API. Restart server to trigger cache refresh.

---

## Error Response Format

```json
{
  "error": {
    "message": "Descriptive error message",
    "type": "authentication_error|invalid_request_error|api_error",
    "code": "invalid_api_key|missing_field|backend_error|..."
  }
}
```

| Type | Used For |
|------|----------|
| `authentication_error` | 401/403 from auth |
| `invalid_request_error` | 400 from validation |
| `api_error` | 500/503 from backend |

---

## Panic Handling

All handlers wrapped by Axum. Panics become 500 Internal Server Error with generic message.

**To get stack trace:** Check server stderr/logs.

```rust
// In main.rs, panics are caught by:
axum::serve(listener, app)
```

**Note:** SSE streams (`stream_converse`, etc.) don't gracefully handle panics in the async block. Check logs if stream terminates unexpectedly.

---

## Logging

### Key Log Points

| Log Level | Where | What |
|-----------|-------|------|
| `error` | `chat_handler.rs:386` | Bedrock SDK error |
| `error` | `chat_handler.rs:263` | Stream timeout/failure |
| `error` | `messages_handler.rs:151` | Anthropic converse error |
| `error` | `messages_handler.rs:296` | Thinking invoke failed |
| `error` | `cloudflare/client.rs` | Cloudflare API error |
| `warn` | `converse.rs:38` | Skipping unknown message role |
| `info` | `auth/service.rs:81` | Schema migration |
| `debug` | `auth/service.rs` | Auth attempt (add manually) |
| `info` | `file_cache.rs` | Cache refresh |

### Trace ID

- `x-openwebui-message-id` header passed through for correlation
- If not provided, generated via `Uuid::new_v4()`
- Check logs for this ID to correlate requests
