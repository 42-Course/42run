NAME    := 42run
LIBS    := pulga_win_lib pulga_gui_lib
LIB_URL := https://github.com/pulgamecanica

# Release identity: a unique tag from the game's version + commit, plus a
# "secret" codename picked from the commit hash (first hex digit ->
# adjective, second -> noun) that names the release and its files. Only
# 16x16 codenames exist, so the tag (not the codename) keeps releases unique.
VERSION = $(shell sed -n 's/^version = "\(.*\)"/\1/p' pulga_run/Cargo.toml | head -n1)
SHA     = $(shell git rev-parse --short=7 HEAD)
TAG     = v$(VERSION)-$(SHA)
ADJ     := Atomic Bouncy Cosmic Dizzy Electric Feral Gnarly Hyper Jolly Lunar Mighty Neon Quantum Rusty Sneaky Turbo
NOUN    := Marvin Norminette Moulinette Piscine Bocal Cadet Segfault Kitten Motorbike Director Cluster Pointer Malloc Bus Hoverboard DeLorean
CODENAME = $(shell a=$$(printf '%d' 0x$$(echo $(SHA) | cut -c1)); n=$$(printf '%d' 0x$$(echo $(SHA) | cut -c2)); \
	echo "$$(echo $(ADJ) | cut -d' ' -f$$((a + 1)))-$$(echo $(NOUN) | cut -d' ' -f$$((n + 1)))")
PKG     = $(NAME)-v$(VERSION)-$(CODENAME)

# Wherever .cargo/config.toml (or $CARGO_TARGET_DIR) puts cargo's output.
TARGET_DIR = $(shell cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')

.PHONY: all $(NAME) libs test fmt clippy doc run clean fclean re help wasm-image release-tag release-name release-pkg dist

.DEFAULT_GOAL := help

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z0-9_-]+:.*##/ {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

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

fclean: clean ## clean + remove the 42run binary and dist/
	rm -rf $(NAME) dist

re: fclean all ## Rebuild from scratch

release-tag: ## Print this commit's release tag
	@echo $(TAG)

release-name: ## Print this commit's secret release codename
	@echo "$(subst -, ,$(CODENAME))"

release-pkg: ## Print the release file prefix (42run-v<version>-<Codename>)
	@echo $(PKG)

# Committed state only (git archive): repo + libs in one tarball that builds
# with no cloning, plus dist/notes.md recording which lib commits went in.
dist: | $(LIBS) ## Bundle repo + libs into dist/<pkg>-src.tar.gz
	rm -rf dist && mkdir -p dist/$(PKG)
	git archive HEAD | tar -x -C dist/$(PKG)
	@for l in $(LIBS); do \
		mkdir dist/$(PKG)/$$l; \
		git -C $$l archive HEAD | tar -x -C dist/$(PKG)/$$l; \
		echo "- $$l @ $$(git -C $$l rev-parse HEAD)" >> dist/notes.md; \
	done
	tar -czf dist/$(PKG)-src.tar.gz -C dist $(PKG)
	rm -rf dist/$(PKG)

wasm-image: ## Build the shared headless-browser wasm test image (used by pulga_gui_lib's `make wasm-test`)
	podman build -t pulga-wasm-test -f wasm-test.Containerfile .
