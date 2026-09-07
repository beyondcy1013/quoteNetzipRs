#!/usr/bin/env python3
"""Would treating +0x10 ≈ 0x12b as leftover false-suppress live OEM?

Does not edit official_5188.rs.

usage:
  score-close-eq-12b-as-leftover.py EXTRACT CALLBACK META_0903
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


def near(a: float, b: float, limit: float = 0.05) -> bool:
    denom = max(abs(a), abs(b))
    if denom == 0:
        return a == 0 and b == 0
    return abs(a - b) / denom < limit


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-close-eq-12b-as-leftover.py EXTRACT CALLBACK META_0903",
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
        picked = pick_later(cands)
        close = seed.i32(row["rec"], 0x10)
        price = seed.rust_scaled(close, scale)
        ref = seed.rust_scaled(seed.i32(row["rec"], 0x12B), scale)
        live = picked["price"] != 0
        suppress = close != 0 and near(price, ref)
        side = "live" if live else "leftover"
        hits[side] += 1
        if suppress:
            hits[f"{side}_suppress"] += 1
        else:
            hits[f"{side}_keep"] += 1
        if live and suppress and len(samples) < 6:
            samples.append(
                {
                    "code": meta["code"],
                    "proj": price,
                    "ref": ref,
                    "oem": picked["price"],
                }
            )

    leftover_n = hits["leftover"] or 1
    live_n = hits["live"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_close_eq_12b_as_leftover.v1",
        "extract_dir": extract_dir,
        "leftover": {
            "n": hits["leftover"],
            "would_suppress": hits["leftover_suppress"],
            "would_keep": hits["leftover_keep"],
            "suppress_rate": round(hits["leftover_suppress"] / leftover_n, 6),
        },
        "live": {
            "n": hits["live"],
            "false_suppress": hits["live_suppress"],
            "would_keep": hits["live_keep"],
            "false_suppress_rate": round(hits["live_suppress"] / live_n, 6),
        },
        "samples_live_false_suppress": samples,
        "note": (
            "Heuristic: nonzero +0x10 within 5% of 0x12b => leftover. "
            "False suppress on live OEM means the rule is unsafe on this "
            "wine-tail and must not ship."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
