#!/usr/bin/env python3
"""236 recovered last-Ok quotes vs last OEM leftover/live.

Section 56 rollback recovers 236 indexes that persist would leave dirty.
Those last-Ok 311Bs may still be leftover (amount/close 0) while the last
OEM callback is already live. Identity can be right and the public state
wrong. Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-recovered-last-ok-vs-oem.py EXTRACT CALLBACK META_0903 MORNING_0104
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


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-recovered-last-ok-vs-oem.py EXTRACT "
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

    persist_st, _ = combo.persist_states(rows, symbol_codes, morning_seeds, False)
    rollback_st, rollback_map = combo.persist_states(
        rows, symbol_codes, morning_seeds, True
    )
    persist_dirty = {
        key for key, st in persist_st.items() if seed.i64(bytes(st), 0x1C) < 0
    }
    recovered = sorted(persist_dirty & set(rollback_st))

    hits = Counter()
    samples_live_vs_zero = []
    samples_both_live = []
    for key in recovered:
        hits["recovered"] += 1
        meta = rollback_map[key]
        st = rollback_st[key]
        amount = seed.i64(bytes(st), 0x1C)
        close_i = seed.i32(bytes(st), 0x10)
        close_f = seed.rust_scaled(close_i, meta.get("scale") or 1.0)
        last_f = seed.rust_scaled(meta.get("last") or 0, meta.get("scale") or 1.0)
        code = meta["code"]
        q = oem.get((key[0], code))
        if q is None:
            hits["no_oem"] += 1
            continue
        hits["has_oem"] += 1
        oem_live = q["price"] != 0
        state_zero = amount == 0 and close_i == 0
        last_ok = seed.same_f32(last_f, q["last_close"])
        if last_ok:
            hits["last_eq"] += 1
        if oem_live:
            hits["oem_live"] += 1
            if state_zero:
                hits["oem_live_state_zero"] += 1
                if len(samples_live_vs_zero) < 6:
                    samples_live_vs_zero.append(
                        {
                            "market": key[0],
                            "index": key[1],
                            "code": code,
                            "oem_price": q["price"],
                            "oem_last": q["last_close"],
                            "state_amount": amount,
                            "state_close": close_f,
                        }
                    )
            elif seed.same_f32(close_f, q["price"]):
                hits["oem_live_close_eq_price"] += 1
            else:
                hits["oem_live_state_nonzero_mismatch"] += 1
                if len(samples_both_live) < 4:
                    samples_both_live.append(
                        {
                            "market": key[0],
                            "index": key[1],
                            "code": code,
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

    n = hits["recovered"] or 1
    o = hits["has_oem"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_recovered_last_ok_vs_oem.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "api_note": (
            "Official5188ShadowDecoder::new fills symbol_codes and "
            "previous_close_seeds from the same code_tables slice; "
            "start_with_code_tables has no split-map argument."
        ),
        "recovered": hits["recovered"],
        "has_oem": hits["has_oem"],
        "no_oem": hits["no_oem"],
        "last_eq": hits["last_eq"],
        "oem_live": hits["oem_live"],
        "oem_leftover": hits["oem_leftover"],
        "oem_live_state_zero": hits["oem_live_state_zero"],
        "oem_live_close_eq_price": hits["oem_live_close_eq_price"],
        "oem_live_state_nonzero_mismatch": hits["oem_live_state_nonzero_mismatch"],
        "oem_leftover_state_zero": hits["oem_leftover_state_zero"],
        "oem_leftover_state_nonzero": hits["oem_leftover_state_nonzero"],
        "rates": {
            "last_eq_among_oem": round(hits["last_eq"] / o, 6),
            "oem_live_among_oem": round(hits["oem_live"] / o, 6),
            "live_oem_but_zero_state": round(hits["oem_live_state_zero"] / o, 6),
        },
        "samples_oem_live_state_zero": samples_live_vs_zero,
        "samples_oem_live_mismatch": samples_both_live,
        "product_wiring": [
            "Rollback recovers a last Ok 311B, not the current OEM leftover/live group.",
            "Do not treat last_close match on recovered indexes as a license to publish that 311B as the live quote.",
            "First-print remains public OEM live. Wine-tail recovered zeros vs live OEM are poll/state-group, not bitstream.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
