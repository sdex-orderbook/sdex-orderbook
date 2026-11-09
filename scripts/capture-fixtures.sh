#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Capture offline test fixtures (read-only GETs). Writes provenance beside each set.
#
#   capture-fixtures.sh buckets <checkpoint>          level-0 curr+snap buckets of that checkpoint (~100 KB)
#   capture-fixtures.sh lake <first-ledger> <count>   Galexie objects (count <= 10) + Horizon /ledgers/N/trades
#
# Diff fixtures are written by `sdx diff --out fixtures/diff/<run>` (see TESTING.md).
set -euo pipefail
cd "$(dirname "$0")/.."
ARCHIVE="${ARCHIVE:-https://history.stellar.org/prd/core-live/core_live_001}"
LAKE="${LAKE:-https://aws-public-blockchain.s3.amazonaws.com}"
HORIZON="${HORIZON:-https://horizon.stellar.org}"
UA="sik-sdex-orderbook fixture capture (read-only)"
now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

case "${1:-}" in
buckets)
  cp="$2"
  hex=$(printf '%08x' "$cp")
  out=fixtures/buckets
  mkdir -p "$out"
  has_url="$ARCHIVE/history/${hex:0:2}/${hex:2:2}/${hex:4:2}/history-$hex.json"
  curl -sfA "$UA" "$has_url" -o "$out/history-$hex.json"
  files=()
  for h in $(python3 -I -c 'import json,sys; l=json.load(open(sys.argv[1]))["currentBuckets"][0]; print(l["curr"], l["snap"])' "$out/history-$hex.json"); do
    [ "$h" = "0000000000000000000000000000000000000000000000000000000000000000" ] && continue
    url="$ARCHIVE/bucket/${h:0:2}/${h:2:2}/${h:4:2}/bucket-$h.xdr.gz"
    curl -sfA "$UA" "$url" -o "$out/bucket-$h.xdr.gz"
    files+=("\"bucket-$h.xdr.gz\": \"$url\"")
    sleep 1
  done
  cat > "$out/buckets.provenance.json" <<EOF
{
  "captured_at": "$(now)",
  "script": "scripts/capture-fixtures.sh buckets $cp",
  "checkpoint": $cp,
  "has": "$has_url",
  "files": { $(IFS=,; echo "${files[*]}") },
  "note": "level-0 curr and snap live buckets; mainnet, public, read-only"
}
EOF
  ;;
lake)
  first="$2"; count="$3"
  [ "$count" -le 10 ] || { echo "count must be <= 10" >&2; exit 2; }
  out=fixtures/lake
  mkdir -p "$out"
  keys=()
  for ((s = first; s < first + count; s++)); do
    start=$(( s / 64000 * 64000 )); end=$(( start + 63999 ))
    part=$(printf '%08X--%d-%d' $(( 4294967295 - start )) $start $end)
    key=$(printf 'v1.1/stellar/ledgers/pubnet/%s/%08X--%d.xdr.zst' "$part" $(( 4294967295 - s )) $s)
    mkdir -p "$out/mirror/$(dirname "$key")"
    curl -sfA "$UA" "$LAKE/$key" -o "$out/mirror/$key"
    python3 -I scripts/horizon-ledger-trades.py "$HORIZON" "$s" > "$out/horizon-trades-$s.json"
    keys+=("\"$key\"")
    sleep 1
  done
  cat > "$out/lake.provenance.json" <<EOF
{
  "captured_at": "$(now)",
  "script": "scripts/capture-fixtures.sh lake $first $count",
  "lake": "$LAKE",
  "horizon": "$HORIZON",
  "ledgers": [$first, $(( first + count - 1 ))],
  "objects": [$(IFS=,; echo "${keys[*]}")],
  "horizon_trades": "horizon-trades-<seq>.json = all records of GET /trades?cursor=<seq<<32>&order=asc&limit=200 (paged) whose operation id belongs to <seq>"
}
EOF
  ;;
*)
  sed -n '3,9p' "$0"; exit 2 ;;
esac
