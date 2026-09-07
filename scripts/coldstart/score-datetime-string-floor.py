#!/usr/bin/env python3
"""Crate datetime landing spec: floor u32 then format Asia/Shanghai.

Official5188PublicQuote.timestamp is a unix seconds u32. Wine OEM datetime
is local 'YYYY-MM-DD HH:MM:SS'. Floor local time >15:00:00 to 15:00:00
before publishing the u32.

Does not edit official_5188.rs.

usage:
  score-datetime-string-floor.py CALLBACK PROBE
"""
from __future__ import annotations

import json
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone

_DIR = __import__("os").path.dirname(__import__("os").path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    __import__("os").path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()

TZ8 = timezone(timedelta(hours=8))


def floor_ts(ts: int) -> int:
    dt = datetime.fromtimestamp(ts, TZ8)
    if dt.hour > 15 or (dt.hour == 15 and (dt.minute > 0 or dt.second > 0)):
        dt = dt.replace(hour=15, minute=0, second=0, microsecond=0)
        return int(dt.timestamp())
    return ts


def fmt(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%Y-%m-%d %H:%M:%S")


def load_probe(path: str) -> dict[tuple[str, str], dict]:
    best = {}
    for row in json.loads(open(path, "rb").read()):
        code = row.get("code") or ""
        market = row.get("market") or ""
        if not code:
            continue
        key = (market, code)
        prev = best.get(key)
        if prev is None or int(row.get("frame_index") or 0) >= int(prev.get("frame_index") or 0):
            best[key] = row
    return best


def load_oem(path: str) -> dict[tuple[str, str], dict]:
    post = {}
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch or int(ev.get("sequence") or 0) < 21:
                continue
            for q in batch.get("quotes") or []:
                post[(q["market"], q["code"])] = q
    return post


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: score-datetime-string-floor.py CALLBACK PROBE", file=sys.stderr)
        return 2
    callback_jsonl, probe_json = sys.argv[1:]
    probe = load_probe(probe_json)
    oem = load_oem(callback_jsonl)
    hits = Counter()
    prefixes = Counter()
    samples = []
    for key, row in probe.items():
        q = oem.get(key)
        if q is None:
            continue
        hits["compared"] += 1
        ts = int(row["ts"])
        oem_dt = q.get("datetime") or ""
        raw = fmt(ts)
        floored_ts = floor_ts(ts)
        floored = fmt(floored_ts)
        if raw == oem_dt:
            hits["raw_string_exact"] += 1
        if floored == oem_dt:
            hits["floor_string_exact"] += 1
        else:
            if len(samples) < 6:
                samples.append(
                    {"code": key[1], "raw": raw, "floored": floored, "oem": oem_dt}
                )
        if floored_ts != ts:
            hits["u32_changed"] += 1
            prefixes[key[1][:3]] += 1
    n = hits["compared"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_datetime_string_floor.v1",
        "compared": hits["compared"],
        "raw_string_exact": hits["raw_string_exact"],
        "floor_string_exact": hits["floor_string_exact"],
        "u32_changed": hits["u32_changed"],
        "floor_prefixes": dict(prefixes),
        "crate_spec": (
            "In to_public_quote, after reading timestamp(), if Asia/Shanghai "
            "local time is after 15:00:00, replace the u32 with 15:00:00 that "
            "civil day. Do not change the formatter; PublicQuote.timestamp "
            "stays unix seconds. Wine callback datetime is UTC+8."
        ),
        "samples_floor_string_miss": samples,
        "rates": {
            "raw": round(hits["raw_string_exact"] / n, 6),
            "floor": round(hits["floor_string_exact"] / n, 6),
        },
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
