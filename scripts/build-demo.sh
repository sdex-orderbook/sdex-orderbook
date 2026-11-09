#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build the investor demo from committed fixtures only (no network after the
# package install): web app at /, docs site at /docs/, into demo-dist/.
# Usage: scripts/build-demo.sh [--offline]
set -euo pipefail
cd "$(dirname "$0")/.."
OFF="${1:-}"
(cd web && pnpm install --frozen-lockfile ${OFF:+--offline} && pnpm demo:data && pnpm test && pnpm build)
(cd site && pnpm install --frozen-lockfile ${OFF:+--offline} && pnpm build)
rm -rf demo-dist
mkdir -p demo-dist
cp -r web/dist/. demo-dist/
cp -r site/dist demo-dist/docs
echo "demo-dist/ ready: serve it with  python3 -m http.server 4173 --directory demo-dist"
