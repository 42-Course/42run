NAME    := 42run
LIBS    := pulga_win_lib pulga_gui_lib
LIB_URL := https://github.com/pulgamecanica

# Wherever .cargo/config.toml (or $CARGO_TARGET_DIR) puts cargo's output.
TARGET_DIR = $(shell cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')

.PHONY: all $(NAME) libs test fmt clippy doc run clean fclean re help wasm-image

.DEFAULT_GOAL := all

all: $(NAME) ## Build the release 42run binary into the repo root

# Phony: cargo does its own up-to-date check, make just always asks it.
$(NAME): | $(LIBS)
	cargo build --release --bin $(NAME)
	cp "$(TARGET_DIR)/release/$(NAME)" $(NAME)

# The libs are their own repos (gitignored here): clone them if missing,
# leave them alone if present.
$(LIBS):
	git clone $(LIB_URL)/$@.git $@

libs: $(LIBS) ## Clone the lib subrepos if missing

# Each lib is its own cargo project; `cd` into it (not --manifest-path) so
# cargo picks up that lib's .cargo/config.toml.
test: | $(LIBS) ## cargo test the libs and the workspace
	@for p in $(LIBS); do echo "==> test $$p"; (cd $$p && cargo test) || exit 1; done
	cargo test --workspace

fmt: | $(LIBS) ## cargo fmt everything
	@for p in $(LIBS); do (cd $$p && cargo fmt); done
	cargo fmt --all

clippy: | $(LIBS) ## cargo clippy everything
	@for p in $(LIBS); do echo "==> clippy $$p"; (cd $$p && cargo clippy) || exit 1; done
	cargo clippy --workspace

doc: | $(LIBS) ## cargo doc the workspace (libs included as deps)
	cargo doc --workspace

run: | $(LIBS) ## Run the game
	cargo run --bin $(NAME)

clean: ## cargo clean the libs and the workspace
	@for p in $(LIBS); do [ -d $$p ] && (cd $$p && cargo clean) || true; done
	cargo clean

fclean: clean ## clean + remove the 42run binary
	rm -f $(NAME)

re: fclean all ## Rebuild from scratch

wasm-image: ## Build the shared headless-browser wasm test image (used by pulga_gui_lib's `make wasm-test`)
	podman build -t pulga-wasm-test -f wasm-test.Containerfile .

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z0-9_-]+:.*##/ {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
