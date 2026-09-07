#!/usr/bin/env python3
"""Day-cut last_close sources: night close vs first auction 2704 close.

Overnight OEM last is 0x12b. Next-day OEM last should be yesterday's +0x10.
Product still needs a snapshot when there is no night /proc dump. This checks
whether the first 2704 close in a mid-session auction capture is that snapshot.

usage:
  daycut-last-close-sources.py EXTRACT CALLBACK META_0903 PROBE
"""
from __future__ import annotations

import json
import sys
from collections import Counter

_DIR = __import__("os").path.dirname(__import__("os").path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    __import__("os").path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()


def score(left: float, right: float) -> bool:
    return seed.same_f32(left, right)


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: daycut-last-close-sources.py EXTRACT CALLBACK META_0903 PROBE",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, probe_json = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    probe = seed.load_probe_by_code(probe_json)
    rows, needed = seed.load_decoded(extract_dir)
    wanted = set()
    for market, index in needed:
        meta = metadata.get((market, index))
        if meta:
            wanted.add((market, meta["code"]))
    quotes, n_batches = seed.stream_quotes(callback_jsonl, wanted)
    lasts = seed.callback_last_mode(quotes)

    first_close: dict[tuple[str, str], int] = {}
    first_12b: dict[tuple[str, str], int] = {}
    first_ts: dict[tuple[str, str], int] = {}
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        key = (row["market"], meta["code"])
        close = seed.i32(row["rec"], 0x10)
        last_12b = seed.i32(row["rec"], 0x12B)
        if key not in first_close and close != 0:
            first_close[key] = close
            first_ts[key] = row["ts"]
        if key not in first_12b and last_12b != 0:
            first_12b[key] = last_12b

    hits = Counter()
    samples = []
    keys = sorted(lasts)
    for key in keys:
        cb = lasts[key]
        scale = None
        for (market, index), m in metadata.items():
            if (m["market"], m["code"]) == key:
                scale = m["scale"]
                break
        if not scale:
            hits["no_scale"] += 1
            continue
        hits["codes"] += 1
        night = probe.get(key)
        night_close = None if night is None else int(night["close"])
        night_12b = None if night is None else int(night["last_close"])
        src = {
            "night_close": None if night_close is None else seed.rust_scaled(night_close, scale),
            "night_12b": None if night_12b is None else seed.rust_scaled(night_12b, scale),
            "first_2704_close": None
            if key not in first_close
            else seed.rust_scaled(first_close[key], scale),
            "first_2704_12b": None
            if key not in first_12b
            else seed.rust_scaled(first_12b[key], scale),
        }
        for name, value in src.items():
            if value is None:
                hits[f"{name}_missing"] += 1
                continue
            hits[f"{name}_present"] += 1
            if score(value, cb):
                hits[f"{name}_hit"] += 1
        if (
            night_close is not None
            and key in first_close
            and first_close[key] == night_close
        ):
            hits["first_equals_night_close"] += 1
        if (
            night_close is not None
            and key in first_close
            and first_close[key] != night_close
        ):
            hits["first_moved_from_night_close"] += 1
            if len(samples) < 8 and not (
                src["first_2704_close"] is not None and score(src["first_2704_close"], cb)
            ):
                samples.append(
                    {
                        "code": key[1],
                        "cb_last": cb,
                        "night_close": src["night_close"],
                        "first_close": src["first_2704_close"],
                        "first_12b": src["first_2704_12b"],
                        "first_ts": first_ts.get(key),
                    }
                )

    n = hits["codes"] or 1

    def rate(name: str) -> float:
        present = hits[f"{name}_present"] or 1
        return round(hits[f"{name}_hit"] / present, 4)

    report = {
        "callback_batches": n_batches,
        "codes_with_callback_last": hits["codes"],
        "sources": {
            "night_close_+0x10": {
                "present": hits["night_close_present"],
                "hits": hits["night_close_hit"],
                "rate": rate("night_close"),
            },
            "night_0x12b": {
                "present": hits["night_12b_present"],
                "hits": hits["night_12b_hit"],
                "rate": rate("night_12b"),
            },
            "first_auction_2704_close": {
                "present": hits["first_2704_close_present"],
                "hits": hits["first_2704_close_hit"],
                "rate": rate("first_2704_close"),
            },
            "first_auction_2704_0x12b": {
                "present": hits["first_2704_12b_present"],
                "hits": hits["first_2704_12b_hit"],
                "rate": rate("first_2704_12b"),
            },
        },
        "first_vs_night_close": {
            "equal": hits["first_equals_night_close"],
            "moved": hits["first_moved_from_night_close"],
        },
        "moved_samples": samples,
        "note": (
            "Day-cut last_close is night +0x10. First mid-session 2704 close is only "
            "a valid snapshot when it has not already become today's price."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
