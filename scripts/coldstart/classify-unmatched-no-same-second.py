#!/usr/bin/env python3
"""Classify auction decoded records with no same-second callback.

Does not widen the join window. Splits coverage misses from time-skew.

usage:
  classify-unmatched-no-same-second.py EXTRACT CALLBACK META_0903
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


def fmt_ts(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%Y-%m-%d %H:%M:%S")


def bucket_abs(delta: int) -> str:
    a = abs(delta)
    if a <= 1:
        return "1s"
    if a <= 5:
        return "2-5s"
    if a <= 30:
        return "6-30s"
    if a <= 60:
        return "31-60s"
    if a <= 300:
        return "1-5m"
    return ">5m"


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: classify-unmatched-no-same-second.py EXTRACT CALLBACK META_0903",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)

    cb_by_code: dict[tuple[str, str], list[int]] = defaultdict(list)
    cb_by_sec = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        key = (q["market"], q["code"])
        cb_by_code[key].append(q["cb_ts"])
        cb_by_sec[(q["market"], q["code"], q["cb_ts"])].append(q)

    for key in cb_by_code:
        cb_by_code[key] = sorted(set(cb_by_code[key]))

    hits = Counter()
    nearest_bucket = Counter()
    hour = Counter()
    prefixes = Counter()
    never_prefixes = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            hits["no_meta"] += 1
            continue
        hits["decoded_with_meta"] += 1
        code_key = (row["market"], meta["code"])
        sec_key = (row["market"], meta["code"], row["ts"])
        if cb_by_sec.get(sec_key):
            hits["same_second"] += 1
            continue
        hits["no_same_second"] += 1
        prefixes[meta["code"][:3]] += 1
        hour[fmt_ts(row["ts"])[11:13]] += 1
        times = cb_by_code.get(code_key)
        close = seed.i32(row["rec"], 0x10)
        plus = cb_by_sec.get((row["market"], meta["code"], row["ts"] + 1), [])
        minus = cb_by_sec.get((row["market"], meta["code"], row["ts"] - 1), [])
        uniq_sec = {q["cb_ts"] for q in plus + minus}
        if not uniq_sec:
            hits["pm1_empty"] += 1
        elif len(uniq_sec) == 1 and (not plus or not minus):
            hits["pm1_one_side"] += 1
            lives = sum(1 for q in plus + minus if q["price"] != 0)
            zeros = sum(1 for q in plus + minus if q["price"] == 0)
            if lives and zeros:
                hits["pm1_one_side_leftover_and_live"] += 1
            elif lives:
                hits["pm1_one_side_live_only"] += 1
            else:
                hits["pm1_one_side_zero_only"] += 1
        else:
            hits["pm1_both_or_multi"] += 1
        if not times:
            hits["code_never_in_callbacks"] += 1
            never_prefixes[meta["code"][:3]] += 1
            if close == 0:
                hits["never_and_close0"] += 1
            continue
        hits["code_seen_other_second"] += 1
        nearest = min(times, key=lambda t: abs(t - row["ts"]))
        delta = nearest - row["ts"]
        nearest_bucket[bucket_abs(delta)] += 1
        if abs(delta) <= 1:
            hits["nearest_1s_but_not_exact"] += 1
        if close == 0:
            hits["other_second_close0"] += 1
        if len(samples) < 10:
            samples.append(
                {
                    "code": meta["code"],
                    "internal_fmt": fmt_ts(row["ts"]),
                    "nearest_cb_fmt": fmt_ts(nearest),
                    "delta": delta,
                    "close": close,
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_unmatched_no_same_second.v2",
        "extract_dir": extract_dir,
        "counts": dict(hits),
        "no_same_second_hour": dict(sorted(hour.items())),
        "no_same_second_prefix_head": dict(prefixes.most_common(12)),
        "never_in_callbacks_prefix_head": dict(never_prefixes.most_common(8)),
        "nearest_other_second_abs_bucket": dict(nearest_bucket),
        "samples_code_seen_other_second": samples,
        "note": (
            "Same-second join is the product key. This report does not widen the "
            "window. pm1_one_side_zero_only would join leftover if ±1s were allowed. "
            "code_never_in_callbacks is coverage; code_seen_other_second is Wine "
            "poll vs 2704 business time."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
