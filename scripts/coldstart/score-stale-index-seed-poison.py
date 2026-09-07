#!/usr/bin/env python3
"""Stale (market, index) 0104 seeds: missing vs silent wrong last_close.

Runtime keys previous_close by (market, symbol_index) and unwrap_or(0) only
when the index is absent. Reusing a previous login's full 0104 looks present
and will not increment decoder_missing_previous_close_seeds.

Does not edit official_5188.rs.

usage:
  score-stale-index-seed-poison.py EXTRACT META_0903 MORNING_0104
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


def index_map(by_code: dict) -> dict[tuple[str, int], dict]:
    out = {}
    for (market, code), row in by_code.items():
        out[(market, row["index"])] = {**row, "code": code, "market": market}
    return out


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-stale-index-seed-poison.py EXTRACT META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, meta_0903, morning_dir = sys.argv[1:]
    morning_by_code = seed.load_largest_0104(morning_dir)
    morning_by_index = index_map(morning_by_code)
    meta_by_index = seed.load_index_metadata(meta_0903)
    rows, needed = seed.load_decoded(extract_dir)

    symbols = Counter()
    samples = []
    for market, index in sorted(needed):
        meta = meta_by_index.get((market, index))
        stale = morning_by_index.get((market, index))
        if meta is None:
            symbols["extract_index_unknown_to_0903"] += 1
            continue
        symbols["extract_indexes"] += 1
        if stale is None:
            symbols["stale_missing_would_unwrap_0"] += 1
            continue
        symbols["stale_present_no_missing_counter"] += 1
        if stale["code"] == meta["code"]:
            symbols["stale_same_code"] += 1
        else:
            symbols["stale_wrong_code"] += 1
            if stale.get("last_0104") == meta.get("last_0104"):
                symbols["stale_wrong_code_same_i32"] += 1
            else:
                symbols["stale_wrong_code_diff_i32"] += 1
            if len(samples) < 8:
                samples.append(
                    {
                        "market": market,
                        "index": index,
                        "extract_code_via_0903": meta["code"],
                        "stale_morning_code": stale["code"],
                        "extract_last_i32": meta.get("last_0104"),
                        "stale_last_i32": stale.get("last_0104"),
                    }
                )

    tables = 0
    for name in os.listdir(extract_dir):
        if name.endswith("0104.code-table.json"):
            tables += 1

    n = symbols["extract_indexes"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_stale_index_seed_poison.v1",
        "extract_dir": extract_dir,
        "unique_indexes_in_2704": symbols["extract_indexes"],
        "decoded_records": len(rows),
        "extract_own_0104_tables": tables,
        "stale_morning_0104_by_index": {
            "missing_unwrap_or_0": symbols["stale_missing_would_unwrap_0"],
            "present_silent": symbols["stale_present_no_missing_counter"],
            "same_code": symbols["stale_same_code"],
            "wrong_code": symbols["stale_wrong_code"],
            "wrong_code_same_last_i32": symbols["stale_wrong_code_same_i32"],
            "wrong_code_diff_last_i32": symbols["stale_wrong_code_diff_i32"],
            "wrong_code_rate": round(symbols["stale_wrong_code"] / n, 6),
        },
        "samples_wrong_code": samples,
        "note": (
            "Wine-tail extract has no same-session 0104 tables. Reusing the "
            "09:15 login 0104 by (market, index) almost never looks missing; "
            "it plants another code's last_close. missing_previous_close_seeds "
            "would stay 0. Rebuild from the new login; key product maps by "
            "(market, code). Runtime still unwrap_or(0) when truly absent."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
