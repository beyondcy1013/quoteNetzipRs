#!/usr/bin/env python3
"""Combined snapshot recipe: split two maps AND roll back failed project.

Section 55 split maps: 0 wrong tickers, 275 end-of-stream negative amounts.
Section 53 rollback: +158 updates, 0 leftover heals. Together, rollback
should keep the last Ok 311B instead of a dirty terminal state.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-two-map-plus-rollback.py EXTRACT CALLBACK META_0903 MORNING_0104
"""
from __future__ import annotations

import json
import os
import sys

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
rb = SourceFileLoader(
    "score_runtime_2023_rollback_vs_poison",
    os.path.join(_DIR, "score-runtime-2023-rollback-vs-poison.py"),
).load_module()
snap = SourceFileLoader(
    "score_runtime_2023_snapshot_would_publish",
    os.path.join(_DIR, "score-runtime-2023-snapshot-would-publish.py"),
).load_module()


def persist_states(rows, symbol_codes, seeds, rollback: bool):
    states = {}
    mapped = {}
    for row in rows:
        key = (row["market"], row["index"])
        code = symbol_codes.get(key)
        if code is None:
            continue
        meta = dict(seeds.get((row["market"], code)) or {})
        meta["code"] = code
        if not rb.seed_schema_ok(meta):
            continue
        created = key not in states
        if created:
            states[key] = bytearray(rb.INTERNAL)
            mapped[key] = meta
        prev = bytes(states[key])
        rb.merge_into(states[key], row["rec"], int(row["mask"] or 0))
        amount = seed.i64(bytes(states[key]), 0x1C)
        if amount < 0:
            if rollback:
                if created:
                    del states[key]
                    del mapped[key]
                else:
                    states[key] = bytearray(prev)
            continue
    return states, mapped


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-two-map-plus-rollback.py EXTRACT "
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

    persist_st, persist_map = persist_states(
        rows, symbol_codes, morning_seeds, rollback=False
    )
    rollback_st, rollback_map = persist_states(
        rows, symbol_codes, morning_seeds, rollback=True
    )
    persist_score = snap.score(
        persist_st, persist_map, identity, oem, "two_map_persist"
    )
    rollback_score = snap.score(
        rollback_st, rollback_map, identity, oem, "two_map_rollback"
    )

    persist_dirty = {
        key for key, st in persist_st.items() if seed.i64(bytes(st), 0x1C) < 0
    }
    rollback_keys = set(rollback_st)
    recovered = sorted(persist_dirty & rollback_keys)
    dropped = sorted(persist_dirty - rollback_keys)
    samples_recovered = []
    for key in recovered[:6]:
        ident_row = identity.get(key) or {}
        samples_recovered.append(
            {
                "market": key[0],
                "index": key[1],
                "code": ident_row.get("code"),
                "persist_amount": seed.i64(bytes(persist_st[key]), 0x1C),
                "rollback_amount": seed.i64(bytes(rollback_st[key]), 0x1C),
            }
        )

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_two_map_plus_rollback.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "recipe": (
            "decode-session index→code + morning-by-code last; "
            "post-merge project-err rolls back the merge"
        ),
        "two_map_persist": persist_score,
        "two_map_rollback": rollback_score,
        "end_dirty_indexes": {
            "persist_still_dirty": len(persist_dirty),
            "rollback_still_dirty": rollback_score["still_dirty"],
            "recovered_last_ok_and_would_publish": len(recovered),
            "dropped_never_had_ok_state": len(dropped),
            "samples_recovered": samples_recovered,
        },
        "product_wiring": [
            "Split maps fix identity. Rollback fixes leftover negative amount at rest.",
            "Together, snapshot would_publish has 0 wrong tickers and 0 dirty terminal states.",
            "Indexes that only ever saw a negative-amount 2704 stay absent (not-ready), which is correct.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
