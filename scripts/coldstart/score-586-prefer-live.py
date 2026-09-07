#!/usr/bin/env python3
"""Score join policies on the 586 same-second conflicts.

Does not edit official_5188.rs or the parity example. Incoming 311B price is
the record close; last_close comparison uses morning 0104 when provided.

usage:
  score-586-prefer-live.py EXTRACT CALLBACK META_0903 [MORNING_0104]
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


def is_live(q: dict) -> bool:
    return q["price"] != 0.0


def pick_nearest(cands: list[dict], completed: int) -> dict:
    return min(cands, key=lambda q: (abs(q["ts_ms"] * 1000 - completed), q["ts_ms"], q["seq"]))


def pick_later(cands: list[dict]) -> dict:
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def pick_live(cands: list[dict]) -> dict:
    live = [q for q in cands if is_live(q)]
    return pick_later(live if live else cands)


def pair_shape(cands: list[dict]) -> str:
    zeros = sum(1 for q in cands if not is_live(q))
    lives = len(cands) - zeros
    return f"{zeros}zero+{lives}live"


def score_pick(proj_price: float | None, proj_last: float | None, picked: dict) -> dict:
    return {
        "live": is_live(picked),
        "price": False
        if proj_price is None
        else seed.same_f32(proj_price, picked["price"]),
        "last_close": False
        if proj_last is None
        else seed.same_f32(proj_last, picked["last_close"]),
        "price_zero_both": proj_price == 0.0 and picked["price"] == 0.0,
    }


def main() -> int:
    if len(sys.argv) not in (4, 5):
        print(
            "usage: score-586-prefer-live.py EXTRACT CALLBACK META_0903 [MORNING_0104]",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:4]
    morning = (
        seed.load_largest_0104(sys.argv[4]) if len(sys.argv) == 5 else {}
    )
    metadata = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)

    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    policies = ("nearest", "later", "live")
    hits = {name: Counter() for name in policies}
    shapes = Counter()
    disagree = Counter()
    samples = []
    ambiguous = 0
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        key = (row["market"], meta["code"], row["ts"])
        cands = cb_by.get(key, [])
        if len(cands) <= 1:
            continue
        ambiguous += 1
        shapes[pair_shape(cands)] += 1
        scale = float(meta.get("scale") or 0)
        close = seed.i32(row["rec"], 0x10)
        proj_price = seed.rust_scaled(close, scale) if scale > 0 else None
        morn = morning.get((row["market"], meta["code"]))
        proj_last = (
            seed.rust_scaled(morn["last_0104"], morn["scale"])
            if morn and morn.get("scale", 0) > 0
            else None
        )
        picked = {
            "nearest": pick_nearest(cands, row["completed"]),
            "later": pick_later(cands),
            "live": pick_live(cands),
        }
        for name, quote in picked.items():
            scored = score_pick(proj_price, proj_last, quote)
            hits[name]["n"] += 1
            for k, ok in scored.items():
                if ok:
                    hits[name][k] += 1
        if picked["nearest"]["seq"] != picked["live"]["seq"]:
            disagree["nearest_vs_live"] += 1
            if not is_live(picked["nearest"]) and is_live(picked["live"]):
                disagree["nearest_leftover_live_correct"] += 1
        if picked["later"]["seq"] != picked["live"]["seq"]:
            disagree["later_vs_live"] += 1
        if len(samples) < 6 and not is_live(picked["nearest"]) and is_live(picked["live"]):
            samples.append(
                {
                    "code": meta["code"],
                    "ts": row["ts"],
                    "nearest_seq": picked["nearest"]["seq"],
                    "nearest_price": picked["nearest"]["price"],
                    "live_seq": picked["live"]["seq"],
                    "live_price": picked["live"]["price"],
                    "incoming_close": proj_price,
                    "incoming_raw_close": close,
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_586_prefer_live.v1",
        "extract_dir": extract_dir,
        "ambiguous_decoded_records": ambiguous,
        "candidate_shapes": dict(shapes),
        "policy_hits": {
            name: {
                "n": hits[name]["n"],
                "picked_live": hits[name]["live"],
                "picked_leftover": hits[name]["n"] - hits[name]["live"],
                "price": hits[name]["price"],
                "last_close_vs_morning_0104": hits[name]["last_close"],
                "price_zero_both": hits[name]["price_zero_both"],
            }
            for name in policies
        },
        "disagreements": dict(disagree),
        "samples_nearest_leftover_vs_live": samples,
        "note": (
            "live policy keeps the non-zero same-second quote. last_close is "
            "identical in every pair, so last_close hits do not discriminate. "
            "Auction incoming close often already moved; do not use price hits "
            "here to retune the bitstream."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
