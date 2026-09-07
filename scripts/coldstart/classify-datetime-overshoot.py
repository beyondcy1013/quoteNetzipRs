#!/usr/bin/env python3
"""Classify the 39 night OEM datetime misses beyond ±1s.

usage:
  classify-datetime-overshoot.py CALLBACK PROBE NIGHT_0104
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
CLOSE = "15:00:00"


def fmt_ts(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%Y-%m-%d %H:%M:%S")


def floor_close(ts: int) -> int:
    dt = datetime.fromtimestamp(ts, TZ8)
    if dt.hour > 15 or (dt.hour == 15 and (dt.minute > 0 or dt.second > 0)):
        closed = dt.replace(hour=15, minute=0, second=0, microsecond=0)
        return int(closed.timestamp())
    return ts


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: classify-datetime-overshoot.py CALLBACK PROBE NIGHT_0104",
            file=sys.stderr,
        )
        return 2
    callback_jsonl, probe_json, night_0104 = sys.argv[1:]
    night_meta = seed.load_largest_0104(night_0104)
    probe = {}
    for row in json.loads(open(probe_json, "rb").read()):
        code = row.get("code") or ""
        if not code:
            continue
        key = (row["market"], code)
        prev = probe.get(key)
        if prev is None or int(row.get("frame_index") or 0) >= int(prev.get("frame_index") or 0):
            probe[key] = row
    post = {}
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch or int(ev.get("sequence") or 0) < 21:
                continue
            for q in batch.get("quotes") or []:
                post[(q["market"], q["code"])] = q

    deltas = Counter()
    over = []
    compared = 0
    exact = 0
    within_1 = 0
    floor_hits = 0
    oem_is_close = 0
    prefixes = Counter()
    for key, row in probe.items():
        oem = post.get(key)
        meta = night_meta.get(key)
        if oem is None or not meta or meta["scale"] <= 0:
            continue
        compared += 1
        ts = int(row["ts"])
        oem_ts = seed.parse_callback_ts(oem.get("datetime") or "")
        if oem_ts is None:
            continue
        delta = oem_ts - ts
        deltas[str(delta)] += 1
        if delta == 0:
            exact += 1
        if abs(delta) <= 1:
            within_1 += 1
        oem_dt = oem.get("datetime") or ""
        if oem_dt.endswith(CLOSE):
            oem_is_close += 1
        floored = floor_close(ts)
        if abs(oem_ts - floored) <= 1 or oem_ts == floored:
            floor_hits += 1
        if abs(delta) > 1:
            prefixes[key[1][:3]] += 1
            over.append(
                {
                    "market": key[0],
                    "code": key[1],
                    "internal_ts": ts,
                    "internal_fmt": fmt_ts(ts),
                    "oem_dt": oem_dt,
                    "delta": delta,
                    "oem_is_150000": oem_dt.endswith(CLOSE),
                    "floor_close_fmt": fmt_ts(floored),
                    "floor_matches": floored == oem_ts,
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_datetime_overshoot.v1",
        "compared": compared,
        "exact": exact,
        "within_1s": within_1,
        "oem_datetime_ends_150000": oem_is_close,
        "after_floor_close_hits": floor_hits,
        "over_1s": len(over),
        "delta_histogram": dict(sorted(deltas.items(), key=lambda kv: int(kv[0]))),
        "over_prefixes": dict(prefixes),
        "over_all_oem_150000": all(row["oem_is_150000"] for row in over),
        "over_all_floor_matches": all(row["floor_matches"] for row in over),
        "overshoot": over,
        "note": (
            "Negative delta means OEM is earlier than 311B. If every >1s miss "
            "is an internal time after 15:00:00 with OEM 15:00:00, floor the "
            "published close second."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
