#!/usr/bin/env python3
"""Validate a redacted official 5188 subscription-partition summary."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


WIRE_FLAG = 0x10000
SYMBOL_INDEX_MASK = 0x7FFF


def load_partitions(path: Path) -> list[dict[str, Any]]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, list) or not document:
        raise ValueError("partition summary must be a non-empty JSON array")
    return document


def expand_partition(partition: dict[str, Any], ordinal: int) -> list[tuple[str, int]]:
    declared = partition.get("entries")
    ranges = partition.get("ranges")
    if not isinstance(declared, int) or declared < 0 or not isinstance(ranges, list):
        raise ValueError(f"partition {ordinal}: invalid entries or ranges")

    entries: list[tuple[str, int]] = []
    for range_ordinal, item in enumerate(ranges, 1):
        if (
            not isinstance(item, list)
            or len(item) != 3
            or item[0] not in ("SH", "SZ")
            or not isinstance(item[1], int)
            or not isinstance(item[2], int)
            or item[2] <= 0
        ):
            raise ValueError(f"partition {ordinal} range {range_ordinal}: invalid range")
        market, first, count = item
        for wire_value in range(first, first + count):
            if wire_value & WIRE_FLAG == 0:
                raise ValueError(
                    f"partition {ordinal}: wire value {wire_value} lacks 0x10000 flag"
                )
            symbol_index = wire_value & SYMBOL_INDEX_MASK
            if symbol_index > SYMBOL_INDEX_MASK:
                raise ValueError(
                    f"partition {ordinal}: symbol index {symbol_index} exceeds mask"
                )
            entries.append((market, wire_value))

    if len(entries) != declared:
        raise ValueError(
            f"partition {ordinal}: declared {declared}, expanded {len(entries)}"
        )
    return entries


def validate(path: Path, expected_sizes: list[int] | None) -> dict[str, Any]:
    partitions = load_partitions(path)
    expanded = [
        expand_partition(partition, ordinal)
        for ordinal, partition in enumerate(partitions, 1)
    ]
    sizes = [len(entries) for entries in expanded]
    if expected_sizes is not None and sizes != expected_sizes:
        raise ValueError(f"partition sizes {sizes} do not match expected {expected_sizes}")

    flattened = [entry for partition in expanded for entry in partition]
    if len(set(flattened)) != len(flattened):
        raise ValueError("duplicate market/wire entries exist across partitions")

    boundary_continuity = []
    for left, right in zip(expanded, expanded[1:]):
        previous = left[-1]
        following = right[0]
        boundary_continuity.append(
            previous[0] == following[0] and previous[1] + 1 == following[1]
        )

    return {
        "partition_count": len(expanded),
        "partition_sizes": sizes,
        "total_entries": len(flattened),
        "unique_entries": len(set(flattened)),
        "boundary_continuity": boundary_continuity,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("summary", type=Path)
    parser.add_argument(
        "--expected-sizes",
        help="comma-separated exact partition sizes, for example 1024,1024,122",
    )
    args = parser.parse_args()
    expected_sizes = (
        [int(value) for value in args.expected_sizes.split(",")]
        if args.expected_sizes
        else None
    )
    result = validate(args.summary, expected_sizes)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
