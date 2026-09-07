#!/usr/bin/env python3
"""What is the nonzero 2704 close during leftover OEM seconds?

Compares incoming +0x10 against morning 0104 last and live 0x12b.
Does not edit official_5188.rs.

usage:
  classify-leftover-nz-close-source.py EXTRACT CALLBACK META_0903 MORNING_0104
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


def rel_bucket(left: float, right: float) -> str:
    denom = max(abs(left), abs(right))
    if denom == 0:
        return "both0"
    err = abs(left - right) / denom
    if err == 0:
        return "0"
    if err < 0.01:
        return "<1%"
    if err < 0.05:
        return "<5%"
    if err < 0.5:
        return "<50%"
    return ">=50%"


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: classify-leftover-nz-close-source.py EXTRACT CALLBACK META_0903 MORNING_0104",
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
    vs_0104 = Counter()
    vs_12b = Counter()
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
        if picked["price"] != 0:
            continue
        close = seed.i32(row["rec"], 0x10)
        if close == 0:
            hits["leftover_in0"] += 1
            continue
        hits["leftover_in_nz"] += 1
        if abs(close) > 1_000_000:
            hits["abs_gt_1e6"] += 1
        price = seed.rust_scaled(close, scale)
        live_12b = seed.rust_scaled(seed.i32(row["rec"], 0x12B), scale)
        morn = morning.get((row["market"], meta["code"]))
        last_i32 = morn["last_0104"] if morn else None
        last_f = (
            seed.rust_scaled(last_i32, morn["scale"])
            if morn and morn.get("scale", 0) > 0
            else None
        )
        if last_i32 is not None and close == last_i32:
            hits["i32_eq_0104"] += 1
        if last_f is not None and seed.same_f32(price, last_f):
            hits["f32_eq_0104"] += 1
        if seed.same_f32(price, live_12b):
            hits["f32_eq_12b"] += 1
        if last_f is not None:
            vs_0104[rel_bucket(price, last_f)] += 1
        vs_12b[rel_bucket(price, live_12b)] += 1
        if len(samples) < 8:
            samples.append(
                {
                    "code": meta["code"],
                    "incoming": close,
                    "proj": price,
                    "last_0104_i32": last_i32,
                    "last_0104_f": last_f,
                    "live_12b": live_12b,
                    "cb_last": picked["last_close"],
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_leftover_nz_close_source.v1",
        "extract_dir": extract_dir,
        "counts": dict(hits),
        "vs_morning_0104_rel": dict(vs_0104),
        "vs_live_0x12b_rel": dict(vs_12b),
        "samples": samples,
        "note": (
            "Leftover OEM + nonzero +0x10. If it equals 0104 last it is T-1 "
            "residue. If not, it is wrong-session or unpublished state and "
            "still must not open the first-print gate."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
