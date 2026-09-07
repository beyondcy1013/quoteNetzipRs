#!/usr/bin/env python3
"""Split auction joins: global 09:25 zero vs per-symbol first live print.

Does not edit official_5188.rs. Recipe v2 said trade fields are 0 before
09:25; auction live prints at 09:24 contradict a global gate.

usage:
  classify-preopen-vs-first-print.py EXTRACT CALLBACK META_0903
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
ident = SourceFileLoader(
    "disambiguate_586_batch_identity",
    os.path.join(_DIR, "disambiguate-586-batch-identity.py"),
).load_module()

TZ8 = timezone(timedelta(hours=8))
GATE = int(datetime(2026, 9, 4, 9, 25, 0, tzinfo=TZ8).timestamp())


def pick_later(cands: list[dict]) -> dict:
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: classify-preopen-vs-first-print.py EXTRACT CALLBACK META_0903",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)

    cb_by = defaultdict(list)
    first_live: dict[tuple[str, str], int] = {}
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)
        if q["price"] != 0:
            key = (q["market"], q["code"])
            prev = first_live.get(key)
            if prev is None or q["cb_ts"] < prev:
                first_live[key] = q["cb_ts"]

    hits = Counter()
    hour = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        picked = pick_later(cands)
        live = picked["price"] != 0
        before_gate = row["ts"] < GATE
        first = first_live.get((row["market"], meta["code"]))
        hits["live" if live else "leftover"] += 1
        hits["before_0925" if before_gate else "at_or_after_0925"] += 1
        if live and before_gate:
            hits["live_before_0925"] += 1
        if live and not before_gate:
            hits["live_at_or_after_0925"] += 1
        if (not live) and before_gate:
            hits["leftover_before_0925"] += 1
        if (not live) and not before_gate:
            hits["leftover_at_or_after_0925"] += 1
            if first is None:
                hits["leftover_after_0925_never_live"] += 1
            elif row["ts"] < first:
                hits["leftover_after_0925_waiting"] += 1
        hour[datetime.fromtimestamp(row["ts"], TZ8).strftime("%H:%M")] += 1
        if first is None:
            hits["code_never_live"] += 1
            if live:
                hits["live_but_first_missing"] += 1
        elif row["ts"] < first:
            hits["before_first_live"] += 1
            if live:
                hits["live_before_own_first"] += 1
            else:
                hits["leftover_before_own_first"] += 1
        else:
            hits["at_or_after_first_live"] += 1
            if live:
                hits["live_at_or_after_own_first"] += 1
            else:
                hits["leftover_after_own_first"] += 1
        if live and before_gate and len(samples) < 6:
            samples.append(
                {
                    "code": meta["code"],
                    "internal_fmt": datetime.fromtimestamp(row["ts"], TZ8).strftime(
                        "%Y-%m-%d %H:%M:%S"
                    ),
                    "cb_price": picked["price"],
                    "first_live_fmt": datetime.fromtimestamp(first, TZ8).strftime(
                        "%Y-%m-%d %H:%M:%S"
                    )
                    if first
                    else None,
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_preopen_vs_first_print.v1",
        "extract_dir": extract_dir,
        "gate": "2026-09-04 09:25:00 Asia/Shanghai",
        "joined": hits["joined"],
        "counts": dict(hits),
        "joined_minute_head": dict(hour.most_common(8)),
        "samples_live_before_0925": samples,
        "note": (
            "A global pre-09:25 trade-field zero would drop live_before_0925 "
            "prints. Per-symbol leftover lasts until that code's first live "
            "callback, not until a wall 09:25 gate."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
