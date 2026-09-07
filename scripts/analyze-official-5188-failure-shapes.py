#!/usr/bin/env python3
"""Aggregate 0x2704 decoder failures without replaying or mutating a session."""

from __future__ import annotations

import argparse
import collections
import datetime as dt
import json
import re
from pathlib import Path


ERROR_FIELDS = {
    "record": re.compile(r"\brecord (\d+)"),
    "mask": re.compile(r"\bmask=0x([0-9a-f]+)"),
    "header": re.compile(r"\bheader=0x([0-9a-f]+)"),
    "baseline": re.compile(r"\bbaseline_mode=(\w+)"),
    "record_start": re.compile(r"\brecord_start=(\d+)"),
    "stage": re.compile(r"\bstage=(\w+)"),
    "bit": re.compile(r"\bbit=(\d+)"),
}


def top(counter: collections.Counter[object], limit: int) -> list[dict[str, object]]:
    return [
        {"value": value, "count": count}
        for value, count in counter.most_common(limit)
    ]


def port_family(destination: str) -> str:
    port = int(destination.rsplit(":", 1)[1])
    if 36000 <= port < 37000:
        return "36xxx"
    if 42000 <= port < 43000:
        return "42xxx"
    if 43000 <= port < 44000:
        return "43xxx"
    return "other"


def local_time(micros: int) -> dt.datetime:
    return dt.datetime.fromtimestamp(micros / 1_000_000).astimezone()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--start", default="14:58:00")
    parser.add_argument("--end", default="15:00:00")
    parser.add_argument("--top", type=int, default=30)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    frames = json.loads(args.manifest.read_text())
    selected: list[dict[str, object]] = []
    parse_failures = 0
    for frame in frames:
        if frame.get("wire_kind") != "2704":
            continue
        timestamp = local_time(int(frame["completed_at_micros"]))
        clock = timestamp.strftime("%H:%M:%S")
        if not (args.start <= clock < args.end):
            continue
        error = frame.get("decoded_value_error")
        parsed_error = None
        if error:
            parsed_error = {
                name: match.group(1) if (match := pattern.search(str(error))) else None
                for name, pattern in ERROR_FIELDS.items()
            }
            if parsed_error["stage"] is None:
                parse_failures += 1
        selected.append(
            {
                "timestamp": timestamp,
                "port_family": port_family(str(frame["dst"])),
                "payload_len": int(frame["payload_len"]),
                "delta_index_count": int(frame.get("delta_index_count") or 0),
                "failed": error is not None,
                "error": parsed_error,
            }
        )

    failures = [frame for frame in selected if frame["failed"]]
    clean = [frame for frame in selected if not frame["failed"]]

    def count(items: list[dict[str, object]], key: str) -> collections.Counter[object]:
        return collections.Counter(item[key] for item in items)

    stages = collections.Counter(frame["error"]["stage"] for frame in failures)
    masks = collections.Counter(
        f"0x{frame['error']['mask']}" if frame["error"]["mask"] else None
        for frame in failures
    )
    headers = collections.Counter(
        f"0x{frame['error']['header']}" if frame["error"]["header"] else None
        for frame in failures
    )
    starts = collections.Counter(
        int(frame["error"]["record_start"])
        if frame["error"]["record_start"]
        else None
        for frame in failures
    )
    record_positions = collections.Counter(
        (int(frame["error"]["record"]) if frame["error"]["record"] else None,
         frame["delta_index_count"])
        for frame in failures
    )
    signatures = collections.Counter(
        (
            frame["error"]["stage"],
            f"0x{frame['error']['mask']}" if frame["error"]["mask"] else None,
            f"0x{frame['error']['header']}" if frame["error"]["header"] else None,
            frame["error"]["baseline"],
            frame["payload_len"],
        )
        for frame in failures
    )
    signature_rows = [
        {
            "stage": value[0],
            "mask": value[1],
            "header": value[2],
            "baseline_mode": value[3],
            "payload_len": value[4],
            "count": occurrence_count,
        }
        for value, occurrence_count in signatures.most_common(args.top)
    ]

    ports: dict[str, dict[str, object]] = {}
    for family in sorted({str(frame["port_family"]) for frame in selected}):
        family_frames = [frame for frame in selected if frame["port_family"] == family]
        family_failures = [frame for frame in family_frames if frame["error"]]
        ports[family] = {
            "frames": len(family_frames),
            "clean": len(family_frames) - len(family_failures),
            "failed": len(family_failures),
            "failure_rate": round(len(family_failures) / len(family_frames), 6),
            "failure_stages": top(
                collections.Counter(frame["error"]["stage"] for frame in family_failures),
                args.top,
            ),
        }

    record_counts = []
    for record_count in sorted({int(frame["delta_index_count"]) for frame in selected}):
        count_frames = [
            frame for frame in selected if frame["delta_index_count"] == record_count
        ]
        count_failed = sum(bool(frame["failed"]) for frame in count_frames)
        record_counts.append(
            {
                "delta_index_count": record_count,
                "frames": len(count_frames),
                "clean": len(count_frames) - count_failed,
                "failed": count_failed,
                "failure_rate": round(count_failed / len(count_frames), 6),
            }
        )

    report = {
        "schema": "quoteNetzipRs.official_5188_failure_shapes.v1",
        "manifest": str(args.manifest),
        "window": {"start": args.start, "end_exclusive": args.end},
        "frames": len(selected),
        "clean": len(clean),
        "failed": len(failures),
        "failure_rate": round(len(failures) / len(selected), 6) if selected else 0,
        "unparsed_failure_messages": parse_failures,
        "clean_payload_lengths": top(count(clean, "payload_len"), args.top),
        "failed_payload_lengths": top(count(failures, "payload_len"), args.top),
        "failure_stages": top(stages, args.top),
        "failure_masks": top(masks, args.top),
        "failure_headers": top(headers, args.top),
        "failure_record_starts": top(starts, args.top),
        "failure_record_positions": [
            {
                "record": value[0],
                "delta_index_count": value[1],
                "count": occurrence_count,
            }
            for value, occurrence_count in record_positions.most_common(args.top)
        ],
        "failure_signatures": signature_rows,
        "record_count_distribution": record_counts,
        "port_families": ports,
    }
    output = json.dumps(report, ensure_ascii=True, indent=2) + "\n"
    if args.output:
        args.output.write_text(output)
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
