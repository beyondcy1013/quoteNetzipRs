#!/usr/bin/env python3
"""Persist-clean two-map quotes vs last OEM leftover/live.

Section 57 scored the 236 rollback extras. This scores the 5,886 indexes
persist would already publish (amount>=0): last_close can be right while
the 311B is still leftover zeros or a nonzero price that is not OEM live.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-persist-clean-vs-oem-live.py EXTRACT CALLBACK META_0903 MORNING_0104
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
two = SourceFileLoader(
    "score_runtime_2010_two_step_seed",
    os.path.join(_DIR, "score-runtime-2010-two-step-seed.py"),
).load_module()
combo = SourceFileLoader(
    "score_runtime_2023_two_map_plus_rollback",
    os.path.join(_DIR, "score-runtime-2023-two-map-plus-rollback.py"),
).load_module()
snap = SourceFileLoader(
    "score_runtime_2023_snapshot_would_publish",
    os.path.join(_DIR, "score-runtime-2023-snapshot-would-publish.py"),
).load_module()


def classify(states, mapped, oem):
    hits = Counter()
    samples_zero = []
    samples_mismatch = []
    samples_eq = []
    for key, st in states.items():
        amount = seed.i64(bytes(st), 0x1C)
        if amount < 0:
            continue
        hits["publishable"] += 1
        meta = mapped[key]
        close_i = seed.i32(bytes(st), 0x10)
        close_f = seed.rust_scaled(close_i, meta.get("scale") or 1.0)
        last_f = seed.rust_scaled(meta.get("last") or 0, meta.get("scale") or 1.0)
        q = oem.get((key[0], meta["code"]))
        if q is None:
            hits["no_oem"] += 1
            continue
        hits["has_oem"] += 1
        if seed.same_f32(last_f, q["last_close"]):
            hits["last_eq"] += 1
        oem_live = q["price"] != 0
        state_zero = amount == 0 and close_i == 0
        if oem_live:
            hits["oem_live"] += 1
            if state_zero:
                hits["oem_live_state_zero"] += 1
                if len(samples_zero) < 4:
                    samples_zero.append(
                        {
                            "market": key[0],
                            "index": key[1],
                            "code": meta["code"],
                            "oem_price": q["price"],
                        }
                    )
            elif seed.same_f32(close_f, q["price"]):
                hits["oem_live_close_eq"] += 1
                if len(samples_eq) < 3:
                    samples_eq.append(
                        {
                            "market": key[0],
                            "index": key[1],
                            "code": meta["code"],
                            "price": q["price"],
                        }
                    )
            else:
                hits["oem_live_mismatch"] += 1
                if len(samples_mismatch) < 4:
                    samples_mismatch.append(
                        {
                            "market": key[0],
                            "index": key[1],
                            "code": meta["code"],
                            "oem_price": q["price"],
                            "state_close": close_f,
                            "state_amount": amount,
                        }
                    )
        else:
            hits["oem_leftover"] += 1
            if state_zero:
                hits["oem_leftover_state_zero"] += 1
            else:
                hits["oem_leftover_state_nonzero"] += 1
    o = hits["has_oem"] or 1
    return {
        "publishable": hits["publishable"],
        "has_oem": hits["has_oem"],
        "no_oem": hits["no_oem"],
        "last_eq": hits["last_eq"],
        "oem_live": hits["oem_live"],
        "oem_leftover": hits["oem_leftover"],
        "oem_live_state_zero": hits["oem_live_state_zero"],
        "oem_live_close_eq": hits["oem_live_close_eq"],
        "oem_live_mismatch": hits["oem_live_mismatch"],
        "oem_leftover_state_zero": hits["oem_leftover_state_zero"],
        "oem_leftover_state_nonzero": hits["oem_leftover_state_nonzero"],
        "rates": {
            "last_eq_among_oem": round(hits["last_eq"] / o, 6),
            "oem_live_among_oem": round(hits["oem_live"] / o, 6),
            "live_close_eq_among_live": round(
                hits["oem_live_close_eq"] / (hits["oem_live"] or 1), 6
            ),
            "live_zero_among_live": round(
                hits["oem_live_state_zero"] / (hits["oem_live"] or 1), 6
            ),
        },
        "samples_oem_live_state_zero": samples_zero,
        "samples_oem_live_close_eq": samples_eq,
        "samples_oem_live_mismatch": samples_mismatch,
    }


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-persist-clean-vs-oem-live.py EXTRACT "
            "CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    identity = seed.load_index_metadata(meta_0903)
    symbol_codes = {key: row["code"] for key, row in identity.items()}
    _, morning_seeds, _ = two.simulate_runtime_maps(two.load_all_tables(morning_dir))
    rows, _ = seed.load_decoded(extract_dir)
    oem = snap.last_oem(callback_jsonl)
    persist_st, persist_map = combo.persist_states(
        rows, symbol_codes, morning_seeds, False
    )
    rollback_st, rollback_map = combo.persist_states(
        rows, symbol_codes, morning_seeds, True
    )
    persist_dirty = {
        key for key, st in persist_st.items() if seed.i64(bytes(st), 0x1C) < 0
    }
    persist_clean = {
        key: persist_st[key] for key in persist_st if key not in persist_dirty
    }
    persist_clean_map = {key: persist_map[key] for key in persist_clean}
    recovered = persist_dirty & set(rollback_st)

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_persist_clean_vs_oem_live.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "persist_clean": classify(persist_clean, persist_clean_map, oem),
        "rollback_all": classify(rollback_st, rollback_map, oem),
        "recovered_count": len(recovered),
        "product_wiring": [
            "last_close identity on persist-clean quotes does not mean the 311B is OEM live.",
            "Wine-tail end-of-stream snapshot would mostly emit leftover or wrong-session prices next to live OEM.",
            "First-print remains public OEM live. Do not treat end-of-stream persist close as the auction print.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
