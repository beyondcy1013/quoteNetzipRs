#!/usr/bin/env python3
"""19:48 not-ready vs stale-full-table poison, plus auction datetime hours.

Runtime now skips OemState merge when (market, index) has no seed.
Empty tables become not-ready. A stale complete 0104 still looks present.

Does not edit official_5188.rs or runtime.

usage:
  score-not-ready-vs-stale-full.py EXTRACT META_0903 MORNING_0104
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()

TZ8 = timezone(timedelta(hours=8))


def index_map(by_code: dict) -> dict[tuple[str, int], dict]:
    out = {}
    for (market, code), row in by_code.items():
        out[(market, row["index"])] = {**row, "code": code, "market": market}
    return out


def hour_of(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%H:%M")


def needs_close_floor(ts: int) -> bool:
    dt = datetime.fromtimestamp(ts, TZ8)
    return dt.hour > 15 or (dt.hour == 15 and (dt.minute > 0 or dt.second > 0))


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-not-ready-vs-stale-full.py EXTRACT META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, meta_0903, morning_dir = sys.argv[1:]
    morning_by_index = index_map(seed.load_largest_0104(morning_dir))
    meta_by_index = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)

    empty = Counter()
    stale = Counter()
    hours = Counter()
    minutes = Counter()
    floor = Counter()
    for row in rows:
        ts = int(row["ts"])
        hours[datetime.fromtimestamp(ts, TZ8).strftime("%H")] += 1
        minutes[hour_of(ts)[:4] + "0"] += 1  # 10-minute buckets
        if needs_close_floor(ts):
            floor["needs_1500_floor"] += 1
        empty["records"] += 1
        empty["not_ready_skip"] += 1  # no tables at all
        key = (row["market"], row["index"])
        stale_row = morning_by_index.get(key)
        meta = meta_by_index.get(key)
        if stale_row is None:
            stale["not_ready_skip"] += 1
            continue
        stale["would_merge"] += 1
        if meta is None:
            stale["merge_unknown_code"] += 1
        elif stale_row["code"] == meta["code"]:
            stale["merge_same_code"] += 1
        else:
            stale["merge_wrong_code"] += 1

    report = {
        "schema": "quoteNetzipRs.official_5188_not_ready_vs_stale_full.v1",
        "extract_dir": extract_dir,
        "decoded_records": len(rows),
        "runtime_1948": (
            "merge_oem_records continues without OemState::new(0) when the "
            "(market, index) seed is absent"
        ),
        "empty_same_session_tables": {
            "not_ready_skip_records": empty["not_ready_skip"],
            "oem_state_updates": 0,
            "note": "wine-tail extract has 0 0104 tables; 19:48 not-ready covers this path",
        },
        "stale_morning_0104_by_index": dict(stale),
        "auction_internal_hour": dict(sorted(hours.items())),
        "auction_internal_10min": dict(sorted(minutes.items())),
        "needs_1500_floor_records": floor["needs_1500_floor"],
        "note": (
            "19:48 not-ready is empty/partial tables. Stale complete 0104 still "
            "merges every record (0 skips) with mostly wrong last_close. "
            "Auction timestamps should not need the 15:00:00 crate floor."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
