.PHONY: help test test-unit test-integration test-all coverage clean build up down logs

help: ## Show this help message
	@echo "Available targets:"
	@echo "  test-unit         - Run unit tests only"
	@echo "  test-integration  - Run integration tests only (requires services)"
	@echo "  test              - Run all tests"
	@echo "  test-all          - Run all tests"
	@echo "  coverage          - Generate code coverage report"
	@echo "  build             - Build the application"
	@echo "  build-tests       - Build test binaries"
	@echo "  up                - Start docker-compose services"
	@echo "  down              - Stop docker-compose services"
	@echo "  logs              - Show docker-compose logs"
	@echo "  clean             - Clean build artifacts"

# Docker Compose commands
up: ## Start docker-compose services (clickhouse, localstack)
	docker-compose up -d clickhouse localstack

down: ## Stop docker-compose services
	docker-compose down

logs: ## Show docker-compose logs
	docker-compose logs -f

# Run services for integration tests
up-full: ## Start all services including app
	docker-compose up --build

# Build commands
build: ## Build the application
	cargo build --release

build-tests: ## Build test binaries
	cargo test --no-run

# Test commands
test-unit: ## Run unit tests only (no integration tests)
	cargo test --lib

test-integration: ## Run integration tests only (requires services)
	cargo test --test '*_test'

test-all: test ## Run all tests

# Coverage
coverage: ## Generate code coverage report
	cargo tarpaulin --out Html --out Lcov

# Development
check: ## Run cargo check
	cargo check

clippy: ## Run clippy
	cargo clippy -- -D warnings

fmt: ## Format code
	cargo fmt

# Cleanup
clean: ## Clean build artifacts
	cargo clean
	rm -rf target/

clean-data: ## Clean data directory
	rm -rf data/
