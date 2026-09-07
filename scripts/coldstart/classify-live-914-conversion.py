#!/usr/bin/env python3
"""Classify live 914 auction joins: conversion vs leftover vs layer-A garbage.

Does not touch owned decoder files. Reuses the same extract/callback/metadata
paths as seed-last-close-and-preopen.py.

usage:
  classify-live-914-conversion.py EXTRACT CALLBACK META_0903 NIGHT_0104 PROBE_JSON
"""
from __future__ import annotations

import json
import math
import os
import struct
import sys
from collections import Counter, defaultdict

# Reuse loaders from the sibling script without making it a package.
_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()


def rel_err(left: float, right: float) -> float | None:
    if right == 0 and left == 0:
        return 0.0
    denom = max(abs(right), abs(left))
    if denom == 0:
        return None
    return abs(left - right) / denom


def oem_price_relative(amount: int, volume: int, price_ticks: int, places: int) -> float:
    volume_f = seed.to_f32(volume)
    amount_f = seed.to_f32(amount)
    if volume_f == 0.0:
        return 0.0
    hand = seed.to_f32(100.0)
    point_scale = seed.to_f32(10.0 ** places)
    per_unit = amount_f / volume_f
    per_share = per_unit / hand
    scaled_price = per_share * point_scale
    price_offset = scaled_price - seed.to_f32(price_ticks)
    scaled_offset = price_offset * seed.to_f32(30.0)
    encoded = int(math.trunc(scaled_offset + 0.5))
    decoded_offset = seed.to_f32(encoded) / seed.to_f32(30.0)
    decoded_price = decoded_offset + seed.to_f32(price_ticks)
    decoded_per_share = decoded_price * volume_f
    decoded_per_hand = decoded_per_share * hand
    return seed.to_f32(decoded_per_hand / point_scale)


def bucket_rel(err: float | None) -> str:
    if err is None:
        return "undefined"
    if err == 0:
        return "0"
    if err < 1e-6:
        return "<1e-6"
    if err < 1e-4:
        return "<1e-4"
    if err < 1e-3:
        return "<1e-3"
    if err < 1e-2:
        return "<1e-2"
    if err < 0.05:
        return "<5%"
    if err < 0.2:
        return "<20%"
    return ">=20%"


def main() -> int:
    if len(sys.argv) != 6:
        print(
            "usage: classify-live-914-conversion.py EXTRACT CALLBACK META_0903 NIGHT_0104 PROBE",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, night_0104, probe_json = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    night_meta = seed.load_largest_0104(night_0104)
    probe = seed.load_probe_by_code(probe_json)
    rows, needed = seed.load_decoded(extract_dir)
    wanted = set()
    for market, index in needed:
        meta = metadata.get((market, index))
        if meta:
            wanted.add((market, meta["code"]))
    quotes, n_batches = seed.stream_quotes(callback_jsonl, wanted)
    open_cutoff = seed.parse_callback_ts("2026-09-04 09:25:00")

    classes = Counter()
    amount_rel = Counter()
    price_rel = Counter()
    volume_rel = Counter()
    oem7709_hits = 0
    raw_f32_hits = 0
    vol_f32_hits = 0
    leftover_yesterday = 0
    negative = 0
    samples = defaultdict(list)
    amount_mode_hits = Counter()

    states: dict[tuple[str, str], seed.Public] = {}
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None or meta["scale"] <= 0:
            continue
        key = (row["market"], meta["code"])
        if key not in states:
            night_row = probe.get(key)
            seed_last = int(night_row["close"]) if night_row else meta["last_0104"]
            states[key] = seed.Public(seed_last)
        incoming_amt = seed.i64(row["rec"], 0x1C)
        seed.merge(states[key], row)
        q = seed.pick_quote(quotes.get(key, []), row["ts"])
        if q is None or incoming_amt == 0:
            continue
        if q[4] == 0 and q[10] == 0:
            continue
        if open_cutoff is not None and q[3] is not None and q[3] < open_cutoff:
            continue

        st = states[key]
        scale = meta["scale"]
        proj_price = seed.rust_scaled(st.close, scale)
        proj_amt = seed.to_f32(st.amount)
        proj_vol = seed.to_f32(st.volume)
        oem_amt = oem_price_relative(st.amount, st.volume, st.close, meta["places"])
        cb_price, cb_last, cb_vol, cb_amt = q[4], q[5], q[9], q[10]
        night_close = probe[key]["close"] if key in probe else None
        night_close_f = seed.rust_scaled(int(night_close), scale) if night_close is not None else None

        price_hit = seed.same_f32(proj_price, cb_price)
        amt_hit = seed.same_f32(proj_amt, cb_amt)
        vol_hit = seed.same_f32(proj_vol, cb_vol)
        oem_hit = seed.same_f32(oem_amt, cb_amt)
        if amt_hit:
            raw_f32_hits += 1
        if oem_hit:
            oem7709_hits += 1
        if vol_hit:
            vol_f32_hits += 1

        amount_rel[bucket_rel(rel_err(proj_amt, cb_amt))] += 1
        price_rel[bucket_rel(rel_err(proj_price, cb_price))] += 1
        volume_rel[bucket_rel(rel_err(proj_vol, cb_vol))] += 1
        amount_mode_hits[str(meta.get("amount_mode"))] += 1

        cls = None
        if st.amount < 0 or incoming_amt < 0:
            cls = "layer_a_negative_amount"
            negative += 1
        elif night_close_f is not None and seed.same_f32(proj_price, night_close_f) and not price_hit and cb_price != 0:
            cls = "leftover_yesterday_close_vs_auction"
            leftover_yesterday += 1
        elif price_hit and amt_hit:
            cls = "price_and_raw_amount_hit"
        elif price_hit and oem_hit:
            cls = "price_hit_oem7709_amount_hit"
        elif price_hit and (rel_err(proj_amt, cb_amt) or 1) < 1e-4:
            cls = "price_hit_amount_rel_1e-4"
        elif price_hit and (rel_err(proj_amt, cb_amt) or 1) < 0.05:
            cls = "price_hit_amount_within_5pct"
        elif price_hit:
            cls = "price_hit_amount_far"
        elif st.close == 0 and cb_price != 0:
            cls = "internal_price_zero_callback_live"
        else:
            cls = "price_mismatch_both_nonzero"
        classes[cls] += 1
        if len(samples[cls]) < 6:
            samples[cls].append(
                {
                    "code": meta["code"],
                    "datetime": q[2],
                    "mask": row["mask"],
                    "internal_close": st.close,
                    "internal_amount": st.amount,
                    "internal_volume": st.volume,
                    "night_close": night_close,
                    "amount_mode": meta.get("amount_mode"),
                    "places": meta["places"],
                    "proj_price": proj_price,
                    "cb_price": cb_price,
                    "proj_amount": proj_amt,
                    "oem7709_amount": oem_amt,
                    "cb_amount": cb_amt,
                    "proj_volume": proj_vol,
                    "cb_volume": cb_vol,
                    "amount_rel": rel_err(proj_amt, cb_amt),
                    "price_rel": rel_err(proj_price, cb_price),
                }
            )

    n = sum(classes.values())
    report = {
        "n_live": n,
        "callback_batches": n_batches,
        "note": (
            "Live = incoming amount≠0, exact business-second join, callback has price or amount, "
            "datetime >= 09:25:00. 7709 PriceRelative is an ablation, not a 5188 proof."
        ),
        "classes": dict(classes),
        "raw_f32_amount_hits": raw_f32_hits,
        "oem7709_price_relative_amount_hits": oem7709_hits,
        "raw_f32_volume_hits": vol_f32_hits,
        "negative_amount": negative,
        "leftover_yesterday_close": leftover_yesterday,
        "amount_rel_buckets": dict(amount_rel),
        "price_rel_buckets": dict(price_rel),
        "volume_rel_buckets": dict(volume_rel),
        "amount_mode_on_live": dict(amount_mode_hits),
        "samples": samples,
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
