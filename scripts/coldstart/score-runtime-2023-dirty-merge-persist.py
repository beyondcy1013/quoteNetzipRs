#!/usr/bin/env python3
"""Runtime 20:23 post-merge project().is_err does not un-merge amount.

Negative 2704 amount is copied by OemState.merge (nonzero 8-byte field),
then project fails and the state stays. Later timestamp-only / zero-amount
rows keep failing. A later positive amount heals the gate and would count
as oem_state_updates.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-dirty-merge-persist.py EXTRACT MORNING_0104 NIGHT_EXTRACT NIGHT_0104
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


def six_digit(code: str) -> bool:
    return len(code) == 6 and code.isdigit()


def seed_schema_ok(mapped_row: dict) -> bool:
    return mapped_row.get("scale", 0) > 0 and six_digit(mapped_row.get("code") or "")


def simulate(rows: list, symbol_codes: dict, seeds: dict) -> dict:
    hits = Counter()
    states: dict[tuple[str, int], dict] = {}
    samples_stuck = []
    samples_healed = []
    samples_first_dirty = []
    unique_ever_dirty = set()
    unique_stuck_later = set()
    unique_healed = set()
    unique_first_dirty = set()

    for row in rows:
        hits["decoded"] += 1
        key = (row["market"], row["index"])
        mapped_code = symbol_codes.get(key)
        if mapped_code is None:
            hits["missing_index"] += 1
            continue
        mapped_row = dict(seeds.get((row["market"], mapped_code)) or {})
        mapped_row["code"] = mapped_code
        if not seed_schema_ok(mapped_row):
            hits["seed_project_err"] += 1
            continue
        incoming_amount = seed.i64(row["rec"], 0x1C)
        created = key not in states
        if created:
            states[key] = {"amount": 0, "ever_dirty": False, "updates": 0, "missing": 0}
        st = states[key]
        was_dirty = st["amount"] < 0
        mask = int(row["mask"] or 0)
        if mask & 0x38 != 0x18:
            if incoming_amount != 0:
                st["amount"] = incoming_amount
        now_dirty = st["amount"] < 0
        if now_dirty:
            hits["missing"] += 1
            st["missing"] += 1
            unique_ever_dirty.add(key)
            st["ever_dirty"] = True
            if created:
                hits["first_insert_dirty"] += 1
                unique_first_dirty.add(key)
                if len(samples_first_dirty) < 4:
                    samples_first_dirty.append(
                        {
                            "market": row["market"],
                            "index": row["index"],
                            "mapped": mapped_code,
                            "incoming_amount": incoming_amount,
                            "mask": mask,
                        }
                    )
            elif was_dirty:
                hits["later_stuck_on_negative"] += 1
                unique_stuck_later.add(key)
                if incoming_amount == 0 or mask & 0x38 == 0x18:
                    hits["later_stuck_zero_or_ts_only"] += 1
                if len(samples_stuck) < 4:
                    samples_stuck.append(
                        {
                            "market": row["market"],
                            "index": row["index"],
                            "mapped": mapped_code,
                            "incoming_amount": incoming_amount,
                            "state_amount": st["amount"],
                            "mask": mask,
                        }
                    )
            else:
                hits["later_poisons_ok_state"] += 1
            continue
        hits["updates"] += 1
        st["updates"] += 1
        if st["ever_dirty"]:
            hits["later_healed_then_update"] += 1
            unique_healed.add(key)
            if len(samples_healed) < 4:
                samples_healed.append(
                    {
                        "market": row["market"],
                        "index": row["index"],
                        "mapped": mapped_code,
                        "incoming_amount": incoming_amount,
                    }
                )

    n = hits["decoded"] or 1
    return {
        "decoded": hits["decoded"],
        "states": len(states),
        "updates": hits["updates"],
        "missing": hits["missing"]
        + hits["missing_index"]
        + hits["seed_project_err"],
        "missing_index": hits["missing_index"],
        "seed_project_err": hits["seed_project_err"],
        "post_merge_project_err": hits["missing"],
        "first_insert_dirty": hits["first_insert_dirty"],
        "later_poisons_ok_state": hits["later_poisons_ok_state"],
        "later_stuck_on_negative": hits["later_stuck_on_negative"],
        "later_stuck_zero_or_ts_only": hits["later_stuck_zero_or_ts_only"],
        "later_healed_then_update": hits["later_healed_then_update"],
        "unique_index_ever_dirty": len(unique_ever_dirty),
        "unique_index_first_dirty": len(unique_first_dirty),
        "unique_index_stuck_later": len(unique_stuck_later),
        "unique_index_healed": len(unique_healed),
        "conservation_decoded_eq_updates_plus_missing": hits["decoded"]
        == hits["updates"]
        + hits["missing"]
        + hits["missing_index"]
        + hits["seed_project_err"],
        "rates": {
            "post_merge_err": round(hits["missing"] / n, 6),
            "healed_among_decoded": round(hits["later_healed_then_update"] / n, 6),
        },
        "samples_first_dirty": samples_first_dirty,
        "samples_stuck": samples_stuck,
        "samples_healed": samples_healed,
    }


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-dirty-merge-persist.py EXTRACT MORNING_0104 "
            "NIGHT_EXTRACT NIGHT_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, morning_dir, night_extract, night_0104 = sys.argv[1:]
    morning_tables = two.load_all_tables(morning_dir)
    symbol_codes, seeds, _ = two.simulate_runtime_maps(morning_tables)
    rows, _ = seed.load_decoded(extract_dir)
    auction = simulate(rows, symbol_codes, seeds)

    night_tables = two.load_all_tables(night_0104)
    night_codes, night_seeds, _ = two.simulate_runtime_maps(night_tables)
    night_rows, _ = seed.load_decoded(night_extract)
    night = simulate(night_rows, night_codes, night_seeds)

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_dirty_merge_persist.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "merge_rule": (
            "OemState.merge copies amount at 0x1c when the incoming 8 bytes "
            "are not all zero and mask&0x38 != 0x18; project then rejects "
            "amount<0; runtime does not roll back the copy."
        ),
        "auction_wine_tail_stale_0915": auction,
        "night_same_session": night,
        "product_wiring": [
            "A negative-amount 2704 row is already inside oem_states when project fails.",
            "Later zero-amount or timestamp-only rows stay not-ready because merge will not overwrite a leftover negative amount with zeros.",
            "A later positive amount heals the schema gate and increments oem_state_updates on the still-wrong stale ticker.",
            "Conservation decoded == updates + missing can still hold while the dirty state remains.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
