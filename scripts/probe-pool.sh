#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Read-only probe of horizon.stellar.org for one classic pool pair (SDX-012 evidence).
# Three GETs, >= 1 s apart: the pool by reserves, a single-hop strict-send through
# the pair, and the SDEX order book of the pair. Bodies and response headers are
# stored verbatim beside a provenance file. Never signs or sends anything.
#
# Usage: scripts/probe-pool.sh <CODE:ISSUER source asset> <CODE:ISSUER dest asset> <source_amount> [out_dir]
set -euo pipefail
cd "$(dirname "$0")/.."

SRC="${1:?source asset CODE:ISSUER}"
DST="${2:?destination asset CODE:ISSUER}"
AMOUNT="${3:?source amount, e.g. 0.0010000}"
STAMP="$(date -u +%Y%m%dT%H%MZ)"
OUT="${4:-fixtures/probes/${STAMP}-pool}"
H="https://horizon.stellar.org"
mkdir -p "$OUT"

asset_params() { # prefix CODE:ISSUER
  local p="$1" code="${2%%:*}" issuer="${2##*:}" t="credit_alphanum4"
  [ "${#code}" -gt 4 ] && t="credit_alphanum12"
  printf '%s_asset_type=%s&%s_asset_code=%s&%s_asset_issuer=%s' "$p" "$t" "$p" "$code" "$p" "$issuer"
}

get() { # name url
  curl -sS -m 30 -D "$OUT/$1.headers.txt" -o "$OUT/$1.json" "$2"
  sleep 1
}

URL_POOL="$H/liquidity_pools?reserves=${SRC},${DST}"
URL_PATH="$H/paths/strict-send?$(asset_params source "$SRC")&source_amount=${AMOUNT}&destination_assets=${DST}"
URL_BOOK="$H/order_book?$(asset_params selling "$SRC")&$(asset_params buying "$DST")"
get pool "$URL_POOL"
get strict-send "$URL_PATH"
get order-book "$URL_BOOK"

ledger_of() { grep -i '^latest-ledger:' "$OUT/$1.headers.txt" | tr -dc '0-9'; }
cat > "$OUT/probe.provenance.json" <<EOF
{
  "captured_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "script": "scripts/probe-pool.sh $SRC $DST $AMOUNT",
  "horizon": "$H",
  "kind": "live read-only GETs, stored verbatim",
  "requests": {
    "pool": {"url": "$URL_POOL", "latest_ledger": "$(ledger_of pool)"},
    "strict-send": {"url": "$URL_PATH", "latest_ledger": "$(ledger_of strict-send)"},
    "order-book": {"url": "$URL_BOOK", "latest_ledger": "$(ledger_of order-book)"}
  }
}
EOF
echo "wrote $OUT"
