#!/usr/bin/env python3
"""Read-only check of crate 20:02 public_timestamp vs night OEM join.

official_5188.rs now publishes public_timestamp() and leaves timestamp() on
the wire. This reimplements that integer UTC+8 close-second clamp and scores
it against Wine OEM datetime, without editing owned files.

usage:
  score-crate-public-timestamp.py NIGHT_CALLBACK NIGHT_PROBE EXTRACT CALLBACK META_0903
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
CHINA_UTC_OFFSET_SECONDS = 8 * 60 * 60
CLOSE_SECONDS = 15 * 60 * 60
DAY_SECONDS = 24 * 60 * 60


def crate_public_timestamp(ts: int) -> int:
    timestamp = int(ts)
    local_seconds = timestamp + CHINA_UTC_OFFSET_SECONDS
    day = local_seconds // DAY_SECONDS if local_seconds >= 0 else -((-local_seconds) // DAY_SECONDS)
    # Python // is floor (same as div_euclid for these non-negative unix seconds)
    day = local_seconds // DAY_SECONDS
    close_timestamp = day * DAY_SECONDS + CLOSE_SECONDS - CHINA_UTC_OFFSET_SECONDS
    if timestamp > close_timestamp and close_timestamp >= 0:
        return close_timestamp
    return timestamp


def datetime_floor(ts: int) -> int:
    dt = datetime.fromtimestamp(ts, TZ8)
    if dt.hour > 15 or (dt.hour == 15 and (dt.minute > 0 or dt.second > 0)):
        dt = dt.replace(hour=15, minute=0, second=0, microsecond=0)
        return int(dt.timestamp())
    return ts


def fmt(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%Y-%m-%d %H:%M:%S")


def probe_ts(row: dict) -> int | None:
    if row.get("ts") is not None:
        return int(row["ts"])
    raw_hex = row.get("record_hex") or ""
    if len(raw_hex) >= 8:
        raw = bytes.fromhex(raw_hex[:8])
        return int.from_bytes(raw, "little")
    return None


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


def load_oem(path: str, min_seq: int) -> dict[tuple[str, str], dict]:
    post = {}
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch or int(ev.get("sequence") or 0) < min_seq:
                continue
            for q in batch.get("quotes") or []:
                post[(q["market"], q["code"])] = q
    return post


def main() -> int:
    if len(sys.argv) != 6:
        print(
            "usage: score-crate-public-timestamp.py NIGHT_CALLBACK NIGHT_PROBE "
            "EXTRACT CALLBACK META_0903",
            file=sys.stderr,
        )
        return 2
    night_cb, night_probe, extract_dir, callback_jsonl, meta_0903 = sys.argv[1:]
    probe = load_probe(night_probe)
    oem = load_oem(night_cb, 21)
    ident = SourceFileLoader(
        "disambiguate_586_batch_identity",
        __import__("os").path.join(_DIR, "disambiguate-586-batch-identity.py"),
    ).load_module()
    meta = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)

    night = Counter()
    prefixes = Counter()
    samples = []
    crate_vs_dt = 0
    for key, row in probe.items():
        q = oem.get(key)
        if q is None:
            continue
        ts = probe_ts(row)
        if ts is None:
            night["no_ts"] += 1
            continue
        night["compared"] += 1
        pub = crate_public_timestamp(ts)
        dt_floor = datetime_floor(ts)
        if pub != dt_floor:
            crate_vs_dt += 1
        oem_dt = q.get("datetime") or ""
        oem_unix = seed.parse_callback_ts(oem_dt)
        if fmt(ts) == oem_dt:
            night["raw_string"] += 1
        if fmt(pub) == oem_dt:
            night["public_string"] += 1
        if oem_unix is not None and ts == oem_unix:
            night["raw_unix_join"] += 1
        if oem_unix is not None and pub == oem_unix:
            night["public_unix_join"] += 1
        if pub != ts:
            night["u32_changed"] += 1
            prefixes[key[1][:3]] += 1
            if len(samples) < 6:
                samples.append(
                    {
                        "code": key[1],
                        "raw": ts,
                        "public": pub,
                        "raw_fmt": fmt(ts),
                        "public_fmt": fmt(pub),
                        "oem": oem_dt,
                    }
                )

    auction = Counter()
    hours = Counter()
    for row in rows:
        meta_row = meta.get((row["market"], row["index"]))
        if meta_row is None:
            continue
        ts = int(row["ts"])
        pub = crate_public_timestamp(ts)
        auction["decoded"] += 1
        hours[datetime.fromtimestamp(ts, TZ8).strftime("%H")] += 1
        if pub != ts:
            auction["would_floor"] += 1

    # crate unit-test constants
    test_post = crate_public_timestamp(1_788_505_201)
    test_pre = crate_public_timestamp(1_788_505_199)

    n = night["compared"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_crate_public_timestamp.v1",
        "crate_mtime": "2026-09-04 20:02",
        "crate_facts": {
            "internal_timestamp_unchanged": True,
            "to_public_quote_uses_public_timestamp": True,
            "not_399_special_case": True,
            "unit_test_1788505201_clamps_to": test_post,
            "unit_test_1788505199_preserved": test_pre,
            "unit_test_expect": {"post": 1_788_505_200, "pre": 1_788_505_199},
        },
        "crate_algo_vs_datetime_floor_disagreements": crate_vs_dt,
        "night_probe_cap_oem_seq21": {
            "compared": night["compared"],
            "raw_string_exact": night["raw_string"],
            "public_string_exact": night["public_string"],
            "raw_unix_join": night["raw_unix_join"],
            "public_unix_join": night["public_unix_join"],
            "u32_changed": night["u32_changed"],
            "prefixes": dict(prefixes),
            "rates": {
                "raw_string": round(night["raw_string"] / n, 6),
                "public_string": round(night["public_string"] / n, 6),
                "raw_unix_join": round(night["raw_unix_join"] / n, 6),
                "public_unix_join": round(night["public_unix_join"] / n, 6),
            },
            "samples_floored": samples,
        },
        "auction_wine_tail": {
            "decoded_with_0903_meta": auction["decoded"],
            "would_floor": auction["would_floor"],
            "hours": dict(hours),
        },
        "join_rule": (
            "Join 2704 bitstream / OemState.merge on timestamp(); publish and "
            "OEM-datetime join on public_timestamp(). Do not floor the wire u32."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
