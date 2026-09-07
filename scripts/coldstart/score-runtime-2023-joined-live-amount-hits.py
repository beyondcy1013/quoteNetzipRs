#!/usr/bin/env python3
"""Dump the 3 exact-second live amount hits from section 66.

5,039 live joins have amount-rel<1e-3 on only 3 rows and all_six 0.
This lists those 3 and which other fields still miss.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-joined-live-amount-hits.py EXTRACT CALLBACK META_0903 MORNING_0104
"""
from __future__ import annotations

import json
import os
import sys
from collections import defaultdict

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
joined = SourceFileLoader(
    "score_joined_auction_fields",
    os.path.join(_DIR, "score-joined-auction-fields.py"),
).load_module()
six = SourceFileLoader(
    "score_runtime_2023_joined_live_all_six",
    os.path.join(_DIR, "score-runtime-2023-joined-live-all-six.py"),
).load_module()
lv = SourceFileLoader(
    "score_runtime_2023_joined_live_bid1_leftover",
    os.path.join(_DIR, "score-runtime-2023-joined-live-bid1-leftover.py"),
).load_module()


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-joined-live-amount-hits.py EXTRACT "
            "CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    morning = seed.load_largest_0104(morning_dir)
    rows, _ = seed.load_decoded(extract_dir)
    cb_by = defaultdict(list)
    for q in six.stream_full(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    hits = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None or float(meta.get("scale") or 0) <= 0:
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        q = joined.pick_later(cands)
        if q["price"] == 0:
            continue
        morn = morning.get((row["market"], meta["code"]))
        flags, proj = six.flags_of(row["rec"], meta, morn, q)
        if not flags["amount"]:
            continue
        bid_raw = seed.i32(row["rec"], 0x68)
        hits.append(
            {
                "market": row["market"],
                "index": row["index"],
                "code": meta["code"],
                "mask": int(row["mask"] or 0),
                "flags": flags,
                "missing": [
                    name
                    for name in ("close", "open", "volume", "amount", "bid1", "last")
                    if not flags[name]
                ],
                "bid_kind": lv.bid_kind(bid_raw, proj["bid1"], q["bid1"]),
                "incoming": proj,
                "oem": {
                    "price": q["price"],
                    "open": q["open"],
                    "volume": q["volume"],
                    "amount": q["amount"],
                    "bid1": q["bid1"],
                    "last_close": q["last_close"],
                },
            }
        )

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_joined_live_amount_hits.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "n": len(hits),
        "all_six": sum(1 for h in hits if not h["missing"]),
        "rows": hits,
        "product_wiring": [
            "The 3 live amount hits are not public OEM quotes; they still miss other fields.",
            "Do not publish persist 311B as auction live. Night dump all_six remains 5166/5166.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
