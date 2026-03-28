# AWS Bedrock to OpenAI-Compatible API Gateway

[![Rust](https://img.shields.io/badge/Language-Rust-black?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![AWS Bedrock](https://img.shields.io/badge/Service-AWS_Bedrock-FF9900?style=flat-square&logo=amazon-aws&logoColor=white)](https://aws.amazon.com/bedrock/)
[![License-MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](https://opensource.org/licenses/MIT)

A high-performance translation layer designed to map Amazon Bedrock foundation models to the industry-standard OpenAI API schema.

---

### Security Disclosure: Proof of Concept
**This repository is currently a Proof of Concept (PoC) and is not intended for production environments.**

The current iteration lacks a robust multi-tenant authentication framework. Exposing this proxy to public networks or untrusted environments presents a significant security risk. It is intended strictly for local development, internal benchmarking, and isolated architectural evaluation. Unauthorized access could result in significant AWS infrastructure costs.

---

## Technical Overview

The **AWS-Bedrock-Wrapper-to-OpenAI-Compatible** project is a low-latency reverse proxy authored in Rust. It addresses the interoperability gap between Amazon’s proprietary invocation mechanisms and the widely adopted OpenAI API specification (`/v1/chat/completions`).

By implementing this gateway, engineering teams can utilize existing OpenAI-compatible SDKs and agentic frameworks (e.g., LangChain, AutoGPT, or custom frontends) while leveraging AWS Bedrock’s managed foundation models. This architecture mitigates vendor lock-in and facilitates easier migration between cloud providers.

### Primary Objectives
1.  **Protocol Standardization:** Mapping proprietary AWS payloads to the OpenAI schema to support the broader ecosystem of AI tooling.
2.  **Operational Telemetry:** Resolving the opacity of real-time usage data in Bedrock by routing granular token metrics to a ClickHouse analytical database.

---

## Architectural Topology

The system utilizes a decoupled architecture to ensure that telemetry ingestion does not block the primary inference request-response cycle.

| Component | Technology | Functional Responsibility |
| :--- | :--- | :--- |
| **Translation Proxy** | Rust | Concurrent HTTP handling, JSON serialization, and AWS SigV4 request signing. |
| **Inference Provider** | Amazon Bedrock | Execution of foundation model inference (Claude, Nova, Titan). |
| **Analytics Engine** | ClickHouse | Columnar storage for high-volume token ingestion and cost aggregation. |

### Cost Aggregation Logic
The proxy facilitates precise financial oversight by calculating total cost $$C_{total}$$ through the aggregation of request-level token counts against model-specific pricing tiers:

$$C_{total} = \sum_{i=1}^{n} (T_{prompt,i} \times P_{prompt,m(i)} + T_{completion,i} \times P_{completion,m(i)})$$

---

## Prerequisites

* **Rust Toolchain:** Latest stable version of Cargo and Rustc.
* **AWS Credentials:** Local configuration with `IAM` permissions for `bedrock:InvokeModel` and `bedrock:ListFoundationModels`.
* **ClickHouse Instance:** Required for telemetry. Can be deployed via Docker for development:
    `docker run -d --name clickhouse-server -p 8123:8123 -p 9000:9000 clickhouse/clickhouse-server`

---

## Installation and Execution

### 1. Clone Repository
```bash
git clone https://github.com/herlangga72/AWS-Bedrock-Wrapper-to-OpenAI-Compatible.git
cd AWS-Bedrock-Wrapper-to-OpenAI-Compatible
```
### 2. Configuration
The project is transitioning to environment-based configuration.

```bash
cp .env.example .env
# Define AWS_REGION and CLICKHOUSE_URL within the .env file.
```
### 3. Build and Run
```bash
cargo build --release
./target/release/aws-bedrock-proxy
```
## Usage Examples

### Direct API Interaction (cURL)

```bash
curl [http://127.0.0.1:8080/v1/chat/completions](http://127.0.0.1:8080/v1/chat/completions) \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer LOCAL_DEV_TOKEN" \
  -d '{
    "model": "anthropic.claude-3-5-sonnet-20240620-v1:0",
    "messages": [{"role": "user", "content": "Analyze the efficiency of Rust in network proxies."}],
    "temperature": 0.1
  }'
```

### Python SDK Integration
```Python
from openai import OpenAI
client = OpenAI(
    base_url="[http://127.0.0.1:8080/v1](http://127.0.0.1:8080/v1)",
    api_key="local-dev-bypass"
)

response = client.chat.completions.create(
    model="anthropic.claude-3-5-sonnet-20240620-v1:0",
    messages=[{"role": "user", "content": "Connection test."}]
)

print(response.choices[0].message.content)
```

## Development Roadmap
| Milestone          | Technical Objective   | Operational Impact                                                                 |
|-------------------|----------------------|-------------------------------------------------------------------------------------|
| State Management  | Context Offloading   | Maintains conversational state via local pointers to reduce network payload size.  |
| Efficiency        | Context Compression  | Algorithmic pruning of prompts to minimize token expenditure on long-form context. |
| Security          | JWT Authentication   | Implementation of secure, multi-tenant API key management.                         |

---

## Testing

The test suite requires no live AWS credentials — all tests that touch the HTTP layer exit before reaching the provider.

```bash
cargo test
```

| Scope | Location | Coverage |
| :--- | :--- | :--- |
| `Config::addr()` | `src/config.rs` | host + port formatting |
| `NoopCompactor` | `src/types/compactor.rs` | passthrough, order, empty input |
| Wire types (serde) | `src/types/openai.rs` | `ChatRequest` / `Message` / `ModelData` / `ModelList` |
| `ProviderError` | `src/provider/mod.rs` | `Display` for both variants |
| `ProviderRegistry::provider_for` | `src/provider/registry.rs` | model-prefix routing |
| `extract_user_email` | `src/middleware.rs` | auth accept / reject / email header |
| `ClickHouseLogger::log_usage` | `src/logging.rs` | fire-and-forget error handling |
| HTTP layer | `src/router.rs` | 401 on bad auth, 400 on bad JSON |


## Contribution and License
Contributions regarding security hardening and context management are prioritized. This software is released under the MIT License.