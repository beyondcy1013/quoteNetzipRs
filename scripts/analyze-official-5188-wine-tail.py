#!/usr/bin/env python3
"""Compare strict 2704 short-tail failures with explicit Wine-tail replay."""

from __future__ import annotations

import argparse
import collections
import json
import re
from pathlib import Path


EARLY_ERROR_RE = re.compile(r"record (?P<record>\d+).*stage=(?P<stage>record_mask|record_header)")


def summary(record: list[int]) -> dict[str, object]:
    def i32(offset: int) -> int:
        return int.from_bytes(bytes(record[offset : offset + 4]), "little", signed=True)

    return {
        "timestamp": i32(0),
        "ohlc": [i32(offset) for offset in (4, 8, 12, 16)],
        "volume": int.from_bytes(bytes(record[0x14:0x1C]), "little", signed=True),
        "amount": int.from_bytes(bytes(record[0x24:0x2C]), "little", signed=True),
        "ladder_prices": [i32(0x58 + slot * 4) for slot in range(10)],
        "ladder_volumes": [i32(0xA8 + slot * 4) for slot in range(10)],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("strict_dir", type=Path)
    parser.add_argument("wine_tail_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    strict_manifest = json.loads((args.strict_dir / "manifest.json").read_text())
    wine_manifest = json.loads((args.wine_tail_dir / "manifest.json").read_text())
    if len(strict_manifest) != len(wine_manifest):
        raise SystemExit("manifest lengths differ")

    stage_counts: collections.Counter[str] = collections.Counter()
    extra_counts: collections.Counter[int] = collections.Counter()
    extra_shapes: collections.Counter[str] = collections.Counter()
    prefix_mismatches = []
    unresolved = []
    examples = []
    zero_bit_extra_records = 0
    zero_public_value_extra_records = 0
    total_extra_records = 0

    for ordinal, (strict, wine) in enumerate(zip(strict_manifest, wine_manifest, strict=True)):
        error = strict.get("decoded_value_error") or ""
        match = EARLY_ERROR_RE.search(error)
        if match is None:
            continue
        stage = match.group("stage")
        stage_counts[stage] += 1
        strict_name = strict.get("decoded_values_file")
        wine_name = wine.get("decoded_values_file")
        strict_rows = (
            json.loads((args.strict_dir / strict_name).read_text()) if strict_name else []
        )
        wine_rows = (
            json.loads((args.wine_tail_dir / wine_name).read_text()) if wine_name else []
        )
        if strict_rows != wine_rows[: len(strict_rows)]:
            prefix_mismatches.append(ordinal)
        extra = wine_rows[len(strict_rows) :]
        extra_counts[len(extra)] += 1
        if not extra:
            unresolved.append({"ordinal": ordinal, "stage": stage, "error": wine.get("decoded_value_error")})
            continue
        for row in extra:
            total_extra_records += 1
            if row["bit_start"] == row["bit_end"]:
                zero_bit_extra_records += 1
            row_summary = summary(row["record"])
            if (
                row_summary["ohlc"] == [0, 0, 0, 0]
                and row_summary["volume"] == 0
                and row_summary["amount"] == 0
                and row_summary["ladder_prices"] == [0] * 10
                and row_summary["ladder_volumes"] == [0] * 10
            ):
                zero_public_value_extra_records += 1
            shape = (
                f"mask=0x{row['mask']:02x}/clear={row['header']['clear_ladder']}/"
                f"raw_levels={row['header']['raw_level_count']}"
            )
            extra_shapes[shape] += 1
        if len(examples) < 30:
            examples.append(
                {
                    "ordinal": ordinal,
                    "stage": stage,
                    "strict_records": len(strict_rows),
                    "index_count": strict.get("delta_index_count"),
                    "wine_records": len(wine_rows),
                    "strict_remaining_bits": strict.get("decoded_value_remaining_bits"),
                    "wine_error": wine.get("decoded_value_error"),
                    "extra": [
                        {
                            "index": row["index"],
                            "mask": row["mask"],
                            "header": row["header"],
                            "bit_start": row["bit_start"],
                            "bit_end": row["bit_end"],
                            "summary": summary(row["record"]),
                        }
                        for row in extra
                    ],
                }
            )

    report = {
        "strict": str(args.strict_dir),
        "wine_tail": str(args.wine_tail_dir),
        "stage_counts": dict(stage_counts),
        "extra_record_counts_per_frame": {
            str(key): value for key, value in sorted(extra_counts.items())
        },
        "extra_shapes": dict(sorted(extra_shapes.items(), key=lambda item: (-item[1], item[0]))),
        "prefix_mismatch_count": len(prefix_mismatches),
        "prefix_mismatch_ordinals": prefix_mismatches[:30],
        "unresolved_count": len(unresolved),
        "total_extra_records": total_extra_records,
        "zero_bit_extra_records": zero_bit_extra_records,
        "zero_public_value_extra_records": zero_public_value_extra_records,
        "unresolved": unresolved[:30],
        "examples": examples,
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {key: value for key, value in report.items() if key not in {"examples", "unresolved", "prefix_mismatch_ordinals", "extra_shapes"}},
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
