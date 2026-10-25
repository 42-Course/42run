.PHONY: help build test fmt clippy clean doc editor

.DEFAULT_GOAL := help

# Workspace member: cargo finds the root workspace (and its goinfre
# target-dir in ../.cargo/config.toml) by walking up from here.

# Project the editor opens; override with `make editor PROJECT=path`.
PROJECT ?= ../pulga_run

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## cargo build (engine library + editor)
	cargo build

test: ## cargo test (unit + doc tests, no display needed)
	cargo test

fmt: ## cargo fmt
	cargo fmt

clippy: ## cargo clippy
	cargo clippy --all-targets

clean: ## cargo clean (this package only)
	cargo clean -p pulga_engine

doc: ## cargo doc --open
	cargo doc --no-deps --open

editor: ## Run the editor on $(PROJECT) (needs a display)
	cargo run --bin pulga_editor -- $(PROJECT)
