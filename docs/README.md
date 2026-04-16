# Documentation

## Files

| File | Description |
|------|-------------|
| [API_SPEC.md](./API_SPEC.md) | API endpoints, request/response formats |
| [ERROR_REFERENCE.md](./ERROR_REFERENCE.md) | Error lookup by message or HTTP status |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Module structure, flame graphs, function reference |
| [TEST_COVERAGE.md](./TEST_COVERAGE.md) | Test summary and coverage |

---

## Quick Links

### Errors (fastest path)
- [Error by message](./ERROR_REFERENCE.md#by-error-message) — substring match
- [Error by HTTP status](./ERROR_REFERENCE.md#by-http-status)
- [Trace guides](./ERROR_REFERENCE.md#trace-guides) — step-by-step debugging

### API
- [`POST /v1/chat/completions`](./API_SPEC.md#post-v1chatcompletions) — OpenAI-compatible
- [`POST /claude/v1/messages`](./API_SPEC.md#post-claudev1messages) — Anthropic-native
- [`GET /v1/models`](./API_SPEC.md#get-v1models) — List models
- [Tools & Function Calling](./API_SPEC.md#tools--function-calling)
- [Claude Extended Thinking](./API_SPEC.md#claude-extended-thinking)
- [DeepSeek R1 Reasoning](./API_SPEC.md#deepseek-r1-reasoning)

### Code
- [Module Structure](./ARCHITECTURE.md#1-module-structure)
- [Flame Graphs](./ARCHITECTURE.md#2-flame-graphs) — execution traces
- [Function Reference](./ARCHITECTURE.md#3-function-reference) — file:line lookup
- [Route → Handler Map](./ARCHITECTURE.md#4-route--handler-map)
- [Supported Models](./ARCHITECTURE.md#supported-models)
- [Parameter Support](./ARCHITECTURE.md#parameter-support)

### Testing
- [Test Coverage](./TEST_COVERAGE.md) — **70 tests total**
- [Test Commands](./TEST_COVERAGE.md#test-commands)
