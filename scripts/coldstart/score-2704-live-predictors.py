#!/usr/bin/env python3
"""Can any decoded 2704 field predict OEM leftover vs live?

Exact-second + max-sequence join. Does not edit official_5188.rs.

usage:
  score-2704-live-predictors.py EXTRACT CALLBACK META_0903
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
GATE = int(datetime(2026, 9, 4, 9, 25, 0, tzinfo=TZ8).timestamp())


def pick_later(cands: list[dict]) -> dict:
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def near(a: float, b: float, limit: float = 0.05) -> bool:
    denom = max(abs(a), abs(b))
    if denom == 0:
        return a == 0 and b == 0
    return abs(a - b) / denom < limit


def score(pred_live: bool, is_live: bool, bag: Counter, prefix: str) -> None:
    if pred_live and is_live:
        bag[f"{prefix}_tp"] += 1
    elif pred_live and not is_live:
        bag[f"{prefix}_fp"] += 1
    elif (not pred_live) and is_live:
        bag[f"{prefix}_fn"] += 1
    else:
        bag[f"{prefix}_tn"] += 1


def pack(bag: Counter, prefix: str, n_live: int, n_left: int) -> dict:
    tp = bag[f"{prefix}_tp"]
    fp = bag[f"{prefix}_fp"]
    fn = bag[f"{prefix}_fn"]
    tn = bag[f"{prefix}_tn"]
    prec = tp / (tp + fp) if (tp + fp) else 0.0
    rec = tp / n_live if n_live else 0.0
    spec = tn / n_left if n_left else 0.0
    return {
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "tn": tn,
        "precision": round(prec, 6),
        "recall_live": round(rec, 6),
        "specificity_leftover": round(spec, 6),
    }


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-2704-live-predictors.py EXTRACT CALLBACK META_0903",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)
    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    bag = Counter()
    mask_live = Counter()
    mask_left = Counter()
    n_live = 0
    n_left = 0
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
        picked = pick_later(cands)
        rec = row["rec"]
        close = seed.i32(rec, 0x10)
        open_ = seed.i32(rec, 0x04)
        high = seed.i32(rec, 0x08)
        low = seed.i32(rec, 0x0C)
        vol = seed.i64(rec, 0x14)
        amt = seed.i64(rec, 0x1C)
        ref = seed.i32(rec, 0x12B)
        bid1 = seed.i32(rec, 0x58 + 4 * 4)
        price = seed.rust_scaled(close, scale)
        ref_px = seed.rust_scaled(ref, scale)
        mask = int(row["mask"])
        mask_class = mask & 0x38
        live = picked["price"] != 0
        if live:
            n_live += 1
            mask_live[str(mask_class)] += 1
        else:
            n_left += 1
            mask_left[str(mask_class)] += 1

        rules = {
            "close_nz": close != 0,
            "volume_nz": vol != 0,
            "amount_nz": amt != 0,
            "open_nz": open_ != 0,
            "ohlc_any_nz": any(v != 0 for v in (open_, high, low, close)),
            "bid1_nz": bid1 != 0,
            "vol_or_amt_nz": vol != 0 or amt != 0,
            "close_ne_12b": close != ref,
            "close_not_near_12b": close != 0 and not near(price, ref_px),
            "ts_ge_0925": row["ts"] >= GATE,
            "mask_not_ts_only": mask_class != 0x18,
            "mask_has_values": mask_class not in (0x00, 0x18),
        }
        for name, pred in rules.items():
            score(pred, live, bag, name)

        if live and close == 0 and vol == 0 and amt == 0 and len(samples) < 6:
            samples.append(
                {
                    "code": meta["code"],
                    "oem": picked["price"],
                    "mask": mask,
                    "mask_class": mask_class,
                    "ts": row["ts"],
                }
            )

    rules_out = {
        name: pack(bag, name, n_live, n_left)
        for name in (
            "close_nz",
            "volume_nz",
            "amount_nz",
            "open_nz",
            "ohlc_any_nz",
            "bid1_nz",
            "vol_or_amt_nz",
            "close_ne_12b",
            "close_not_near_12b",
            "ts_ge_0925",
            "mask_not_ts_only",
            "mask_has_values",
        )
    }
    report = {
        "schema": "quoteNetzipRs.official_5188_2704_live_predictors.v1",
        "extract_dir": extract_dir,
        "n_leftover": n_left,
        "n_live": n_live,
        "predict_live": rules_out,
        "mask_class_leftover": dict(mask_left),
        "mask_class_live": dict(mask_live),
        "samples_live_all_trade_zero": samples,
        "note": (
            "Each rule predicts OEM live (callback price != 0). "
            "High recall with low precision means leftover is also nonzero. "
            "Low recall means live OEM has a zero 2704 field. "
            "No rule is a product first-print gate on this wine-tail."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
