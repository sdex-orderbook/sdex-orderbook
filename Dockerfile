# SPDX-License-Identifier: Apache-2.0
# Render image for `sdx serve` (read-only). Build context: repo root.
# The tag must match rust-toolchain.toml (1.97.1); verify it exists on Docker Hub before the first build.
FROM rust:1.97.1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml NOTICE ./
COPY crates ./crates
RUN cargo build --release --locked -p sik-sdex-orderbook-cli

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates python3-minimal \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/sdx /usr/local/bin/sdx
COPY deploy/render-entrypoint.sh /usr/local/bin/render-entrypoint.sh
COPY deploy/placeholder.py /usr/local/share/sdx/placeholder.py
COPY NOTICE /usr/local/share/sdx/NOTICE
# The persistent disk is mounted here; sdx's relative defaults (data/state, data/bucket-cache) land on it.
WORKDIR /var/lib/sdx
ENTRYPOINT ["/usr/local/bin/render-entrypoint.sh"]
