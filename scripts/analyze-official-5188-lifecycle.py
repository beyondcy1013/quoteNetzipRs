#!/usr/bin/env python3
"""Bucket 2704 decoder lifecycle traces without changing decoder behavior."""

from __future__ import annotations

import argparse
import collections
import json
import re
from pathlib import Path


ERROR_RE = re.compile(
    r"record (?P<record_index>\d+) mask=(?P<mask>0x[0-9a-f]+) "
    r"mask_class=(?P<mask_class>0x[0-9a-f]+) header=(?P<header>0x[0-9a-f]+) "
    r"clear_ladder=(?P<clear_ladder>true|false) raw_level_count=(?P<raw_level_count>\d+) "
    r"level_count=(?P<level_count>\d+) baseline_mode=(?P<baseline_mode>\w+) "
    r"stored_present=(?P<stored_present>true|false) "
    r"baseline_present=(?P<baseline_present>true|false).*? stage=(?P<stage>\w+)"
)
TARGETS = {43: 3882, 836: 25095, 7644: 3821}


def lifecycle_key(row: dict[str, object]) -> str:
    mask_class = row.get("mask_class")
    mask_class_text = "unknown" if mask_class is None else f"0x{int(mask_class):02x}"
    return "/".join(
        [
            f"stored={str(row['stored_present']).lower()}",
            f"baseline={str(row['baseline_present']).lower()}",
            f"mask_bit0={int(row['mask']) & 1}",
            f"mask_class={mask_class_text}",
            f"clear_ladder={str(row.get('clear_ladder', 'unknown')).lower()}",
            f"raw_level_count={row.get('raw_level_count', 'unknown')}",
            f"level_count={row.get('level_count', 'unknown')}",
        ]
    )


def parse_error(message: str) -> dict[str, object] | None:
    match = ERROR_RE.search(message)
    if match is None:
        return None
    values: dict[str, object] = match.groupdict()
    for name in ("record_index", "raw_level_count", "level_count"):
        values[name] = int(str(values[name]))
    for name in ("mask", "mask_class", "header"):
        values[name] = int(str(values[name]), 16)
    for name in ("clear_ladder", "stored_present", "baseline_present"):
        values[name] = values[name] == "true"
    return values


def sorted_counts(counter: collections.Counter[str]) -> dict[str, int]:
    return dict(sorted(counter.items(), key=lambda item: (-item[1], item[0])))


def i32_at(record: list[int], offset: int) -> int:
    return int.from_bytes(bytes(record[offset : offset + 4]), "little", signed=True)


def record_summary(record: list[int]) -> dict[str, object]:
    return {
        "timestamp": i32_at(record, 0),
        "ohlc": [i32_at(record, offset) for offset in (4, 8, 12, 16)],
        "last": i32_at(record, 16),
        "ladder_prices": [i32_at(record, 0x58 + slot * 4) for slot in range(10)],
        "ladder_volumes": [i32_at(record, 0xA8 + slot * 4) for slot in range(10)],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("extract_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    manifest = json.loads((args.extract_dir / "manifest.json").read_text())
    frame_counts: collections.Counter[str] = collections.Counter()
    record_buckets: dict[str, collections.Counter[str]] = collections.defaultdict(
        collections.Counter
    )
    error_stages: collections.Counter[str] = collections.Counter()
    early_error_stages: collections.Counter[str] = collections.Counter()
    unparsed_errors: list[dict[str, object]] = []
    targets: dict[str, dict[str, object]] = {}
    completed_record_count = 0
    latest_records: dict[tuple[str, tuple[int, int], int], dict[str, object]] = {}

    for ordinal, frame in enumerate(manifest):
        if frame.get("wire_kind") != "2704":
            continue
        trace_name = frame.get("decoded_value_trace_file")
        traces = []
        if trace_name:
            traces = json.loads((args.extract_dir / trace_name).read_text())
        decoded_name = frame.get("decoded_values_file")
        decoded_rows = []
        if decoded_name and any(
            row.get("symbol_index") in TARGETS.values() for row in traces
        ):
            decoded_rows = json.loads((args.extract_dir / decoded_name).read_text())
        error = frame.get("decoded_value_error")
        outcome = "clean" if error is None else ("partial" if traces else "failed")
        frame_counts[outcome] += 1
        for row in traces:
            record_buckets[outcome][lifecycle_key(row)] += 1
            if ordinal in TARGETS and row.get("symbol_index") == TARGETS[ordinal]:
                key = (frame["dst"], tuple(row["market"]), row["symbol_index"])
                decoded_row = next(
                    item
                    for item in decoded_rows
                    if item["index"]["symbol_index"] == row["symbol_index"]
                    and item["index"]["market"] == row["market"]
                )
                targets[str(ordinal)] = {
                    "frame_ordinal": ordinal,
                    "dst": frame.get("dst"),
                    "completed_at_micros": frame.get("completed_at_micros"),
                    "outcome": outcome,
                    "trace": row,
                    "previous": latest_records.get(key),
                    "current": record_summary(decoded_row["record"]),
                }
            completed_record_count += 1
        for decoded_row in decoded_rows:
            index = decoded_row["index"]
            if index["symbol_index"] not in TARGETS.values():
                continue
            key = (frame["dst"], tuple(index["market"]), index["symbol_index"])
            latest_records[key] = {
                "frame_ordinal": ordinal,
                "record_index": decoded_row.get("record_index"),
                **record_summary(decoded_row["record"]),
            }

        if error is not None:
            parsed = parse_error(error)
            if parsed is None:
                unparsed_errors.append({"ordinal": ordinal, "error": error})
                stage_match = re.search(r"stage=(\w+)", error)
                early_error_stages[
                    stage_match.group(1) if stage_match else "unknown"
                ] += 1
            else:
                error_stages[str(parsed["stage"])] += 1
                record_buckets[f"{outcome}_error"][lifecycle_key(parsed)] += 1

    successful_buckets = record_buckets["clean"] + record_buckets["partial"]
    failed_buckets = record_buckets["partial_error"] + record_buckets["failed_error"]
    bucket_error_rates = []
    for key, failed_count in failed_buckets.items():
        successful_count = successful_buckets[key]
        bucket_error_rates.append(
            {
                "key": key,
                "errors": failed_count,
                "completed": successful_count,
                "error_rate": failed_count / (failed_count + successful_count),
            }
        )
    bucket_error_rates.sort(
        key=lambda row: (-row["error_rate"], -row["errors"], row["key"])
    )

    report = {
        "source": str(args.extract_dir),
        "frame_counts": sorted_counts(frame_counts),
        "record_lifecycle_buckets": {
            outcome: sorted_counts(counts)
            for outcome, counts in sorted(record_buckets.items())
        },
        "error_stages": sorted_counts(error_stages),
        "early_error_stages": sorted_counts(early_error_stages),
        "lifecycle_bucket_error_rates": bucket_error_rates,
        "unparsed_error_count": len(unparsed_errors),
        "unparsed_errors": unparsed_errors[:20],
        "completed_record_count": completed_record_count,
        "target_ordinals": targets,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                key: report[key]
                for key in (
                    "frame_counts",
                    "error_stages",
                    "early_error_stages",
                    "unparsed_error_count",
                )
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
