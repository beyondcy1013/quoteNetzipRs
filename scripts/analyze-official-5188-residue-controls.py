#!/usr/bin/env python3
"""Report residue-offset failures and clean trace controls for one 2704 replay."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any


ERROR = {
    "record": re.compile(r"\brecord (\d+)"),
    "mask": re.compile(r"\bmask=0x([0-9a-f]+)"),
    "header": re.compile(r"\bheader=0x([0-9a-f]+)"),
    "baseline_mode": re.compile(r"\bbaseline_mode=(\w+)"),
    "stage": re.compile(r"\bstage=(\w+)"),
    "bit": re.compile(r"\bbit=(\d+)"),
}


def parse_error(message: str) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for name, pattern in ERROR.items():
        match = pattern.search(message)
        result[name] = int(match.group(1)) if name in {"record", "bit"} and match else (
            int(match.group(1), 16) if name in {"mask", "header"} and match else
            match.group(1) if match else None
        )
    return result


def load_trace(root: Path, frame: dict[str, Any]) -> list[dict[str, Any]]:
    name = frame.get("decoded_value_trace_file")
    if not name:
        return []
    path = root / name
    return json.loads(path.read_text()) if path.is_file() else []


def analyze(manifest_path: Path, failures_path: Path) -> dict[str, Any]:
    root = manifest_path.parent
    manifest = json.loads(manifest_path.read_text())
    by_file = {frame.get("payload_file"): frame for frame in manifest}
    failures = json.loads(failures_path.read_text())
    rows: list[dict[str, Any]] = []
    for failure in failures:
        frame = by_file.get(failure.get("file"))
        if not frame or frame.get("decoded_value_residue_bit_offset") != 5:
            continue
        payload = root / failure["file"]
        parsed = parse_error(str(failure.get("error", "")))
        rows.append(
            {
                "file": failure["file"],
                "sha256": hashlib.sha256(payload.read_bytes()).hexdigest(),
                "payload_len": frame.get("payload_len"),
                "delta_index_count": frame.get("delta_index_count"),
                "completed_prefix_bits": frame.get("decoded_value_completed_prefix_bits"),
                "residue_bit_offset": frame.get("decoded_value_residue_bit_offset"),
                "error": parsed,
            }
        )

    clean_controls: dict[str, int] = {}
    bit5_controls: dict[str, int] = {}
    for frame in manifest:
        if frame.get("decoded_value_error") is not None:
            continue
        for record in load_trace(root, frame):
            key = f"0x{int(record['mask']):x}/0x{int(record['header']):x}/{record['baseline_mode']}"
            clean_controls[key] = clean_controls.get(key, 0) + 1
            if int(record.get("record_start", 0)) % 8 == 5:
                bit5_controls[key] = bit5_controls.get(key, 0) + 1

    for row in rows:
        error = row["error"]
        key = f"0x{error['mask']:x}/0x{error['header']:x}/{error['baseline_mode']}"
        row["clean_same_shape"] = clean_controls.get(key, 0)
        row["clean_same_shape_bit5"] = bit5_controls.get(key, 0)
    return {
        "schema": "quoteNetzipRs.official_5188_residue_controls.v1",
        "manifest": str(manifest_path),
        "failures_total": len(failures),
        "residue_offset_5_failures": rows,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("failures", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    output = json.dumps(analyze(args.manifest, args.failures), indent=2) + "\n"
    if args.output:
        args.output.write_text(output)
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
