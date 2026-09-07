#!/usr/bin/env python3
"""Same-session night 0104 index identity + datetime floor exactness.

Positive control for runtime (market, index) seeding: the 01:19 Wine session
has its own 0104 tables. Contrast with auction wine-tail stale-index poison.
Also scores crate remaining datetime: exact match after >15:00:00 floor,
and whether that floor would smash a non-15:00:00 OEM datetime.

Does not edit official_5188.rs.

usage:
  score-night-same-session-index-and-datetime.py CALLBACK PROBE NIGHT_0104
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
CLOSE = datetime(2026, 9, 3, 15, 0, 0, tzinfo=TZ8)


def floor_close(ts: int) -> int:
    dt = datetime.fromtimestamp(ts, TZ8)
    if dt.hour > 15 or (dt.hour == 15 and (dt.minute > 0 or dt.second > 0)):
        return int(dt.replace(hour=15, minute=0, second=0, microsecond=0).timestamp())
    return ts


def needs_floor(ts: int) -> bool:
    return floor_close(ts) != ts


def index_map(by_code: dict) -> dict[tuple[str, int], dict]:
    out = {}
    for (market, code), row in by_code.items():
        out[(market, row["index"])] = {**row, "code": code, "market": market}
    return out


def load_probe(path: str) -> dict[tuple[str, str], dict]:
    best: dict[tuple[str, str], dict] = {}
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
    if len(sys.argv) != 4:
        print(
            "usage: score-night-same-session-index-and-datetime.py CALLBACK PROBE NIGHT_0104",
            file=sys.stderr,
        )
        return 2
    callback_jsonl, probe_json, night_0104 = sys.argv[1:]
    by_code = seed.load_largest_0104(night_0104)
    by_index = index_map(by_code)
    probe = load_probe(probe_json)
    oem = load_oem(callback_jsonl)

    ident = Counter()
    last = Counter()
    dt = Counter()
    samples_ident = []
    samples_false_floor = []

    for key, row in probe.items():
        q = oem.get(key)
        if q is None:
            ident["probe_without_oem"] += 1
            continue
        ident["compared"] += 1
        market, code = key
        idx = int(row["idx"])
        scale = float(row.get("scale") or 0)
        table_idx = by_index.get((market, idx))
        table_code = by_code.get(key)
        if table_idx is None:
            ident["index_missing_in_same_session_0104"] += 1
        elif table_idx["code"] == code:
            ident["index_same_code"] += 1
        else:
            ident["index_wrong_code"] += 1
            if len(samples_ident) < 6:
                samples_ident.append(
                    {
                        "probe_code": code,
                        "index": idx,
                        "table_code": table_idx["code"],
                    }
                )

        cb_last = float(q.get("last_close") or 0)
        if scale > 0 and table_code:
            proj = seed.rust_scaled(table_code["last_0104"], table_code["scale"] or scale)
            last["by_code_hit" if seed.same_f32(proj, cb_last) else "by_code_miss"] += 1
        else:
            last["by_code_missing"] += 1
        if scale > 0 and table_idx:
            proj = seed.rust_scaled(table_idx["last_0104"], table_idx["scale"] or scale)
            last["by_index_hit" if seed.same_f32(proj, cb_last) else "by_index_miss"] += 1
        else:
            last["by_index_missing"] += 1
        if scale > 0:
            use_scale = float(table_code["scale"] or scale) if table_code else scale
            live = seed.rust_scaled(int(row["last_close"]), use_scale)
            last["x12b_hit" if seed.same_f32(live, cb_last) else "x12b_miss"] += 1

        ts = int(row["ts"])
        oem_ts = seed.parse_callback_ts(q.get("datetime") or "")
        if oem_ts is None:
            dt["oem_unparsed"] += 1
            continue
        dt["datetime_compared"] += 1
        if oem_ts == ts:
            dt["raw_exact"] += 1
        if abs(oem_ts - ts) <= 1:
            dt["raw_within_1s"] += 1
        floored = floor_close(ts)
        if needs_floor(ts):
            dt["needs_floor"] += 1
            oem_dt = q.get("datetime") or ""
            if not oem_dt.endswith("15:00:00"):
                dt["false_floor"] += 1
                if len(samples_false_floor) < 6:
                    samples_false_floor.append(
                        {
                            "code": code,
                            "internal": datetime.fromtimestamp(ts, TZ8).strftime("%H:%M:%S"),
                            "oem": oem_dt,
                        }
                    )
        if oem_ts == floored:
            dt["floor_exact"] += 1
        if abs(oem_ts - floored) <= 1:
            dt["floor_within_1s"] += 1

    n = ident["compared"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_night_same_session_index_datetime.v1",
        "callback": callback_jsonl,
        "probe": probe_json,
        "compared": ident["compared"],
        "same_session_index": {
            "same_code": ident["index_same_code"],
            "wrong_code": ident["index_wrong_code"],
            "missing": ident["index_missing_in_same_session_0104"],
            "same_code_rate": round(ident["index_same_code"] / n, 6),
        },
        "last_close_vs_oem": dict(last),
        "datetime": dict(dt),
        "samples_index_mismatch": samples_ident,
        "samples_false_floor": samples_false_floor,
        "note": (
            "Same-session 0104 keyed by index should match probe codes. "
            "Stale cross-session index is the opposite (section 41). "
            "Crate to_public_quote still emits raw timestamp(); floor exact "
            "is the remaining projection datetime gap."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
