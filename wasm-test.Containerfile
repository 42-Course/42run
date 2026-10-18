# Shared image for running a crate's wasm32 tests in a real headless
# browser. Keeps the wasm32 toolchain / wasm-pack / geckodriver out of the
# host (this is a 42-school box with a tiny home quota and no rustup), while
# still letting the compiled .wasm genuinely run in a browser engine.
#
# Build once: podman build -t pulga-wasm-test -f wasm-test.Containerfile .
# Run: see PulgaGuiLib's `make wasm-test` (mounts the crate plus any sibling
# path-dependencies at their original host paths, since `cargo metadata`
# needs path deps to exist on disk even when cfg-gated out of the wasm32
# build).
FROM rust:slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends firefox-esr ca-certificates curl pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
    && curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

ENV CARGO_TARGET_DIR=/tmp/wasm-target

CMD ["wasm-pack", "test", "--headless", "--firefox"]
