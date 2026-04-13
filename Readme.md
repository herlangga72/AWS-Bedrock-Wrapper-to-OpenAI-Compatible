# AWS Bedrock & Cloudflare Workers AI to OpenAI-Compatible API Gateway

[![Rust](https://img.shields.io/badge/Language-Rust-black?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![AWS Bedrock](https://img.shields.io/badge/Service-AWS_Bedrock-FF9900?style=flat-square&logo=amazon-aws&logoColor=white)](https://aws.amazon.com/bedrock/)
[![Cloudflare](https://img.shields.io/badge/Service-Cloudflare_Workers_AI-F38020?style=flat-square&logo=cloudflare&logoColor=white)](https://developers.cloudflare.com/workers-ai/)
[![License-MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](https://opensource.org/licenses/MIT)

A high-performance, multi-vendor AI proxy that translates OpenAI-compatible API requests to AWS Bedrock and Cloudflare Workers AI endpoints.

---

## Features

- **Multi-Vendor Support**: Route requests to AWS Bedrock or Cloudflare Workers AI based on model prefix
- **OpenAI-Compatible**: Use existing OpenAI SDKs and tools
- **Domain-Driven Design**: Clean, maintainable architecture
- **Streaming Support**: Full streaming response handling
- **Usage Logging**: ClickHouse integration for token tracking
- **Extensible**: Easy to add new AI vendors

---

## Supported Models

### AWS Bedrock
| Provider | Models |
|----------|--------|
| Anthropic | claude-3-5-sonnet, claude-opus-4-5, claude-sonnet-4-5, claude-haiku-4-5 |
| DeepSeek | r1, chat |
| Cohere | command-r, command-r-plus |
| Mistral | mistral-7b-instruct, mistral-large |
| Meta | llama-3.1-70b-instruct, llama-3.1-8b-instruct |
| Amazon | titan, nova |

### Cloudflare Workers AI
| Model Prefix | Examples |
|--------------|----------|
| `@cf/meta/*` | llama-3.1-8b-instruct |
| `@cf/deepseek-ai/*` | deepseek-r1-distill-qwen-32b |
| `@cf/google/*` | gemma-2-2b-it |
| `@cf/mistral/*` | mistral-7b-instruct |

---

## Quick Start

### 1. Clone and Configure
```bash
git clone https://github.com/herlangga72/AWS-Bedrock-Wrapper-to-OpenAI-Compatible.git
cd AWS-Bedrock-Wrapper-to-OpenAI-Compatible
cp .env.example .env
```

### 2. Configure Environment
```bash
# AWS Bedrock (required)
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=your-key
AWS_SECRET_ACCESS_KEY=your-secret

# Cloudflare (optional - for @cf/ models)
CLOUDFLARE_ACCOUNT_ID=your-account-id
CLOUDFLARE_API_TOKEN=your-api-token

# Authentication
DEFAULT_API_KEY=your-api-key
```

### 3. Build and Run
```bash
cargo build --release
./target/release/aws-bedrock-translation-to-openai
```

### 4. Test
```bash
make test  # Run all 70 tests
```

---

## Model Routing

Requests are routed based on model prefix:

| Model Prefix | Provider |
|-------------|----------|
| `@cf/` | Cloudflare Workers AI |
| `bedrock/` | AWS Bedrock |
| (others) | AWS Bedrock |

**Example:**
```bash
# Cloudflare model
curl -X POST http://localhost:3001/v1/chat/completions \
  -H "Authorization: Bearer YOUR_KEY" \
  -d '{"model": "@cf/meta/llama-3.1-8b-instruct", "messages": [...]}'

# AWS Bedrock model
curl -X POST http://localhost:3001/v1/chat/completions \
  -H "Authorization: Bearer YOUR_KEY" \
  -d '{"model": "anthropic.claude-3-5-sonnet-20240620-v1:0", "messages": [...]}'
```

---

## Architecture

```
src/
├── domain/           # Business logic (DDD)
│   ├── chat/        # Chat types, capabilities
│   ├── embedding/   # Embedding types
│   ├── auth/        # Authentication
│   └── logging/     # Usage logging
├── infrastructure/   # External integrations
│   ├── bedrock/     # AWS Bedrock client
│   ├── cloudflare/  # Cloudflare Workers AI client
│   └── cache/       # File-based caching
├── interface/        # HTTP handlers
│   ├── chat/        # Chat endpoints
│   ├── embedding/   # Embedding endpoints
│   └── models/      # Model listing
└── shared/          # Shared utilities
```

---

## Docker Support

```bash
# Start all services
docker-compose up --build

# Start infrastructure only
make up

# View logs
make logs

# Stop services
make down
```

| Service | Port | Purpose |
|---------|------|---------|
| app | 3001 | Main application |
| clickhouse | 8123 | Usage logging |
| localstack | 4566 | AWS mocking |

---

## Testing

```
Total Tests: 70 (68 unit + 2 integration)
Coverage: 21.50%
```

```bash
make test              # All tests
make test-unit         # Unit tests only (68)
make test-integration  # Integration tests (2)
make coverage           # Coverage report
```

---

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/chat/completions` | Chat completions |
| POST | `/v1/embeddings` | Embeddings |
| GET | `/v1/models` | List available models |

---

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AWS_REGION` | Yes | us-east-1 | AWS region |
| `DEFAULT_API_KEY` | Yes | - | API authentication |
| `CLOUDFLARE_ACCOUNT_ID` | No | - | Cloudflare account |
| `CLOUDFLARE_API_TOKEN` | No | - | Cloudflare API token |
| `CLICKHOUSE_URL` | No | http://127.0.0.1:8123 | ClickHouse URL |
| `SERVER_PORT` | No | 3001 | Server port |

---

## Documentation

- [Architecture](./docs/ARCHITECTURE.md) - Detailed codebase documentation
- [Test Coverage](./docs/TEST_COVERAGE.md) - Test inventory and coverage

---

## License

MIT
