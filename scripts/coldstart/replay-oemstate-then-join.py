#!/usr/bin/env python3
"""Replay OemState-style merge, then score exact-second joins.

Compares per-record projection (section 30) against capture-order sparse merge
plus morning 0104 previous_close. Does not edit official_5188.rs.

usage:
  replay-oemstate-then-join.py EXTRACT CALLBACK META_0903 MORNING_0104
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


def project_close_vol_amt(close: int, vol: int, amt: int, scale: float, star: bool):
    price = seed.rust_scaled(close, scale)
    volume = star_lots(vol) if star else seed.to_f32(vol)
    amount = seed.to_f32(amt)
    return price, volume, amount


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: replay-oemstate-then-join.py EXTRACT CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    morning = seed.load_largest_0104(morning_dir)
    rows, _ = seed.load_decoded(extract_dir)
    rows = sorted(rows, key=lambda r: r["order"])

    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    states: dict[tuple[str, str], seed.Public] = {}
    hits = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        scale = float(meta.get("scale") or 0)
        if scale <= 0:
            continue
        code_key = (row["market"], meta["code"])
        morn = morning.get(code_key)
        last_i32 = morn["last_0104"] if morn else 0
        state = states.get(code_key)
        if state is None:
            state = seed.Public(last_i32)
            states[code_key] = state
            hits["states"] += 1
        incoming_close = seed.i32(row["rec"], 0x10)
        had_close = state.close
        seed.merge(state, row)
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        picked = pick_later(cands)
        live = picked["price"] != 0
        prefix = "live" if live else "leftover"
        hits[prefix] += 1
        star = meta["code"].startswith(("688", "689"))
        in_price, in_vol, in_amt = project_close_vol_amt(
            incoming_close,
            seed.i64(row["rec"], 0x14),
            seed.i64(row["rec"], 0x1C),
            scale,
            star,
        )
        mg_price, mg_vol, mg_amt = project_close_vol_amt(
            state.close, state.volume, state.amount, scale, star
        )
        last = seed.rust_scaled(state.last_close, scale) if state.last_close else 0.0
        if seed.same_f32(in_price, picked["price"]):
            hits[f"{prefix}_in_price"] += 1
        if seed.same_f32(mg_price, picked["price"]):
            hits[f"{prefix}_mg_price"] += 1
        if seed.same_f32(last, picked["last_close"]):
            hits[f"{prefix}_last_close"] += 1
        if seed.same_f32(in_vol, picked["volume"]):
            hits[f"{prefix}_in_volume"] += 1
        if seed.same_f32(mg_vol, picked["volume"]):
            hits[f"{prefix}_mg_volume"] += 1
        if rel_ok(in_amt, picked["amount"], 1e-3):
            hits[f"{prefix}_in_amount"] += 1
        if rel_ok(mg_amt, picked["amount"], 1e-3):
            hits[f"{prefix}_mg_amount"] += 1
        if live and incoming_close == 0:
            hits["live_in_close0"] += 1
            if state.close != 0:
                hits["live_in_close0_merge_filled"] += 1
                if seed.same_f32(mg_price, picked["price"]):
                    hits["live_in_close0_merge_price_hit"] += 1
            if had_close == 0 and state.close == 0:
                hits["live_in_close0_no_history"] += 1
        if (
            live
            and incoming_close == 0
            and state.close != 0
            and not seed.same_f32(mg_price, picked["price"])
            and len(samples) < 6
        ):
            samples.append(
                {
                    "code": meta["code"],
                    "incoming_close": incoming_close,
                    "merged_close": state.close,
                    "mg_price": mg_price,
                    "cb_price": picked["price"],
                }
            )

    live_n = hits["live"] or 1
    leftover_n = hits["leftover"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_oemstate_merge_then_join.v1",
        "extract_dir": extract_dir,
        "joined": hits["joined"],
        "states": hits["states"],
        "live": {
            "n": hits["live"],
            "last_close": hits["live_last_close"],
            "incoming_price": hits["live_in_price"],
            "merged_price": hits["live_mg_price"],
            "incoming_volume": hits["live_in_volume"],
            "merged_volume": hits["live_mg_volume"],
            "incoming_amount": hits["live_in_amount"],
            "merged_amount": hits["live_mg_amount"],
            "incoming_price_rate": round(hits["live_in_price"] / live_n, 6),
            "merged_price_rate": round(hits["live_mg_price"] / live_n, 6),
            "in_close0": hits["live_in_close0"],
            "in_close0_merge_filled": hits["live_in_close0_merge_filled"],
            "in_close0_merge_price_hit": hits["live_in_close0_merge_price_hit"],
            "in_close0_no_history": hits["live_in_close0_no_history"],
        },
        "leftover": {
            "n": hits["leftover"],
            "last_close": hits["leftover_last_close"],
            "incoming_price": hits["leftover_in_price"],
            "merged_price": hits["leftover_mg_price"],
        },
        "samples_filled_but_still_miss": samples,
        "note": (
            "Capture-order sparse merge mirrors Official5188OemState. Auction "
            "live misses after merge still mean this wine-tail lacks the same-"
            "session dump, not that the night recipe is wrong."
        ),
        "leftover_n_check": leftover_n,
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
