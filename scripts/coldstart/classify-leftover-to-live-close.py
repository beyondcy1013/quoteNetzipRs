#!/usr/bin/env python3
"""At leftover→live, does 2704 +0x10 leave overnight 0x12b?

Does not edit official_5188.rs.

usage:
  classify-leftover-to-live-close.py EXTRACT CALLBACK META_0903
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
            "usage: classify-leftover-to-live-close.py EXTRACT CALLBACK META_0903",
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

    by_code: dict[tuple[str, str], dict] = {}
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
        slot = {
            "ts": row["ts"],
            "price": price,
            "ref": ref,
            "cb": picked["price"],
            "close_i32": close,
        }
        key = (row["market"], meta["code"])
        rec = by_code.setdefault(key, {"leftover": None, "live": None, "code": meta["code"]})
        if live:
            hits["live_joins"] += 1
            if rec["live"] is None or slot["ts"] < rec["live"]["ts"]:
                rec["live"] = slot
            if seed.same_f32(price, picked["price"]):
                hits["live_eq_oem"] += 1
            if near(price, ref):
                hits["live_near_12b"] += 1
            if near(price, picked["price"]):
                hits["live_near_oem"] += 1
        else:
            hits["leftover_joins"] += 1
            if rec["leftover"] is None or slot["ts"] > rec["leftover"]["ts"]:
                rec["leftover"] = slot
            if near(price, ref) and close != 0:
                hits["leftover_nz_near_12b"] += 1

    both = 0
    for rec in by_code.values():
        if rec["leftover"] is None or rec["live"] is None:
            continue
        both += 1
        lo, lv = rec["leftover"], rec["live"]
        lo_res = lo["close_i32"] != 0 and near(lo["price"], lo["ref"])
        lv_oem = near(lv["price"], lv["cb"])
        lv_res = lv["close_i32"] != 0 and near(lv["price"], lv["ref"])
        if lo_res and lv_oem and not lv_res:
            hits["transition_residue_to_oem"] += 1
        elif lo_res and lv_res:
            hits["stayed_on_12b_after_oem_live"] += 1
        elif lv_oem:
            hits["live_matches_oem_other"] += 1
        else:
            hits["live_matches_neither"] += 1
        if both <= 6:
            samples.append(
                {
                    "code": rec["code"],
                    "leftover_proj": lo["price"],
                    "leftover_12b": lo["ref"],
                    "live_proj": lv["price"],
                    "live_12b": lv["ref"],
                    "live_oem": lv["cb"],
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_leftover_to_live_close.v1",
        "extract_dir": extract_dir,
        "codes_with_both": both,
        "counts": dict(hits),
        "samples_both": samples,
        "note": (
            "If +0x10 still tracks 0x12b after OEM goes live, this extract did "
            "not observe the first-print slot update. Same-session dump is still "
            "required for conversion; the gate remains OEM live."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
