#!/usr/bin/env python3
"""Audit 2a10 membership against a caller-supplied redacted receive list.

The Wine receive-list file is vendor-serialized and is intentionally not
parsed here. Callers provide a UTF-8/UTF-16 text list of codes and a sanitized
0104 mapping (market<TAB>ordinal<TAB>code). Only counts, hashes and partition
sizes are emitted; entry payload bytes are never printed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path


def read_text(path: Path) -> set[str]:
    data = path.read_bytes()
    for encoding in ("utf-8-sig", "utf-16", "utf-16-le"):
        try:
            text = data.decode(encoding)
            break
        except UnicodeDecodeError:
            continue
    else:
        raise ValueError(f"cannot decode receive list: {path}")
    return {line.strip().upper() for line in text.splitlines() if line.strip()}


def read_mapping(path: Path) -> dict[tuple[str, int], str]:
    mapping: dict[tuple[str, int], str] = {}
    for line_no, line in enumerate(path.read_text(encoding="utf-8-sig").splitlines(), 1):
        fields = line.strip().split("\t")
        if len(fields) != 3:
            raise ValueError(f"mapping line {line_no} must be market<TAB>ordinal<TAB>code")
        market, ordinal, code = fields
        key = (market.upper(), int(ordinal, 0))
        if key in mapping:
            raise ValueError(f"duplicate mapping key at line {line_no}")
        mapping[key] = code.strip().upper()
    return mapping


def read_entries(directory: Path) -> list[list[tuple[str, int]]]:
    partitions: list[list[tuple[str, int]]] = []
    for path in sorted(directory.glob("*_2a10_*.bin")):
        raw = path.read_bytes()
        if len(raw) < 18:
            continue
        kind, payload_len = struct.unpack_from("<HH", raw, 0)
        if kind != 0x102A or payload_len + 8 > len(raw):
            continue
        payload = raw[8 : 8 + payload_len]
        if len(payload) < 10 or (len(payload) - 10) % 6:
            continue
        entries = []
        for offset in range(10, len(payload), 6):
            market = payload[offset : offset + 2].decode("ascii", "replace").upper()
            ordinal = struct.unpack_from("<I", payload, offset + 2)[0] & 0xFFFF
            entries.append((market, ordinal))
        partitions.append(entries)
    return partitions


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("frames", type=Path)
    parser.add_argument("--mapping", required=True, type=Path)
    parser.add_argument("--receive-list", required=True, type=Path)
    args = parser.parse_args()
    mapping = read_mapping(args.mapping)
    receive = read_text(args.receive_list)
    partitions = read_entries(args.frames)
    if not partitions:
        raise SystemExit("no valid 2a10 frames")
    report = []
    for index, entries in enumerate(partitions, 1):
        codes = [mapping.get(entry) for entry in entries]
        missing_mapping = sum(code is None for code in codes)
        matched = sum(code in receive for code in codes if code is not None)
        digest = hashlib.sha256(",".join(f"{m}:{i}" for m, i in entries).encode()).hexdigest()
        report.append({
            "partition": index,
            "entry_count": len(entries),
            "mapping_missing": missing_mapping,
            "receive_list_matches": matched,
            "entry_sha256": digest,
        })
    print(json.dumps({
        "schema": "quoteNetzipRs.official_5188_receive_list_audit.v1",
        "frame_count": len(partitions),
        "receive_list_size": len(receive),
        "partitions": report,
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
