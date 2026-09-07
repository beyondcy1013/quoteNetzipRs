#!/usr/bin/env python3
"""Correct two-map snapshot recipe vs the stale same-table wiring.

Runtime 20:23 builds symbol_codes and previous_close_seeds from ONE table.
The product recipe is: index→code from the decode-session 0104, last/scale/
name from the same-day login 0104 by (market, code).

This scores end-of-stream persist projectable quotes under that split
against last Wine OEM. Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-correct-two-map-snapshot.py EXTRACT CALLBACK META_0903 MORNING_0104
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
snap = SourceFileLoader(
    "score_runtime_2023_snapshot_would_publish",
    os.path.join(_DIR, "score-runtime-2023-snapshot-would-publish.py"),
).load_module()


def star_prefix(code: str) -> bool:
    return len(code) >= 3 and code[:3] in ("688", "689")


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-correct-two-map-snapshot.py EXTRACT "
            "CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    identity = seed.load_index_metadata(meta_0903)
    symbol_codes = {key: row["code"] for key, row in identity.items()}
    morning_tables = two.load_all_tables(morning_dir)
    _, morning_seeds, _ = two.simulate_runtime_maps(morning_tables)
    rows, needed = seed.load_decoded(extract_dir)

    census = Counter()
    star_cross = 0
    for market, idx in needed:
        census["decoded_indexes"] += 1
        code = symbol_codes.get((market, idx))
        if code is None:
            census["no_decode_session_code"] += 1
            continue
        census["has_decode_session_code"] += 1
        row = morning_seeds.get((market, code))
        if row is None:
            census["no_morning_seed"] += 1
            continue
        census["has_both"] += 1

    states, mapped = snap.persist_states(rows, symbol_codes, morning_seeds)
    oem = snap.last_oem(callback_jsonl)
    scored = snap.score(states, mapped, identity, oem, "decode_session_index_plus_morning_by_code")

    for key, meta in mapped.items():
        pub = meta["code"]
        true_code = identity.get(key, {}).get("code")
        if true_code and star_prefix(pub) != star_prefix(true_code):
            star_cross += 1

    stale_path = os.path.join(
        os.path.dirname(os.path.abspath(extract_dir.rstrip("/"))),
        "runtime-2023-snapshot-would-publish.json",
    )
    stale = {}
    if os.path.isfile(stale_path):
        blob = json.loads(open(stale_path, "rb").read())
        stale = blob.get("auction_stale_0915") or {}

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_correct_two_map_snapshot.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "recipe": (
            "symbol_codes from decode-session 0104 (09-03 identity); "
            "previous_close_seeds from same-day login 0104 by (market, code)"
        ),
        "census_wine_tail_indexes": dict(census),
        "star_prefix_cross_on_mapped_states": star_cross,
        "end_of_stream": scored,
        "delta_vs_stale_same_table": {
            "stale_wrong_ticker": stale.get("wrong_ticker"),
            "correct_wrong_ticker": scored["wrong_ticker"],
            "stale_last_eq_true_oem": stale.get("last_eq_true_oem"),
            "correct_last_eq_true_oem": scored["last_eq_true_oem"],
            "stale_last_eq_published_oem": stale.get("last_eq_published_oem"),
            "correct_last_eq_published_oem": scored["last_eq_published_oem"],
            "stale_would_publish": stale.get("would_publish"),
            "correct_would_publish": scored["would_publish"],
        },
        "product_wiring": [
            "Runtime still fills both maps from one table. Splitting them is a caller/lifecycle change, not a bitstream change.",
            "Decode-session index→code plus morning-by-code last_close is the snapshot identity recipe.",
            "Do not treat last_eq_published_oem as identity: the stale same-table path is 99.5% self-consistent and still wrong.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
