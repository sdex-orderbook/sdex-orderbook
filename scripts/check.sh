#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Offline checks: format, lint, tests. No network.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
