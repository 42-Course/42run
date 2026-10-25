.PHONY: help build test fmt clippy clean doc run

.DEFAULT_GOAL := help

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## cargo build
	cargo build

test: ## cargo test
	cargo test

fmt: ## cargo fmt
	cargo fmt

clippy: ## cargo clippy
	cargo clippy

clean: ## cargo clean
	cargo clean

doc: ## cargo doc --open
	cargo doc --open

run: ## cargo run (the 42run binary)
	cargo run
