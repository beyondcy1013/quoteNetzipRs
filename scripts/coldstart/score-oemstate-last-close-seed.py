#!/usr/bin/env python3
"""Score auction last_close if OemState is seeded from morning 0104 vs new(0).

Does not edit official_5188.rs. Join uses max callback sequence in the same
business second (section 24). last_close is the injected previous_close, not
live 0x12b.

usage:
  score-oemstate-last-close-seed.py EXTRACT CALLBACK META_0903 MORNING_0104
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


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-oemstate-last-close-seed.py EXTRACT CALLBACK META_0903 MORNING_0104",
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
    samples_zero = []
    samples_12b = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            hits["no_meta"] += 1
            continue
        key3 = (row["market"], meta["code"], row["ts"])
        cands = cb_by.get(key3, [])
        if not cands:
            hits["no_callback_same_second"] += 1
            continue
        hits["joined"] += 1
        picked = pick_later(cands)
        scale = float(meta.get("scale") or 0)
        if scale <= 0:
            hits["no_scale"] += 1
            continue
        live_12b = seed.rust_scaled(seed.i32(row["rec"], 0x12B), scale)
        shadow_zero = 0.0
        morn = morning.get((row["market"], meta["code"]))
        seeded = (
            seed.rust_scaled(morn["last_0104"], morn["scale"])
            if morn and morn.get("scale", 0) > 0
            else None
        )
        cb_last = picked["last_close"]
        if seed.same_f32(shadow_zero, cb_last):
            hits["new0_hit"] += 1
        else:
            hits["new0_miss"] += 1
        if seed.same_f32(live_12b, cb_last):
            hits["live_12b_hit"] += 1
        if seeded is None:
            hits["no_morning_0104"] += 1
        elif seed.same_f32(seeded, cb_last):
            hits["morning_0104_hit"] += 1
        else:
            hits["morning_0104_miss"] += 1
            if len(samples_12b) < 6:
                samples_12b.append(
                    {
                        "code": meta["code"],
                        "seeded": seeded,
                        "cb": cb_last,
                        "live_12b": live_12b,
                    }
                )
        if cb_last != 0.0 and len(samples_zero) < 4:
            samples_zero.append(
                {
                    "code": meta["code"],
                    "shadow_new0": shadow_zero,
                    "cb_last": cb_last,
                    "morning": seeded,
                }
            )

    n = hits["joined"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_oemstate_last_close_seed.v1",
        "extract_dir": extract_dir,
        "joined_same_second_later_seq": hits["joined"],
        "no_callback_same_second": hits["no_callback_same_second"],
        "shadow_new0": {
            "hits": hits["new0_hit"],
            "misses": hits["new0_miss"],
            "rate": round(hits["new0_hit"] / n, 6),
        },
        "live_0x12b": {
            "hits": hits["live_12b_hit"],
            "rate": round(hits["live_12b_hit"] / n, 6),
        },
        "morning_0104_seed": {
            "hits": hits["morning_0104_hit"],
            "misses": hits["morning_0104_miss"],
            "missing_table": hits["no_morning_0104"],
            "rate_on_present": round(
                hits["morning_0104_hit"]
                / max(hits["morning_0104_hit"] + hits["morning_0104_miss"], 1),
                6,
            ),
        },
        "samples_new0_vs_nonzero_callback": samples_zero,
        "samples_0104_miss": samples_12b,
        "note": (
            "Current shadow constructs Official5188OemState::new(0), so projected "
            "last_close is 0 until day-cut seed is wired. Morning 0104 is the seed."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
