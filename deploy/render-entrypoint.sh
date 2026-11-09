#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# First boot: answer 503 on $PORT while bootstrapping from the history archive (~82 min)
# and replaying to head; later boots: skip straight to serve. Read-only network use.
set -euo pipefail
PORT="${PORT:-10000}"
cd /var/lib/sdx

if [ ! -f data/state/state.json ]; then
  echo "no state on the disk: bootstrapping (first boot only)"
  python3 /usr/local/share/sdx/placeholder.py "$PORT" &
  PLACEHOLDER=$!
  trap 'kill $PLACEHOLDER 2>/dev/null || true' EXIT
  sdx bootstrap --out data/state
  sdx replay --state data/state --to head
  kill "$PLACEHOLDER" 2>/dev/null || true
  wait "$PLACEHOLDER" 2>/dev/null || true
  trap - EXIT
fi

exec sdx serve --state data/state --addr "0.0.0.0:${PORT}" --follow
