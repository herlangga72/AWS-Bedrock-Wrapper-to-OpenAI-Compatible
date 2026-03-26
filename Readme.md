# An Wrapper to Connect AWS Bedrock to OpenAPI

This Code is just in prove of concept that we can wrap bedrock to OpenAI compatible API. It serves as a minimal example and is not production‑ready.

## Current Capability:

1. list all models from AWS
2. Inference with Selected models

## Upcoming Capability Planned:
1. Add context offloading and pointer
2. Context Compression for reducing tokens
3. Multi Authentication (the authenticatin basicly broken, don't expose this to internet)
4. Restructure using ENVIRONTMENT Variable, right now is hard coded

## What needed to run this service:
1. A running computer
2. A Clickhouse database for logging purpose how much token you use. (AWS Kinda S*ck at let us know that if we not logging our self), / maybe we will upgrade it by collecting from cloudwatch later on