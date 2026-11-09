#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Export the measured bootstrap + replay statistics of a local state directory
into a small committed fixture (SDX-024).

`data/` is a working directory and is not committed; the demo surface must build
from a clean checkout, so the numbers it shows about the bootstrap and replay
runs are copied here, verbatim, with provenance. Nothing is computed that the
run logs do not contain except sums, a median and de-duplication.

Usage: python3 -I scripts/export-run-stats.py [--state data/state] [--out fixtures/runs/<name>]
"""
import argparse
import datetime as dt
import hashlib
import json
import os
import statistics
import sys


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state", default="data/state")
    ap.add_argument("--out", default="fixtures/runs/20261104-mainnet")
    ap.add_argument("--bootstrap-out", default="data/bootstrap.out",
                    help="stdout of `sdx bootstrap` (holds the HAS and the HAS-vs-header check)")
    a = ap.parse_args()

    manifest = json.load(open(os.path.join(a.state, "state.json")))
    boot = json.load(open(os.path.join(a.state, "bootstrap-report.json")))

    # replay.jsonl may hold a ledger twice when a replay was interrupted after
    # the line was written and before state.json was saved; the re-applied line
    # is identical in stats (checked below), so keep the last one.
    by_ledger = {}
    dup_lines = 0
    dup_conflicts = 0
    with open(os.path.join(a.state, "replay.jsonl")) as f:
        for line in f:
            if not line.strip():
                continue
            x = json.loads(line)
            prev = by_ledger.get(x["ledger"])
            if prev is not None:
                dup_lines += 1
                if prev["stats"] != x["stats"] or prev["trades"] != x["trades"]:
                    dup_conflicts += 1
            by_ledger[x["ledger"]] = x
    if dup_conflicts:
        sys.exit(f"replay.jsonl: {dup_conflicts} duplicated ledgers disagree; refusing to export")
    rows = [by_ledger[k] for k in sorted(by_ledger)]
    seqs = [r["ledger"] for r in rows]
    if seqs != list(range(seqs[0], seqs[-1] + 1)):
        sys.exit("replay.jsonl: ledgers are not consecutive")

    def tot(key):
        return sum(r["stats"][key] for r in rows)

    replay = {
        "from_ledger": seqs[0],
        "to_ledger": seqs[-1],
        "ledgers": len(rows),
        "first_close_time": rows[0]["close_time"],
        "last_close_time": rows[-1]["close_time"],
        "trades": sum(r["trades"] for r in rows),
        "trade_errors": sum(len(r["trade_errors"]) for r in rows),
        "state_checked": tot("state_checked"),
        "state_mismatch": tot("state_mismatch"),
        "offers_upserted": tot("offers_upserted"),
        "offers_removed": tot("offers_removed"),
        "pools_upserted": tot("pools_upserted"),
        "pools_removed": tot("pools_removed"),
        "apply_ms_median": round(statistics.median(r["apply_ms"] for r in rows), 3),
        "duplicate_lines_dropped": dup_lines,
        # columns: ledger, close_time, trades, state_checked, state_mismatch,
        # offers_upserted, offers_removed, pools_upserted, apply_ms
        "columns": ["ledger", "close_time", "trades", "state_checked", "state_mismatch",
                    "offers_upserted", "offers_removed", "pools_upserted", "apply_ms"],
        "rows": [[r["ledger"], r["close_time"], r["trades"], r["stats"]["state_checked"],
                  r["stats"]["state_mismatch"], r["stats"]["offers_upserted"], r["stats"]["offers_removed"],
                  r["stats"]["pools_upserted"], round(r["apply_ms"], 2)] for r in rows],
    }

    buckets = []
    for b in boot["buckets"]:
        buckets.append({k: b[k] for k in ("hash", "records_read", "bytes_uncompressed", "offers_live",
                                          "offers_dead", "pools_live", "pools_dead", "early_exit",
                                          "hash_verified", "seconds") if k in b})
    has_check = None
    archive = None
    level_of = {}
    if os.path.exists(a.bootstrap_out):
        src = json.load(open(a.bootstrap_out))["manifest"]["source"]
        has_check = src.get("has_header_check")
        archive = src.get("archive")
        for lvl, cb in enumerate(src["has"]["currentBuckets"]):
            for kind in ("curr", "snap"):
                level_of[cb[kind]] = f"{lvl}{kind}"
    for b in buckets:
        b["level"] = level_of.get(b["hash"])
    bootstrap = {
        "checkpoint": boot["checkpoint"],
        "offers": boot["offers"],
        "pools": boot["pools"],
        "assets": boot["assets"],
        "seconds_this_attempt": boot["seconds"],
        "peak_rss_kib": boot["peak_rss_kib"],
        "bytes_uncompressed": sum(b["bytes_uncompressed"] for b in boot["buckets"]),
        "has_header_check": has_check,
        "archive": archive,
        "buckets": buckets,
    }

    state = {k: manifest[k] for k in ("network", "ledger", "close_time", "offers", "pools", "sha256", "bootstrap_checkpoint")}

    os.makedirs(a.out, exist_ok=True)
    out = {"state": state, "bootstrap": bootstrap, "replay": replay}
    with open(os.path.join(a.out, "run-stats.json"), "w") as f:
        json.dump(out, f, separators=(",", ":"))
        f.write("\n")
    prov = {
        "exported_at": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "script": "python3 -I scripts/export-run-stats.py --state " + a.state + " --out " + a.out,
        "network": "pubnet",
        "kind": "recorded mainnet run (read-only: history archive + Galexie public lake)",
        "inputs": {
            "state.json": sha256(os.path.join(a.state, "state.json")),
            "bootstrap-report.json": sha256(os.path.join(a.state, "bootstrap-report.json")),
            "replay.jsonl": sha256(os.path.join(a.state, "replay.jsonl")),
            "bootstrap.out": sha256(a.bootstrap_out) if os.path.exists(a.bootstrap_out) else None,
        },
        "notes": [
            "bootstrap.seconds_this_attempt covers the second (successful) attempt only; the cold total over both attempts was ~82 min wall (README, data/bootstrap-attempt1.err).",
            "replay rows are de-duplicated by ledger; duplicated lines had identical stats.",
            "Runs were on a shared 4-core machine with a debug build (dev profile with optimised hot crates).",
        ],
    }
    with open(os.path.join(a.out, "run.provenance.json"), "w") as f:
        json.dump(prov, f, indent=2)
        f.write("\n")
    print(f"wrote {a.out}/run-stats.json: {replay['ledgers']} ledgers, {replay['state_checked']} pre-images, "
          f"{replay['state_mismatch']} mismatches, {len(buckets)} buckets")


if __name__ == "__main__":
    main()
