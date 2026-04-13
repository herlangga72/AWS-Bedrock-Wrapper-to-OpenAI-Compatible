# Test Coverage Report

**Last Updated:** 2026-04-11
**Total Tests:** 70 (68 unit + 2 integration)

---

## Executive Summary

| Category | Count | Status |
|----------|-------|--------|
| Unit Tests (src/) | 68 | ✅ All Passing |
| Integration Tests (tests/) | 2 | ✅ Passing |
| Total | **70** | ✅ |

**Coverage:** 21.50% (146/679 lines)

---

## Test Breakdown

### Unit Tests (68 tests in `src/`)

| Module | Count | Coverage |
|--------|-------|----------|
| `domain/chat/types.rs` | 22 | Chat request/response types |
| `domain/chat/capabilities.rs` | 31 | Model registry, param mapping |
| `domain/embedding/types.rs` | 9 | Embedding types |
| `infrastructure/cloudflare/client.rs` | 11 | Response conversion, builder, detection |
| `interface/chat/chat_handler.rs` | 7 | Response serialization, model normalization |

### Integration Tests (2 tests in `tests/`)

| File | Count | Description |
|------|-------|-------------|
| `tests/cloudflare_client_test.rs` | 2 | Cloudflare client builder & detection |

### Additional Integration Tests (require infrastructure)

| File | Description | Requires |
|------|-------------|----------|
| `tests/integration/auth.rs` | 7 tests | SQLite |
| `tests/integration/logging.rs` | 3 tests | ClickHouse |
| `tests/integration/cloudflare_api.rs` | 5 tests | mockito |

---

## Weak Points Analysis

### Critical Gaps

| Area | Risk | Status |
|------|------|--------|
| HTTP Handlers | HIGH | ❌ Not tested |
| AWS Bedrock Converse | HIGH | ❌ Not tested |
| AWS Bedrock Invoke | HIGH | ❌ Not tested |
| Cloudflare API | MEDIUM | ⚠️ Partial (response conversion tested) |
| ClickHouse Logging | MEDIUM | ⚠️ Infrastructure required |
| SQLite Auth | LOW | ⚠️ Infrastructure required |

---

## Test Commands

```bash
# Run unit tests only (68 tests)
make test-unit

# Run integration tests (2 tests)
make test-integration

# Run all tests (70 tests)
make test

# Run with coverage
make coverage

# Docker for infrastructure tests
make up           # Start ClickHouse + LocalStack
make down         # Stop services
make logs         # View logs
```

---

## Coverage by Feature

| Feature | Unit | Integration | Status |
|---------|------|-------------|--------|
| Chat Completions | 22 | 0 | ✅ Types tested |
| Extended Thinking | 4 | 0 | ✅ Types tested |
| Reasoning | 2 | 0 | ✅ Types tested |
| Embeddings | 9 | 0 | ✅ Types tested |
| Model Capabilities | 31 | 2 | ✅ Well tested |
| Cloudflare Client | 11 | 5 | ⚠️ Response tested |
| Auth | 0 | 7 | ⚠️ Requires infrastructure |
| Logging | 0 | 3 | ⚠️ Requires infrastructure |
