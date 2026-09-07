#!/usr/bin/env python3
"""Score recipe-v2 fields on exact-second + max-sequence auction joins.

Does not edit official_5188.rs. Auction misses do not disprove the night dump
recipe; this only asks which fields still miss after join is correct.

usage:
  score-joined-auction-fields.py EXTRACT CALLBACK META_0903 MORNING_0104
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
ident = SourceFileLoader(
    "disambiguate_586_batch_identity",
    os.path.join(_DIR, "disambiguate-586-batch-identity.py"),
).load_module()


def pick_later(cands: list[dict]) -> dict:
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def star_lots(value: int) -> float:
    if value <= 0:
        return seed.to_f32(0)
    lots = (value + 50) // 100
    if lots == 0:
        lots = 1
    return seed.to_f32(lots)


def rel_ok(left: float, right: float, limit: float) -> bool:
    denom = max(abs(left), abs(right))
    if denom == 0:
        return left == 0 and right == 0
    return abs(left - right) / denom < limit


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-joined-auction-fields.py EXTRACT CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    morning = seed.load_largest_0104(morning_dir)
    rows, _ = seed.load_decoded(extract_dir)

    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    hits = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        scale = float(meta.get("scale") or 0)
        if scale <= 0:
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        picked = pick_later(cands)
        rec = row["rec"]
        close = seed.i32(rec, 0x10)
        vol = seed.i64(rec, 0x14)
        amt = seed.i64(rec, 0x1C)
        star = meta["code"].startswith(("688", "689"))
        price = seed.rust_scaled(close, scale)
        volume = star_lots(vol) if star else seed.to_f32(vol)
        amount = seed.to_f32(amt)
        morn = morning.get((row["market"], meta["code"]))
        last = (
            seed.rust_scaled(morn["last_0104"], morn["scale"])
            if morn and morn.get("scale", 0) > 0
            else None
        )
        live = picked["price"] != 0
        hits["live" if live else "leftover"] += 1
        prefix = "live" if live else "leftover"
        if seed.same_f32(price, picked["price"]):
            hits[f"{prefix}_price"] += 1
        elif live and close == 0:
            hits["live_internal_close0"] += 1
        elif live:
            hits["live_price_miss_nonzero"] += 1
            if abs(close) > 1_000_000:
                hits["live_internal_abs_gt_1e6"] += 1
        if last is not None and seed.same_f32(last, picked["last_close"]):
            hits[f"{prefix}_last_close"] += 1
        if seed.same_f32(volume, picked["volume"]):
            hits[f"{prefix}_volume"] += 1
        if rel_ok(amount, picked["amount"], 1e-3):
            hits[f"{prefix}_amount"] += 1
        if live and not seed.same_f32(price, picked["price"]) and len(samples) < 8:
            samples.append(
                {
                    "code": meta["code"],
                    "internal_close": close,
                    "proj_price": price,
                    "cb_price": picked["price"],
                    "proj_vol": volume,
                    "cb_vol": picked["volume"],
                    "proj_amt": amount,
                    "cb_amt": picked["amount"],
                }
            )

    n = hits["joined"] or 1
    live_n = hits["live"] or 1
    leftover_n = hits["leftover"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_joined_auction_fields.v2",
        "extract_dir": extract_dir,
        "joined_same_second_later_seq": hits["joined"],
        "live": {
            "n": hits["live"],
            "price": hits["live_price"],
            "last_close": hits["live_last_close"],
            "volume": hits["live_volume"],
            "amount_rel_1e-3": hits["live_amount"],
            "price_rate": round(hits["live_price"] / live_n, 6),
            "internal_close0": hits["live_internal_close0"],
            "price_miss_nonzero": hits["live_price_miss_nonzero"],
            "internal_abs_gt_1e6": hits["live_internal_abs_gt_1e6"],
        },
        "leftover": {
            "n": hits["leftover"],
            "price": hits["leftover_price"],
            "last_close": hits["leftover_last_close"],
            "volume": hits["leftover_volume"],
            "amount_rel_1e-3": hits["leftover_amount"],
        },
        "samples_live_price_miss": samples,
        "note": (
            "Join is exact business second plus max sequence. last_close uses "
            "morning 0104, not live 0x12b. Auction live misses are not a night "
            "recipe falsifier; they measure remaining mid-session state vs OEM."
        ),
        "joined_n_check": n,
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
