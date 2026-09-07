#!/usr/bin/env python3
"""Score last_close seed key: market+code vs market+symbol_index.

Runtime caches login 0104 by (market, symbol_index). The offline seed map is
market+code. This measures cross-session index drift without editing
official_5188.rs.

usage:
  score-last-close-key-index-vs-code.py EXTRACT CALLBACK META_0903 MORNING_0104
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


def index_map(by_code: dict) -> dict[tuple[str, int], dict]:
    out = {}
    for (market, code), row in by_code.items():
        out[(market, row["index"])] = {**row, "code": code, "market": market}
    return out


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-last-close-key-index-vs-code.py EXTRACT CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    morning_by_code = seed.load_largest_0104(morning_dir)
    morning_by_index = index_map(morning_by_code)
    meta_0903_by_code = seed.load_largest_0104(meta_0903)
    meta_0903_by_index = seed.load_index_metadata(meta_0903)

    stable = moved = missing_morn = 0
    for key, row in meta_0903_by_code.items():
        morn = morning_by_code.get(key)
        if morn is None:
            missing_morn += 1
            continue
        if row["index"] == morn["index"]:
            stable += 1
        else:
            moved += 1

    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    rows, _ = seed.load_decoded(extract_dir)
    hits = Counter()
    samples_index_miss = []
    for row in rows:
        meta = meta_0903_by_index.get((row["market"], row["index"]))
        if meta is None:
            hits["no_0903_meta"] += 1
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        picked = pick_later(cands)
        cb_last = picked["last_close"]
        by_code = morning_by_code.get((row["market"], meta["code"]))
        by_idx = morning_by_index.get((row["market"], row["index"]))
        if by_code and by_code.get("scale", 0) > 0:
            proj = seed.rust_scaled(by_code["last_0104"], by_code["scale"])
            if seed.same_f32(proj, cb_last):
                hits["by_code_hit"] += 1
            else:
                hits["by_code_miss"] += 1
        else:
            hits["by_code_missing"] += 1
        if by_idx and by_idx.get("scale", 0) > 0:
            proj = seed.rust_scaled(by_idx["last_0104"], by_idx["scale"])
            if seed.same_f32(proj, cb_last):
                hits["by_index_hit"] += 1
            else:
                hits["by_index_miss"] += 1
                if len(samples_index_miss) < 6:
                    samples_index_miss.append(
                        {
                            "extract_index": row["index"],
                            "extract_code_via_0903": meta["code"],
                            "morning_code_at_index": by_idx.get("code"),
                            "proj": proj,
                            "cb": cb_last,
                        }
                    )
        else:
            hits["by_index_missing"] += 1

    n = hits["joined"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_last_close_key_index_vs_code.v1",
        "extract_dir": extract_dir,
        "joined": hits["joined"],
        "index_stability_0903_vs_morning": {
            "same_index": stable,
            "moved": moved,
            "code_missing_from_morning": missing_morn,
            "stable_rate": round(stable / max(stable + moved, 1), 6),
        },
        "last_close_on_joined": {
            "by_market_code": {
                "hits": hits["by_code_hit"],
                "misses": hits["by_code_miss"],
                "missing_table": hits["by_code_missing"],
                "rate": round(hits["by_code_hit"] / n, 6),
            },
            "by_market_index_morning_0104": {
                "hits": hits["by_index_hit"],
                "misses": hits["by_index_miss"],
                "missing_table": hits["by_index_missing"],
                "rate": round(hits["by_index_hit"] / n, 6),
            },
        },
        "samples_index_miss": samples_index_miss,
        "note": (
            "Offline seed map is market+code. Runtime same-session 0104 may key "
            "by symbol_index. Cross-session index injection of the 09:15 tables "
            "onto this wine-tail extract is the negative control."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
