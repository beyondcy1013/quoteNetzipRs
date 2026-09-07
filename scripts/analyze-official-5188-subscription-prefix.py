#!/usr/bin/env python3
"""Emit a redacted cross-session summary of official 2a10 subscription prefixes."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path


def summarize(directory: Path) -> dict[str, object]:
    frames = []
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
        prefix = bytearray(payload[:10])
        declared = struct.unpack_from("<I", prefix, 4)[0]
        entry_count = (len(payload) - 10) // 6
        prefix[4:8] = b"\0" * 4
        markets = []
        previous_market = None
        for offset in range(10, len(payload), 6):
            market = payload[offset : offset + 2].decode("ascii", "replace")
            if market != previous_market:
                markets.append(market)
                previous_market = market
        frames.append(
            {
                "payload_len": len(payload),
                "declared_count": declared,
                "entry_count": entry_count,
                "declared_count_matches_entries": declared == entry_count,
                "market_run_count": len(markets),
                "market_sequence_sha256": hashlib.sha256(
                    ",".join(markets).encode()
                ).hexdigest(),
                "normalized_prefix_sha256": hashlib.sha256(prefix).hexdigest(),
            }
        )
    topology = sorted(
        [
            (
                item["entry_count"],
                item["market_run_count"],
                item["market_sequence_sha256"],
            )
            for item in frames
        ]
    )
    return {
        "directory": str(directory),
        "frame_count": len(frames),
        "payload_lengths": sorted({item["payload_len"] for item in frames}),
        "declared_counts": sorted({item["declared_count"] for item in frames}),
        "normalized_prefix_sha256": sorted(
            {item["normalized_prefix_sha256"] for item in frames}
        ),
        "topology_signature": topology,
        "frames": frames,
    }


def validate(summary: dict[str, object]) -> None:
    frames = summary["frames"]
    if not isinstance(frames, list) or not frames:
        raise ValueError(f"no valid 2a10 frames in {summary['directory']}")
    if any(not frame["declared_count_matches_entries"] for frame in frames):
        raise ValueError(f"declared entry count mismatch in {summary['directory']}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("directories", nargs="+", type=Path)
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    summaries = [summarize(path) for path in args.directories]
    if args.strict:
        for summary in summaries:
            validate(summary)
    print(json.dumps(summaries, indent=2))


if __name__ == "__main__":
    main()
