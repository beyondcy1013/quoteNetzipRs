#!/usr/bin/env python3
"""Night same-session dump: persist close should match OEM price.

Section 58 auction wine-tail persist-clean is 65/5,421 live close hits.
This scores the 01:19 same-session dump with the same persist replay:
when the 311B IS the public state, close==OEM price should be high.
That isolates leftover-vs-live from identity/projection.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-night-dump-close-vs-oem.py NIGHT_EXTRACT NIGHT_0104 NIGHT_CALLBACK
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
two = SourceFileLoader(
    "score_runtime_2010_two_step_seed",
    os.path.join(_DIR, "score-runtime-2010-two-step-seed.py"),
).load_module()
combo = SourceFileLoader(
    "score_runtime_2023_two_map_plus_rollback",
    os.path.join(_DIR, "score-runtime-2023-two-map-plus-rollback.py"),
).load_module()
snap = SourceFileLoader(
    "score_runtime_2023_snapshot_would_publish",
    os.path.join(_DIR, "score-runtime-2023-snapshot-would-publish.py"),
).load_module()
pc = SourceFileLoader(
    "score_runtime_2023_persist_clean_vs_oem_live",
    os.path.join(_DIR, "score-runtime-2023-persist-clean-vs-oem-live.py"),
).load_module()


def market_split(states, mapped, oem):
    by = Counter()
    for key, st in states.items():
        amount = seed.i64(bytes(st), 0x1C)
        if amount < 0:
            continue
        meta = mapped[key]
        q = oem.get((key[0], meta["code"]))
        if q is None or q["price"] == 0:
            continue
        close_f = seed.rust_scaled(seed.i32(bytes(st), 0x10), meta.get("scale") or 1.0)
        m = key[0]
        by[f"{m}_live"] += 1
        if seed.same_f32(close_f, q["price"]):
            by[f"{m}_close_eq"] += 1
    return dict(by)


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-runtime-2023-night-dump-close-vs-oem.py "
            "NIGHT_EXTRACT NIGHT_0104 NIGHT_CALLBACK",
            file=sys.stderr,
        )
        return 2
    night_extract, night_0104, night_callback = sys.argv[1:]
    tables = two.load_all_tables(night_0104)
    symbol_codes, seeds, _ = two.simulate_runtime_maps(tables)
    rows, _ = seed.load_decoded(night_extract)
    states, mapped = combo.persist_states(rows, symbol_codes, seeds, False)
    oem = snap.last_oem(night_callback, min_seq=21)
    scored = pc.classify(states, mapped, oem)
    scored["market_live_close_eq"] = market_split(states, mapped, oem)
    dirty = sum(1 for st in states.values() if seed.i64(bytes(st), 0x1C) < 0)

    auction_path = (
        "/home/codes/stock/quoteNetzipRs/diagnostics/20260904-live-rust/"
        "auction-0924/runtime-2023-persist-clean-vs-oem-live.json"
    )
    auction = {}
    if os.path.isfile(auction_path):
        auction = json.loads(open(auction_path, "rb").read()).get("persist_clean") or {}

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_night_dump_close_vs_oem.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "night_same_session_persist": scored,
        "night_still_dirty": dirty,
        "auction_persist_clean_from_section_58": {
            "has_oem": auction.get("has_oem"),
            "last_eq": auction.get("last_eq"),
            "oem_live_close_eq": auction.get("oem_live_close_eq"),
            "oem_live": auction.get("oem_live"),
        },
        "product_wiring": [
            "Same-session initial dump 311B is the public OEM state; mid-session wine-tail 311B is leftover vs auction live.",
            "Identity/last_close can be closed while live price stays a leftover/live join problem.",
            "Do not treat auction 65 close hits as a bitstream or projection defect.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
