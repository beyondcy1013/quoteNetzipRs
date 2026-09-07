#!/usr/bin/env python3
"""Night dump persist all_six vs auction all_six=0.

Section 61 auction persist-clean OEM-live is all_six 0, including bid1 0/5
on first-prints. If the same project_state on the 01:19 dump is ~5166, the
bid1 offset is correct and auction bid1 garbage is leftover, not a getter bug.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-night-dump-all-six.py NIGHT_EXTRACT NIGHT_0104 NIGHT_CALLBACK
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
fp = SourceFileLoader(
    "score_runtime_2023_first_print_fields",
    os.path.join(_DIR, "score-runtime-2023-first-print-fields.py"),
).load_module()


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-runtime-2023-night-dump-all-six.py "
            "NIGHT_EXTRACT NIGHT_0104 NIGHT_CALLBACK",
            file=sys.stderr,
        )
        return 2
    night_extract, night_0104, night_callback = sys.argv[1:]
    tables = two.load_all_tables(night_0104)
    symbol_codes, seeds, _ = two.simulate_runtime_maps(tables)
    rows, _ = seed.load_decoded(night_extract)
    states, mapped = combo.persist_states(rows, symbol_codes, seeds, False)
    oem = fp.last_oem_full(night_callback, min_seq=21)

    hits = fp.empty_hits()
    no_oem = 0
    leftover_oem = 0
    dirty = 0
    samples_miss = []
    for key, st in states.items():
        if seed.i64(bytes(st), 0x1C) < 0:
            dirty += 1
            continue
        meta = mapped[key]
        q = oem.get((key[0], meta["code"]))
        if q is None:
            no_oem += 1
            continue
        if q["price"] == 0:
            leftover_oem += 1
            continue
        proj = fp.project_state(bytes(st), meta)
        flags = fp.score_one(proj, q, hits)
        if (
            not (
                flags["close"]
                and flags["open"]
                and flags["volume"]
                and flags["amount"]
                and flags["bid1"]
                and flags["last"]
            )
            and len(samples_miss) < 4
        ):
            samples_miss.append(
                {
                    "market": key[0],
                    "index": key[1],
                    "code": meta["code"],
                    "flags": flags,
                    "state_bid1": proj["bid1"],
                    "oem_bid1": q["bid1"],
                    "state_close": proj["close"],
                    "oem_price": q["price"],
                }
            )

    auction_path = (
        "/home/codes/stock/quoteNetzipRs/diagnostics/20260904-live-rust/"
        "auction-0924/runtime-2023-first-print-fields.json"
    )
    auction_summary = {}
    if os.path.isfile(auction_path):
        raw = json.loads(open(auction_path, "rb").read())
        groups = raw.get("groups") or {}
        auction_summary = {
            "oem_live": raw.get("oem_live"),
            "all_six_by_group": {
                name: {
                    "n": g.get("n"),
                    "all_six": g.get("all_six"),
                    "bid1": g.get("bid1"),
                }
                for name, g in groups.items()
            },
        }

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_night_dump_all_six.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "crate_path": "crates/netzip-fullpull/src/official_5188.rs",
        "night_persist_oem_live": fp.pack(hits),
        "night_no_oem": no_oem,
        "night_oem_price_zero": leftover_oem,
        "night_dirty": dirty,
        "samples_not_all_six": samples_miss,
        "auction_from_section_61": auction_summary,
        "product_wiring": [
            "Same project_state bid1 getter: night dump should match OEM book; auction first-print bid1 garbage is leftover.",
            "Do not treat auction bid1 misses as a 0x68 offset defect if night all_six is high.",
            "Do not publish persist 311B as auction live.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
