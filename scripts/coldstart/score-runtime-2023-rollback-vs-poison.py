#!/usr/bin/env python3
"""Rollback-on-project-err vs leftover field poison after a 'heal'.

Runtime 20:23 keeps the merged 311B when post-merge project fails. A later
positive amount reopens the gate (section 52). This scores:

1. What conservation would look like if project-err rolled back the merge.
2. Which nonzero OHLC/book/volume slots from the dirty record are still in
   oem_states at the first heal, because sparse merge will not copy zeros.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-rollback-vs-poison.py EXTRACT MORNING_0104 NIGHT_EXTRACT NIGHT_0104
"""
from __future__ import annotations

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

INTERNAL = 0x137
SLOTS = (
    [("ts", 0x00, 4), ("open", 0x04, 4), ("high", 0x08, 4), ("low", 0x0C, 4), ("close", 0x10, 4),
     ("volume", 0x14, 8), ("amount", 0x1C, 8)]
    + [(f"bid_px_{i}", 0x58 + i * 4, 4) for i in range(5)]
    + [(f"ask_px_{i}", 0x6C + i * 4, 4) for i in range(5)]
    + [(f"bid_vol_{i}", 0xA8 + i * 4, 4) for i in range(5)]
    + [(f"ask_vol_{i}", 0xBC + i * 4, 4) for i in range(5)]
)


def six_digit(code: str) -> bool:
    return len(code) == 6 and code.isdigit()


def seed_schema_ok(mapped_row: dict) -> bool:
    return mapped_row.get("scale", 0) > 0 and six_digit(mapped_row.get("code") or "")


def merge_into(state: bytearray, incoming: bytes, mask: int) -> None:
    state[0xDF:0xE3] = incoming[0xDF:0xE3]
    if incoming[:4] != b"\x00\x00\x00\x00":
        state[:4] = incoming[:4]
    if mask & 0x38 == 0x18:
        return
    for offset, length in (
        (0x04, 4),
        (0x08, 4),
        (0x0C, 4),
        (0x10, 4),
        (0x14, 8),
        (0x1C, 8),
    ):
        chunk = incoming[offset : offset + length]
        if any(chunk):
            state[offset : offset + length] = chunk
    for offset in range(0x58, 0x80, 4):
        chunk = incoming[offset : offset + 4]
        if any(chunk):
            state[offset : offset + 4] = chunk
    for offset in range(0xA8, 0xD0, 4):
        chunk = incoming[offset : offset + 4]
        if any(chunk):
            state[offset : offset + 4] = chunk


def leftover_slots(dirty: bytes, incoming: bytes, healed: bytes, mask: int) -> list[str]:
    names = []
    ts_only = mask & 0x38 == 0x18
    for name, offset, length in SLOTS:
        if name == "amount":
            continue
        d = dirty[offset : offset + length]
        h = healed[offset : offset + length]
        inc = incoming[offset : offset + length]
        if not any(d):
            continue
        copied = False
        if name == "ts":
            copied = incoming[:4] != b"\x00\x00\x00\x00"
        elif not ts_only:
            copied = any(inc)
        if (not copied) and h == d:
            names.append(name)
    return names


def simulate(rows: list, symbol_codes: dict, seeds: dict, rollback: bool) -> dict:
    hits = Counter()
    states: dict[tuple[str, int], bytearray] = {}
    last_dirty: dict[tuple[str, int], bytes] = {}
    leftover_by_slot = Counter()
    samples_leftover = []
    unique_leftover = set()
    unique_heal = set()

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
        incoming = row["rec"]
        mask = int(row["mask"] or 0)
        created = key not in states
        if created:
            states[key] = bytearray(INTERNAL)
        prev = bytes(states[key])
        merge_into(states[key], incoming, mask)
        amount = seed.i64(bytes(states[key]), 0x1C)
        if amount < 0:
            hits["missing"] += 1
            last_dirty[key] = bytes(states[key])
            if rollback:
                if created:
                    del states[key]
                    last_dirty.pop(key, None)
                    hits["rollback_drop_new"] += 1
                else:
                    states[key] = bytearray(prev)
                    hits["rollback_restore"] += 1
            continue
        hits["updates"] += 1
        if key in last_dirty and not rollback:
            hits["heal_updates"] += 1
            unique_heal.add(key)
            left = leftover_slots(last_dirty[key], incoming, bytes(states[key]), mask)
            if left:
                hits["heal_with_leftover_slots"] += 1
                unique_leftover.add(key)
                for name in left:
                    leftover_by_slot[name] += 1
                if len(samples_leftover) < 6:
                    samples_leftover.append(
                        {
                            "market": row["market"],
                            "index": row["index"],
                            "mapped": mapped_code,
                            "leftover_slots": left,
                            "heal_amount": seed.i64(incoming, 0x1C),
                        }
                    )
            last_dirty.pop(key, None)

    n = hits["decoded"] or 1
    return {
        "rollback": rollback,
        "decoded": hits["decoded"],
        "states": len(states),
        "updates": hits["updates"],
        "missing": hits["missing"] + hits["missing_index"] + hits["seed_project_err"],
        "missing_index": hits["missing_index"],
        "seed_project_err": hits["seed_project_err"],
        "post_merge_project_err": hits["missing"],
        "rollback_drop_new": hits["rollback_drop_new"],
        "rollback_restore": hits["rollback_restore"],
        "heal_updates": hits["heal_updates"],
        "heal_with_leftover_slots": hits["heal_with_leftover_slots"],
        "unique_index_healed": len(unique_heal),
        "unique_index_heal_leftover": len(unique_leftover),
        "leftover_slot_counts": dict(leftover_by_slot),
        "conservation": hits["decoded"]
        == hits["updates"] + hits["missing"] + hits["missing_index"] + hits["seed_project_err"],
        "update_rate": round(hits["updates"] / n, 6),
        "samples_heal_leftover": samples_leftover,
    }


def main() -> int:
    import json

    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-rollback-vs-poison.py EXTRACT MORNING_0104 "
            "NIGHT_EXTRACT NIGHT_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, morning_dir, night_extract, night_0104 = sys.argv[1:]
    morning_tables = two.load_all_tables(morning_dir)
    symbol_codes, seeds, _ = two.simulate_runtime_maps(morning_tables)
    rows, _ = seed.load_decoded(extract_dir)
    persist = simulate(rows, symbol_codes, seeds, rollback=False)
    rolled = simulate(rows, symbol_codes, seeds, rollback=True)

    night_tables = two.load_all_tables(night_0104)
    night_codes, night_seeds, _ = two.simulate_runtime_maps(night_tables)
    night_rows, _ = seed.load_decoded(night_extract)
    night_persist = simulate(night_rows, night_codes, night_seeds, rollback=False)
    night_rolled = simulate(night_rows, night_codes, night_seeds, rollback=True)

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_rollback_vs_poison.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "auction_wine_tail_stale_0915": {
            "persist": persist,
            "rollback": rolled,
            "delta_updates": rolled["updates"] - persist["updates"],
            "delta_missing": rolled["missing"] - persist["missing"],
        },
        "night_same_session": {
            "persist": night_persist,
            "rollback": night_rolled,
            "delta_updates": night_rolled["updates"] - night_persist["updates"],
        },
        "product_wiring": [
            "project-err rollback would drop a failed first insert and restore the previous Ok 311B.",
            "A later positive amount currently heals the schema gate while sparse-merge leftover OHLC/book/volume from the dirty record can remain.",
            "Night same-session dump has no negative amounts, so persist and rollback are identical.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
