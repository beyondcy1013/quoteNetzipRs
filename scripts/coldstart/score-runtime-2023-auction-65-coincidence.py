#!/usr/bin/env python3
"""Classify the 65 persist-clean close==OEM-live hits.

Night dump (section 59) is 5,166/5,166. Auction persist-clean is 65/5,421.
This checks whether those 65 are leftover/yesterday close coinciding with the
auction print, or a 311B that actually moved in the wine-tail.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-auction-65-coincidence.py EXTRACT CALLBACK META_0903 MORNING_0104
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
two = SourceFileLoader(
    "score_runtime_2010_two_step_seed",
    os.path.join(_DIR, "score-runtime-2010-two-step-seed.py"),
).load_module()
combo = SourceFileLoader(
    "score_runtime_2023_two_map_plus_rollback",
    os.path.join(_DIR, "score-runtime-2023-two-map-plus-rollback.py"),
).load_module()
snap = SourceFileLoader(
    "score_runtime_2023_snapshot_would_publish",
    os.path.join(_DIR, "score-runtime-2023-snapshot-would-publish.py"),
).load_module()


def close_i(rec: bytes) -> int:
    return seed.i32(rec, 0x10)


def history(rows, keys):
    hist = defaultdict(list)
    for row in rows:
        key = (row["market"], row["index"])
        if key not in keys:
            continue
        hist[key].append(close_i(row["rec"]))
    return hist


def bucket_of(close_f, last_f, first_i, scale):
    first_f = seed.rust_scaled(first_i, scale)
    eq_last = seed.same_f32(close_f, last_f)
    first_eq_persist = seed.same_f32(first_f, close_f)
    if first_i == 0:
        return "first_print_in_extract"
    if first_eq_persist and eq_last:
        return "first_already_0104_and_oem"
    if first_eq_persist:
        return "first_already_live_not_0104"
    return "first_nonzero_then_changed_to_oem"


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-auction-65-coincidence.py EXTRACT "
            "CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    identity = seed.load_index_metadata(meta_0903)
    symbol_codes = {key: row["code"] for key, row in identity.items()}
    _, morning_seeds, _ = two.simulate_runtime_maps(two.load_all_tables(morning_dir))
    rows, _ = seed.load_decoded(extract_dir)
    oem = snap.last_oem(callback_jsonl)
    states, mapped = combo.persist_states(rows, symbol_codes, morning_seeds, False)

    hits = []
    market = Counter()
    for key, st in states.items():
        amount = seed.i64(bytes(st), 0x1C)
        if amount < 0:
            continue
        meta = mapped[key]
        q = oem.get((key[0], meta["code"]))
        if q is None or q["price"] == 0:
            continue
        scale = meta.get("scale") or 1.0
        close_f = seed.rust_scaled(close_i(bytes(st)), scale)
        if not seed.same_f32(close_f, q["price"]):
            continue
        last_f = seed.rust_scaled(meta.get("last") or 0, scale)
        hits.append(
            {
                "key": key,
                "code": meta["code"],
                "scale": scale,
                "close_f": close_f,
                "last_f": last_f,
                "oem_price": q["price"],
                "amount": amount,
            }
        )
        market[key[0]] += 1

    keys = {h["key"] for h in hits}
    hist = history(rows, keys)
    buckets = Counter()
    samples = defaultdict(list)
    for h in hits:
        seq = hist.get(h["key"]) or [0]
        first_i, last_i = seq[0], seq[-1]
        n_distinct = len(set(seq))
        later_zero = sum(1 for v in seq[1:] if v == 0)
        b = bucket_of(h["close_f"], h["last_f"], first_i, h["scale"])
        buckets[b] += 1
        if len(samples[b]) < 4:
            samples[b].append(
                {
                    "market": h["key"][0],
                    "index": h["key"][1],
                    "code": h["code"],
                    "oem_price": h["oem_price"],
                    "state_close": h["close_f"],
                    "last_close": h["last_f"],
                    "n_records": len(seq),
                    "n_distinct_close": n_distinct,
                    "later_incoming_close_zero": later_zero,
                    "first_close": seed.rust_scaled(first_i, h["scale"]),
                    "last_incoming_close": seed.rust_scaled(last_i, h["scale"]),
                    "amount": h["amount"],
                }
            )

    already = (
        buckets["first_already_0104_and_oem"]
        + buckets["first_already_live_not_0104"]
    )
    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_auction_65_coincidence.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "close_eq_oem_live": len(hits),
        "market_split": dict(market),
        "buckets": dict(buckets),
        "summary": {
            "first_wine_tail_already_equals_oem": already,
            "first_print_appeared_in_this_extract": buckets["first_print_in_extract"],
            "first_nonzero_then_changed_to_oem": buckets[
                "first_nonzero_then_changed_to_oem"
            ],
        },
        "samples": {k: samples[k] for k in samples},
        "product_wiring": [
            "Later incoming close=0 does not mean the price moved; sparse merge keeps the first nonzero close.",
            "Most of the 65 already equal OEM on the first wine-tail frame; only first-print-from-zero is a real close update in this extract.",
            "Do not publish persist close as auction live. Night dump remains the layer-C positive control.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
