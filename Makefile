PROJECTS := pulga_win_lib pulga_gui_lib pulga_run

.PHONY: help build test fmt clippy clean run wasm-image $(PROJECTS)

.DEFAULT_GOAL := help

# `cd` into each project (not --manifest-path) so cargo's own config
# discovery picks up that project's .cargo/config.toml (its goinfre
# target-dir redirect) — config discovery walks up from the CWD, not from
# the manifest path.
help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## cargo build every project
	@for p in $(PROJECTS); do \
		echo "==> build $$p"; \
		(cd $$p && cargo build) || exit 1; \
	done

test: ## cargo test every project
	@for p in $(PROJECTS); do \
		echo "==> test $$p"; \
		(cd $$p && cargo test) || exit 1; \
	done

fmt: ## cargo fmt every project
	@for p in $(PROJECTS); do \
		(cd $$p && cargo fmt); \
	done

clippy: ## cargo clippy every project
	@for p in $(PROJECTS); do \
		(cd $$p && cargo clippy) || exit 1; \
	done

clean: ## cargo clean every project
	@for p in $(PROJECTS); do \
		(cd $$p && cargo clean); \
	done

run: ## Run the game (pulga_run)
	cd pulga_run && cargo run

wasm-image: ## Build the shared headless-browser wasm test image (used by pulga_gui_lib's `make wasm-test`)
	podman build -t pulga-wasm-test -f wasm-test.Containerfile .
