# SPDX-License-Identifier: Apache-2.0
"""Print every Horizon trade of one ledger as JSON (read-only GETs, 0.6 s apart).

usage: python3 -I horizon-ledger-trades.py <horizon-url> <ledger>
Horizon has no /ledgers/N/trades, so page /trades from cursor = N << 32.
"""
import json
import sys
import time
import urllib.request

base, seq = sys.argv[1].rstrip("/"), int(sys.argv[2])
cursor, out = str(seq << 32), []
while True:
    url = f"{base}/trades?cursor={cursor}&order=asc&limit=200"
    req = urllib.request.Request(url, headers={"User-Agent": "sik-sdex-orderbook fixture capture (read-only)"})
    recs = json.load(urllib.request.urlopen(req, timeout=60))["_embedded"]["records"]
    done = False
    for r in recs:
        if int(r["paging_token"].split("-")[0]) >> 32 != seq:
            done = True
            break
        out.append(r)
    if done or len(recs) < 200:
        break
    cursor = recs[-1]["paging_token"]
    time.sleep(0.6)
json.dump({"ledger": seq, "source": f"{base}/trades?cursor={seq << 32}&order=asc&limit=200", "records": out}, sys.stdout, indent=1)
