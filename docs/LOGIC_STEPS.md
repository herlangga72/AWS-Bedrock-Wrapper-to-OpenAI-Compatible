# How the Logic Works - Step by Step

**Last Updated:** 2026-04-15

---

## Request Flow Overview

```
Client (OpenAI format)
        │
        ▼
┌────────────────────────────────────────┐
│ 1. HTTP Request received               │
│    POST /v1/chat/completions           │
│    (or /openai/v1/chat/completions)    │
└────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────┐
│ 2. Authenticate via API key            │
│    Bearer token in Authorization header│
│    Check against SQLite database       │
└────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────┐
│ 3. Model routing decision              │
│    Check model prefix → provider       │
└────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────┐
│ 4. Request → Provider-specific format  │
│    Build payload for AWS/Cloudflare    │
└────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────┐
│ 5. Call provider API                   │
│    AWS Bedrock Converse/Invoke or      │
│    Cloudflare Workers AI               │
└────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────┐
│ 6. Response → OpenAI format            │
│    Normalize back to client format     │
└────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────┐
│ 7. Log usage to ClickHouse             │
│    Async spawn, non-blocking           │
└────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────┐
│ 8. Return JSON/SSE response            │
└────────────────────────────────────────┘
```

---

## Step 1: HTTP Request Received

**File:** `src/main.rs` lines 97-121

Routes registered:
- `POST /v1/chat/completions` → `chat_with_thinking_handler`
- `POST /openai/v1/chat/completions` → `chat_with_thinking_handler`
- `POST /claude/v1/messages` → `claude_messages_handler`
- `GET /v1/models` → `list_models_handler`
- `POST /v1/embeddings` → `handle_embeddings`

```rust
let app = Router::new()
    .route("/v1/chat/completions", post(interface::chat::chat_with_thinking_handler))
    .route("/openai/v1/chat/completions", post(interface::chat::chat_with_thinking_handler))
    .route("/claude/v1/messages", post(interface::anthropic::messages_handler::claude_messages_handler))
    .route("/v1/models", get(interface::models::models_handler::list_models_handler))
    .route("/v1/embeddings", post(interface::embedding::embedding_handler::handle_embeddings))
    .with_state(state);
```

---

## Step 2: Authenticate via API Key

**File:** `src/interface/chat/chat_handler.rs` lines 160-178

```rust
let temp_user_email = match auth {
    Some(TypedHeader(Authorization(bearer))) => {
        match state.auth.authenticate(bearer.token()) {
            Ok(email) => email,
            Err(_) => return error_response(StatusCode::UNAUTHORIZED, "Invalid API Key"),
        }
    }
    None => return error_response(StatusCode::UNAUTHORIZED, "Missing API Key"),
};
```

1. Extract `Authorization: Bearer <token>` header
2. Call `state.auth.authenticate(token)` which queries SQLite `api_keys.db`
3. If valid, returns email/user identifier
4. If invalid or missing, return 401 Unauthorized

**Note:** If user_email is "chat", the system checks for Open Web UI email header (`x-openwebui-user-email`) to support multi-user Open Web UI deployments.

---

## Step 3: Model Routing Decision

**File:** `src/interface/chat/chat_handler.rs` lines 188-195

```rust
let is_cloudflare = req.model.starts_with("@cf/");
let model_name = normalize_model_name(&req.model);
let model_id = req.model.clone().replace("bedrock/", "");
```

**Routing Logic:**

| Model Prefix | Provider | Handler |
|--------------|----------|---------|
| `@cf/` | Cloudflare Workers AI | `stream_cloudflare` / `non_stream_cloudflare` |
| `anthropic.claude-*` | AWS Bedrock | `stream_converse` / `non_stream` (Converse API) |
| `deepseek.r1-*` | AWS Bedrock | reasoning handler (Invoke API) |
| `cohere.command-*` | AWS Bedrock | `non_stream` (Invoke API) |
| `ai21.j2-*` | AWS Bedrock | `non_stream` (Invoke API) |
| `mistral.*` | AWS Bedrock | `non_stream` (Invoke API) |
| `meta.llama-*` | AWS Bedrock | `non_stream` (Invoke API) |
| `amazon.titan-*` | AWS Bedrock | `stream_converse` / `non_stream` (Converse API) |
| `amazon.nova-*` | AWS Bedrock | `stream_converse` / `non_stream` (Converse API) |

**Model name normalization:**
- `@cf/meta/llama` → `cloudflare/meta/llama`
- `bedrock/anthropic.claude` → `aws/bedrock/anthropic.claude`

---

## Step 4: Request → Provider-Specific Format

### For AWS Bedrock Converse API

**File:** `src/infrastructure/bedrock/converse.rs` lines 19-81

```rust
pub fn build_converse_payload(req: &ChatRequest) -> ConversePayload {
    // 1. Extract system messages (role=system)
    for m in &req.messages {
        if m.role == "system" {
            system_blocks.push(SystemContentBlock::Text(text));
            continue; // system messages don't go into messages array
        }
        // Convert user/assistant messages
        bedrock_messages.push(BedrockMessage::builder()
            .role(conversation_role)
            .content(BContentBlock::Text(text))
            .build());
    }

    // 2. Build inference config (temperature, top_p, max_tokens)
    let config_builder = InferenceConfiguration::builder()
        .set_temperature(base_params.temperature)
        .set_top_p(base_params.top_p)
        .set_max_tokens(base_params.max_tokens.map(|m| m as i32));

    // 3. Return ConversePayload
    ConversePayload {
        system: Some(system_blocks),  // system prompt
        messages: bedrock_messages,    // conversation
        inference_config: config_builder.build(),
    }
}
```

### For Cloudflare Workers AI

**File:** `src/infrastructure/cloudflare/client.rs` lines 93-121

```rust
pub async fn chat(&self, req: ChatRequest) -> Result<CloudflareResponse, String> {
    let cf_req = CloudflareRequest {
        messages: req.messages.into_iter().map(Into::into).collect(),
        max_tokens: req.max_tokens.unwrap_or(256),
        stream: false,
        temperature: req.temperature,
    };

    let resp = self.client.post(&url)
        .header("Authorization", format!("Bearer {}", self.api_token))
        .json(&cf_req)
        .send()
        .await?;
}
```

Format conversion:
- `Message.role` → `CfMessage.role` (passthrough)
- `Content::Text(s)` → `CfMessage.content` (passthrough)
- `Content::Blocks(blocks)` → extract all `text` fields, join with `\n`

---

## Step 5: Call Provider API

### AWS Bedrock Converse API (Streaming)

**File:** `src/interface/chat/chat_handler.rs` lines 234-361

```rust
async_stream::stream! {
    let sdk_call = client.converse_stream()
        .model_id(&model_id)
        .set_messages(Some(payload.messages))
        .set_system(payload.system)
        .inference_config(payload.inference_config)
        .send();

    let mut resp = match timeout(REQUEST_TIMEOUT, sdk_call).await {
        Ok(Ok(r)) => r,
        _ => { yield sse_error("Stream failed"); return; }
    };

    while let Ok(Some(event)) = resp.stream.recv().await {
        match event {
            Out::ContentBlockDelta(delta) => {
                // Extract text delta → SSE chunk
            }
            Out::Metadata(m) => {
                // Capture usage metrics (input_tokens, output_tokens)
            }
            Out::MessageStop(stop) => {
                // Send finish_reason
            }
            _ => {}
        }
    }

    // Send usage chunk with ttft_ms, latency_ms, tokens_per_second
    yield Ok(Event::default().data("[DONE]"));
}
```

### AWS Bedrock Invoke API (Thinking)

**File:** `src/infrastructure/bedrock/invoke.rs`

Used for Claude extended thinking - different API than Converse.

```rust
pub async fn invoke_thinking_model(
    client: &RuntimeClient,
    model_id: &str,
    body: &ThinkingRequestBody,
) -> Result<ThinkingResponse, String> {
    let resp = client.invoke_model()
        .model_id(model_id)
        .body(Body::from(serde_json::to_string(body)?))
        .content_type("application/json")
        .send()
        .await?;
    // Parse response containing thinking blocks
}
```

### Cloudflare Workers AI (Streaming)

**File:** `src/infrastructure/cloudflare/client.rs` lines 125-160

```rust
pub async fn chat_streaming(&self, req: ChatRequest) -> Result<impl Stream<Item = Result<String, String>>, String> {
    let resp = self.client.post(&url)
        .header("Authorization", format!("Bearer {}", self.api_token))
        .send()
        .await?;

    let stream = resp.bytes_stream()
        .map(|chunk| {
            chunk
                .map(|b| String::from_utf8_lossy(&b).to_string())
                .map_err(|e| e.to_string())
        });
    Ok(stream)
}
```

---

## Step 6: Response → OpenAI Format

### Converse API → OpenAI

**File:** `src/interface/chat/chat_handler.rs` lines 403-415

```rust
let content = resp
    .output
    .and_then(|o| match o {
        OutputEnum::Message(m) => Some(m),
        _ => None,
    })
    .and_then(|m| m.content.into_iter().next())
    .and_then(|cb| match cb {
        ContentBlock::Text(t) => Some(t),
        _ => None,
    })
    .unwrap_or_default();
```

Build `FullResponse`:
```rust
FullResponse {
    id: &request_id,
    object: "chat.completion",
    created: timestamp,
    model: &model_name,
    choices: [FullChoice {
        index: 0,
        message: FullMessage {
            role: "assistant",
            content: &content,
        },
        finish_reason: "stop",
    }],
    usage: Usage { input_tokens, output_tokens, total_tokens, ... }
}
```

### Cloudflare → OpenAI

**File:** `src/infrastructure/cloudflare/client.rs` lines 229-260

```rust
pub fn to_openai_response(&self, model: &str, request_id: &str) -> OpenAiChatResponse {
    let content = self.result.as_ref()
        .and_then(|r| r.messages.first())
        .map(|m| m.content.clone())
        .unwrap_or_default();

    OpenAiChatResponse {
        id: request_id.to_string(),
        object: "chat.completion".to_string(),
        model: model.to_string(),
        choices: vec![OpenAiChoice {
            index: 0,
            message: OpenAiMessage {
                role: "assistant".to_string(),
                content,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage: usage.map(|u| OpenAiUsage {
            prompt_tokens: u.input_tokens.unwrap_or(0),
            completion_tokens: u.output_tokens.unwrap_or(0),
            total_tokens: ...,
        }),
    }
}
```

---

## Step 7: Log Usage to ClickHouse

**File:** `src/shared/logging.rs`

```rust
pub fn spawn_log(
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
) {
    tokio::spawn(async move {
        logger.log(...).await;
    });
}
```

Async spawn - does not block response. Uses `tokio::spawn` to fire-and-forget.

---

## Step 8: Return JSON/SSE Response

### Non-Streaming Response

```rust
(StatusCode::OK, [("content-type", "application/json")], json).into_response()
```

### Streaming Response

```rust
Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
```

SSE events sent:
1. Content chunks: `{"id":"...","choices":[{"delta":{"content":"..."}}]}`
2. Usage chunk: `{"usage":{"input_tokens":...,"output_tokens":...,"total_tokens":...}}`
3. Done marker: `[DONE]`

---

## Extended Thinking Flow (Claude)

**File:** `src/interface/chat/thinking_handler.rs` lines 63-98

```
chat_with_thinking_handler receives request
        │
        ▼
Check if model supports reasoning (DeepSeek R1)?
        │ No
        ▼
Check if model supports thinking (Claude 3.5+/4.x)?
        │ No
        ▼
Standard chat handler (no thinking)
        │
        ▼ (Yes to thinking)
Check x-openwebui-thinking header OR req.thinking.enabled
        │ false
        ▼
Standard chat handler
        │
        ▼ (true)
thinking_handler processes request
        │
        ▼
build_thinking_request() - builds Invoke API body
        │
        ▼
invoke_thinking_model() - calls AWS Invoke API
        │
        ▼
Parse response for thinking blocks and text blocks
        │
        ▼
Return response with both thinking + text in content blocks
```

---

## Reasoning Flow (DeepSeek R1)

**File:** `src/interface/chat/reasoning_handler.rs`

Similar to thinking flow but for DeepSeek R1 models. Uses Invoke API (not Converse API).

---

## Model Capability Lookup

**File:** `src/domain/chat/capabilities.rs` lines 95-123

```rust
pub fn get_model_capabilities(model_id: &str) -> Option<ModelCapabilities> {
    let model_lower = model_id.to_lowercase();

    if model_lower.starts_with("@cf/") {
        return Some(cloudflare_capabilities(&model_lower));
    }

    if model_lower.contains("anthropic.claude") {
        Some(claude_capabilities(&model_lower))
    } else if model_lower.contains("deepseek") {
        Some(deepseek_capabilities(&model_lower))
    } // ... etc
}
```

Each capability function (e.g., `claude_capabilities`) returns `ModelCapabilities`:
```rust
ModelCapabilities {
    provider: "anthropic",
    vendor: Vendor::AwsBedrock,
    model_id_pattern: "anthropic.claude",
    uses_converse_api: true,
    supports_thinking: true,   // for Claude 3.5+ and 4.x
    supports_reasoning: false,  // for DeepSeek R1
    base_params: BaseParams { max_tokens: Some(4096), ... },
    model_specific: ModelSpecificParams { top_k: Some(250), ... },
    thinking_config: Some(ThinkingConfig { ... }),
}
```

---

## Parameter Mapping

**File:** `src/domain/chat/capabilities.rs` lines 391-449

OpenAI params → provider-specific format:

```rust
pub fn map_openai_params(...) -> (BaseParams, Option<serde_json::Value>) {
    let caps = get_model_capabilities(model_id);

    let base = BaseParams {
        max_tokens: max_tokens.or(caps.base_params.max_tokens),
        temperature: temperature.or(caps.base_params.temperature),
        top_p: top_p.or(caps.base_params.top_p),
        stop_sequences,
    };

    // Provider-specific params
    if c.provider == "cohere" {
        additional.insert("frequency_penalty", serde_json::json!(fp));
    } else if c.provider == "ai21" {
        additional.insert("frequencyPenalty", serde_json::json!({"scale": fp}));
    }

    (base, additional_params)
}
```

---

## Summary: Key Files by Function

| Function | File |
|----------|------|
| HTTP entry point | `src/main.rs` |
| Auth validation | `src/domain/auth/types.rs` |
| Chat handler routing | `src/interface/chat/chat_handler.rs` |
| Thinking handler | `src/interface/chat/thinking_handler.rs` |
| Reasoning handler | `src/interface/chat/reasoning_handler.rs` |
| Converse API builder | `src/infrastructure/bedrock/converse.rs` |
| Invoke API (thinking) | `src/infrastructure/bedrock/invoke.rs` |
| Cloudflare client | `src/infrastructure/cloudflare/client.rs` |
| Model capabilities | `src/domain/chat/capabilities.rs` |
| Request/Response types | `src/domain/chat/types.rs` |
| Usage logging | `src/shared/logging.rs` |
| App state | `src/shared/app_state.rs` |
