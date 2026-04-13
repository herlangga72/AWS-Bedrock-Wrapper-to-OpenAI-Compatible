# Edge Cases Documentation

This document captures edge cases identified during testing of the AWS Bedrock Translation to OpenAI Compatible API.

## Authentication Edge Cases

| Edge Case | Input | Expected Behavior | Actual Behavior | Status |
|-----------|-------|-------------------|-----------------|--------|
| Missing authorization header | No `Authorization` header | 401 Unauthorized | `{"error":"Missing API Key","code":401}` | **Fixed** |
| Wrong authorization format | `Authorization: Basic invalid` | 400 Bad Request | `invalid HTTP header (authorization)` | As-is |
| Missing Bearer prefix | `Authorization: put-your-api-key` | 400 Bad Request | `invalid HTTP header (authorization)` | As-is |
| Invalid API key | `Authorization: Bearer wrong-key` | 401 Unauthorized | `{"error":"Invalid API Key","code":401}` | **Fixed** |
| Correct API key | `Authorization: Bearer put-your-api-key` | 200 OK | Works correctly | OK |

## Request Validation Edge Cases

| Edge Case | Input | Expected Behavior | Actual Behavior | Status |
|-----------|-------|-------------------|-----------------|--------|
| Empty request body | `{}` | 400 Bad Request | `missing field 'model'` (JSON error) | As-is |
| Missing model field | `{"messages": [...]}` | 400 Bad Request | `missing field 'model'` (JSON error) | As-is |
| Empty model name | `"model": ""` | 400 Bad Request | Validation rejects at handler | **Fixed** |
| Invalid model name | `"model": "nonexistent-model-xyz"` | 400/404 | `{"error":"Stream failed"}` | As-is |
| Wrong content type | `Content-Type: text/plain` | 400 Bad Request | `Expected request with 'Content-Type: application/json'` | As-is |

## Message Structure Edge Cases

| Edge Case | Input | Expected Behavior | Actual Behavior | Status |
|-----------|-------|-------------------|-----------------|--------|
| Missing role in message | `{"content": "Hello"}` | 400 Bad Request | `missing field 'role'` (JSON error) | As-is |
| Invalid role value | `"role": "admin"` | 400 Bad Request | `{"error":"messages[0].role must be 'user', 'assistant', or 'system', got 'admin'","code":400}` | **Fixed** |
| Empty messages array | `"messages": []` | 400 Bad Request | `{"error":"messages array cannot be empty","code":400}` | **Fixed** |
| System prompt injection | System role message | Should handle safely | Works (model ignores) | OK |
| Very long message (10k chars) | `"content": "aaa..."` | Handle gracefully | Works correctly | OK |
| Special characters in message | HTML/script tags | Handle safely | JSON parse error (unquoted) | As-is |

## Parameter Validation Edge Cases

| Edge Case | Input | Expected Behavior | Actual Behavior | Status |
|-----------|-------|-------------------|-----------------|--------|
| Negative temperature | `"temperature": -1` | Handle gracefully | `{"error":"temperature must be between 0.0 and 2.0, got -1.0","code":400}` | **Fixed** |
| Temperature > 2 | `"temperature": 5` | Handle gracefully | `{"error":"temperature must be between 0.0 and 2.0, got 5.0","code":400}` | **Fixed** |
| Stop sequences with wrong format | `"stop": "word"` (string, not array) | Handle gracefully | Works (accepted) | OK |
| Invalid streaming field type | `"stream": "yes"` (string) | 400 Bad Request | `expected a boolean` | OK |
| Multi-byte characters (UTF-8) | `"你好世界 🌍 привет"` | Handle correctly | Works correctly | OK |

## Embeddings Endpoint Edge Cases

| Edge Case | Input | Expected Behavior | Actual Behavior | Status |
|-----------|-------|-------------------|-----------------|--------|
| Missing input field | `{"model": "nova-embed-v1"}` | 400 Bad Request | `missing field 'input'` (JSON error) | As-is |
| Invalid input type (integer) | `"input": 12345` | 400 Bad Request | `expected a sequence` (JSON error) | As-is |
| Empty input array | `"input": []` | 400 Bad Request | `{"error":"input array cannot be empty","code":400}` | **Fixed** |
| Very large input (10k words) | Long text array | Handle gracefully | May timeout silently | As-is |
| Concurrent embedding requests | 3 parallel requests | Handle correctly | Works | OK |
| Missing authentication | No `Authorization` header | 401 Unauthorized | `{"error":"Missing API Key","code":401}` | **Fixed** |

## Concurrency Edge Cases

| Edge Case | Input | Expected Behavior | Actual Behavior | Status |
|-----------|-------|-------------------|-----------------|--------|
| 5 concurrent chat requests | Parallel POST requests | Handle correctly | All 5 completed successfully | OK |
| 3 concurrent embedding requests | Parallel embedding calls | Handle correctly | All completed | OK |

## Summary of Fixes Applied

### Fixed Issues

1. **Authentication responses now consistent JSON format**
   - Missing API Key: `{"error":"Missing API Key","code":401}`
   - Invalid API Key: `{"error":"Invalid API Key","code":401}`

2. **Empty messages array validation**
   - Previously: silent failure (empty response)
   - Now: `{"error":"messages array cannot be empty","code":400}`

3. **Invalid message role validation**
   - Previously: `Stream failed`
   - Now: `{"error":"messages[0].role must be 'user', 'assistant', or 'system', got 'admin'","code":400}`

4. **Temperature range validation**
   - Previously: negative temp or temp > 2 caused `Stream failed`
   - Now: `{"error":"temperature must be between 0.0 and 2.0, got X.X","code":400}`

5. **Empty embedding input validation**
   - Previously: silent failure
   - Now: `{"error":"input array cannot be empty","code":400}`

6. **Embedding authentication**
   - Previously: embeddings endpoint had no auth check
   - Now: requires API key with proper error response

### Issues Not Fixed (By Design)

1. **JSON deserialization errors** - These occur before reaching the handler, so we can't customize the response format
2. **Invalid model names** - We don't maintain a list of valid models; invalid names are passed to AWS and result in stream failures
3. **Very large input** - Passed to Bedrock; may timeout silently (no easy fix without adding size limits)

## Error Response Format

All fixed endpoints now return consistent JSON error format:
```json
{
  "error": "Description of the error",
  "code": 400
}
```

Where `code` is the HTTP status code as a number.