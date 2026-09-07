#!/usr/bin/env python3
"""Audit an anchor-only 2704 replay against its unchanged baseline replay."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


LADDER_PRICE_RANGE = range(0x58, 0x80)


def i32_at(record: list[int], offset: int) -> int:
    return int.from_bytes(bytes(record[offset : offset + 4]), "little", signed=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("baseline_dir", type=Path)
    parser.add_argument("candidate_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    changed_files = 0
    changed_records = 0
    unexpected_records = []
    transitions: dict[str, int] = {}
    transition_shapes: dict[str, int] = {}
    uniform_shifts: dict[str, int] = {}
    transition_examples: dict[str, list[dict[str, object]]] = {}
    examples = []

    for baseline_path in args.baseline_dir.glob("*.decoded-values.json"):
        candidate_path = args.candidate_dir / baseline_path.name
        if baseline_path.read_bytes() == candidate_path.read_bytes():
            continue
        changed_files += 1
        baseline_rows = json.loads(baseline_path.read_text())
        candidate_rows = json.loads(candidate_path.read_text())
        if len(baseline_rows) != len(candidate_rows):
            unexpected_records.append(
                {"file": baseline_path.name, "reason": "record count changed"}
            )
            continue
        for baseline, candidate in zip(baseline_rows, candidate_rows, strict=True):
            before = baseline["record"]
            after = candidate["record"]
            if before == after:
                continue
            changed_records += 1
            changed_offsets = [
                offset for offset, pair in enumerate(zip(before, after, strict=True)) if pair[0] != pair[1]
            ]
            if any(offset not in LADDER_PRICE_RANGE for offset in changed_offsets):
                unexpected_records.append(
                    {
                        "file": baseline_path.name,
                        "index": baseline["index"],
                        "changed_offsets": changed_offsets,
                    }
                )
            before_prices = [i32_at(before, 0x58 + slot * 4) for slot in range(10)]
            after_prices = [i32_at(after, 0x58 + slot * 4) for slot in range(10)]
            transition = (
                f"before_negative={any(value < 0 for value in before_prices)}/"
                f"after_negative={any(value < 0 for value in after_prices)}"
            )
            transitions[transition] = transitions.get(transition, 0) + 1
            shape = (
                f"{transition}/mask=0x{baseline['mask']:02x}/"
                f"clear={baseline['header']['clear_ladder']}/"
                f"raw_levels={baseline['header']['raw_level_count']}"
            )
            transition_shapes[shape] = transition_shapes.get(shape, 0) + 1
            shifts = {
                after_value - before_value
                for before_value, after_value in zip(before_prices, after_prices, strict=True)
                if before_value != after_value
            }
            shift_key = str(next(iter(shifts))) if len(shifts) == 1 else "nonuniform"
            uniform_shifts[shift_key] = uniform_shifts.get(shift_key, 0) + 1
            transition_examples.setdefault(transition, [])
            if len(transition_examples[transition]) < 10:
                transition_examples[transition].append(
                    {
                        "file": baseline_path.name,
                        "index": baseline["index"],
                        "mask": baseline["mask"],
                        "header": baseline["header"],
                        "shift": shift_key,
                        "before_prices": before_prices,
                        "after_prices": after_prices,
                    }
                )
            if len(examples) < 20:
                examples.append(
                    {
                        "file": baseline_path.name,
                        "index": baseline["index"],
                        "mask": baseline["mask"],
                        "header": baseline["header"],
                        "before_prices": before_prices,
                        "after_prices": after_prices,
                    }
                )

    report = {
        "baseline": str(args.baseline_dir),
        "candidate": str(args.candidate_dir),
        "changed_files": changed_files,
        "changed_records": changed_records,
        "transitions": dict(sorted(transitions.items())),
        "transition_shapes": dict(
            sorted(transition_shapes.items(), key=lambda item: (-item[1], item[0]))
        ),
        "uniform_shifts": dict(
            sorted(uniform_shifts.items(), key=lambda item: (-item[1], item[0]))
        ),
        "transition_examples": transition_examples,
        "unexpected_record_count": len(unexpected_records),
        "unexpected_records": unexpected_records[:20],
        "examples": examples,
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                "baseline": report["baseline"],
                "candidate": report["candidate"],
                "changed_files": changed_files,
                "changed_records": changed_records,
                "transitions": report["transitions"],
                "top_transition_shapes": dict(
                    list(report["transition_shapes"].items())[:10]
                ),
                "top_uniform_shifts": dict(list(report["uniform_shifts"].items())[:10]),
                "unexpected_record_count": len(unexpected_records),
            },
            indent=2,
        )
    )
    return 1 if unexpected_records else 0


if __name__ == "__main__":
    raise SystemExit(main())
