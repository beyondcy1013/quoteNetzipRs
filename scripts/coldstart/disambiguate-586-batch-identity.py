#!/usr/bin/env python3
"""Classify the 586 same-second multi-candidate joins without touching owned files.

v2 nearest-among-same-second still matches them (9184). v3-state-safe leaves all
586 unmatched because distinct WineQuote fingerprints never collapse. This
checks whether callback batch order can zip them to decoded records, and
whether the night same-session fixture even has this conflict.

usage:
  disambiguate-586-batch-identity.py EXTRACT CALLBACK META_0903 [NIGHT_CALLBACK]
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()


def fp_quote(q: dict) -> tuple:
    return (
        seed.f32_bits(float(q.get("price") or 0)),
        seed.f32_bits(float(q.get("last_close") or 0)),
        seed.f32_bits(float(q.get("volume") or 0)),
        seed.f32_bits(float(q.get("amount") or 0)),
        seed.f32_bits(float((q.get("bid_prices") or [0])[0] or 0)),
        seed.f32_bits(float((q.get("ask_prices") or [0])[0] or 0)),
        seed.f32_bits(float((q.get("open") or 0))),
        seed.f32_bits(float((q.get("high") or 0))),
        seed.f32_bits(float((q.get("low") or 0))),
    )


def stream_callback_quotes(path: str):
    with open(path, "r", encoding="utf-8") as fh:
        seen = set()
        for line in fh:
            ev = json.loads(line)
            seq = ev.get("sequence")
            if seq in seen:
                continue
            seen.add(seq)
            batch = ev.get("quote_batch") or {}
            if batch.get("schema") != "quoteNetzipWine.quote_batch.v1":
                continue
            ts_ms = int(ev.get("timestamp_ms") or 0)
            for q in batch.get("quotes") or []:
                cb_ts = seed.parse_callback_ts(q.get("datetime") or "")
                yield {
                    "seq": int(seq or 0),
                    "ts_ms": ts_ms,
                    "market": q["market"],
                    "code": q["code"],
                    "cb_ts": cb_ts,
                    "datetime": q.get("datetime") or "",
                    "price": float(q.get("price") or 0),
                    "last_close": float(q.get("last_close") or 0),
                    "volume": float(q.get("volume") or 0),
                    "amount": float(q.get("amount") or 0),
                    "fp": fp_quote(q),
                }


def callback_multiplicity(path: str) -> dict:
    by = defaultdict(list)
    n_quotes = 0
    n_batches = 0
    seen = set()
    for q in stream_callback_quotes(path):
        n_quotes += 1
        if q["seq"] not in seen:
            seen.add(q["seq"])
            n_batches += 1
        if q["cb_ts"] is None:
            continue
        by[(q["market"], q["code"], q["cb_ts"])].append(q)
    multi = {k: v for k, v in by.items() if len(v) > 1}
    fps = Counter()
    seq_spans = Counter()
    sizes = Counter()
    last_close_same = 0
    last_close_diff = 0
    for quotes in multi.values():
        sizes[len(quotes)] += 1
        nfp = len({q["fp"] for q in quotes})
        fps[nfp] += 1
        seqs = {q["seq"] for q in quotes}
        seq_spans[max(seqs) - min(seqs) if seqs else 0] += 1
        lasts = {seed.f32_bits(q["last_close"]) for q in quotes}
        if len(lasts) == 1:
            last_close_same += 1
        else:
            last_close_diff += 1
    return {
        "batches": n_batches,
        "quotes": n_quotes,
        "groups": len(by),
        "multi_groups": len(multi),
        "multi_quotes": sum(len(v) for v in multi.values()),
        "size_histogram": dict(sizes),
        "distinct_fp_histogram": dict(fps),
        "last_close_same_in_group": last_close_same,
        "last_close_diff_in_group": last_close_diff,
        "seq_span_histogram_head": dict(sorted(seq_spans.items())[:12]),
    }


def main() -> int:
    if len(sys.argv) not in (4, 5):
        print(
            "usage: disambiguate-586-batch-identity.py EXTRACT CALLBACK META_0903 [NIGHT_CALLBACK]",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:4]
    night_callback = sys.argv[4] if len(sys.argv) == 5 else None
    metadata = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)

    cb_by = defaultdict(list)
    for q in stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    decoded_by = defaultdict(list)
    ambiguous_records = 0
    zip_equal = 0
    zip_decoded_lt = 0
    zip_decoded_gt = 0
    nearest_vs_zip_same = 0
    nearest_vs_zip_diff = 0
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        key = (row["market"], meta["code"], row["ts"])
        decoded_by[key].append(row)
        cands = cb_by.get(key, [])
        if len(cands) > 1:
            ambiguous_records += 1

    zip_groups = 0
    for key, recs in decoded_by.items():
        cands = cb_by.get(key, [])
        if len(cands) <= 1:
            continue
        zip_groups += 1
        recs_sorted = sorted(recs, key=lambda r: (r["completed"], r["order"]))
        cands_sorted = sorted(cands, key=lambda q: (q["ts_ms"], q["seq"]))
        n_r, n_c = len(recs_sorted), len(cands_sorted)
        if n_r == n_c:
            zip_equal += 1
        elif n_r < n_c:
            zip_decoded_lt += 1
        else:
            zip_decoded_gt += 1
        if recs_sorted and cands_sorted:
            nearest = min(
                cands_sorted,
                key=lambda q: (
                    abs(q["ts_ms"] * 1000 - recs_sorted[0]["completed"]),
                    q["ts_ms"],
                    q["seq"],
                ),
            )
            zipped = cands_sorted[0]
            if nearest["seq"] == zipped["seq"] and nearest["fp"] == zipped["fp"]:
                nearest_vs_zip_same += 1
            else:
                nearest_vs_zip_diff += 1
        if len(samples) < 8:
            samples.append(
                {
                    "market": key[0],
                    "code": key[1],
                    "ts": key[2],
                    "decoded": n_r,
                    "callbacks": n_c,
                    "distinct_fp": len({q["fp"] for q in cands}),
                    "batch_seqs": sorted({q["seq"] for q in cands})[:12],
                    "last_closes": sorted({round(q["last_close"], 4) for q in cands}),
                    "prices": sorted({round(q["price"], 4) for q in cands})[:6],
                }
            )

    auction_cb = callback_multiplicity(callback_jsonl)
    night_cb = callback_multiplicity(night_callback) if night_callback else None

    report = {
        "schema": "quoteNetzipRs.official_5188_586_batch_identity.v1",
        "extract_dir": extract_dir,
        "ambiguous_decoded_records": ambiguous_records,
        "multi_callback_groups_touched_by_decoded": zip_groups,
        "zip_decoded_vs_callbacks": {
            "equal_count": zip_equal,
            "decoded_fewer": zip_decoded_lt,
            "decoded_more": zip_decoded_gt,
        },
        "first_record_nearest_vs_batch_order": {
            "same": nearest_vs_zip_same,
            "different": nearest_vs_zip_diff,
        },
        "auction_callback_multiplicity": auction_cb,
        "night_callback_multiplicity": night_cb,
        "samples": samples,
        "shadow_previous_close": {
            "runtime": "Official5188OemState::new(0) in official_5188_runtime.rs",
            "api": "OemState::new(previous_close) then project() writes 0x12b",
            "day_cut_source": "same-morning 0104 opaque_tail i32@+11, not live +0x10",
            "owned_file_untouched": True,
        },
        "note": (
            "586 v3 conflicts are distinct WineQuote states in the same business "
            "second. Zip is a join rule only when decoded-count == callback-count."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
