#!/usr/bin/env python3
"""Flow-aware official-side 2a10 export for the synchronized dual-session capture.

Usage:
  export-official-2a10-sync.py EXTRACT_DIR OUTPUT_DIR

EXTRACT_DIR is the output of official_5188_extract over the merged capture
(contains manifest.json plus *-2a10.payload.bin files). The clone emits all
seven partitions on ONE uplink flow; the official client emits one partition
per socket across seven sockets. This tool groups 2a10 payload bins by client
endpoint, attributes the largest-group (seven-bin) endpoint as the clone, and
copies every other endpoint's bins into OUTPUT_DIR as the official side.

If no endpoint has >=5 bins (capture hole case), nothing is attributed and the
tool exits non-zero with a per-endpoint census so the caller can attribute
manually.
"""

from __future__ import annotations

import json
import shutil
import sys
from collections import Counter
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    extract_dir = Path(sys.argv[1])
    output_dir = Path(sys.argv[2])
    manifest = json.loads((extract_dir / "manifest.json").read_text(encoding="utf-8"))

    bins_by_client: dict[str, list[Path]] = {}
    for frame in manifest:
        if frame.get("wire_kind") != "2a10":
            continue
        payload_file = frame.get("payload_file")
        if not payload_file:
            continue
        src = f"{frame['src']}"
        bins_by_client.setdefault(src, []).append(extract_dir / payload_file)

    census = {src: len(paths) for src, paths in sorted(bins_by_client.items())}
    print("S\t2a10-bins-per-client\t" + json.dumps(census, sort_keys=True))
    if not bins_by_client:
        raise SystemExit("no 2a10 frames found in the manifest")

    ranked = sorted(
        ((len(paths), src, paths) for src, paths in bins_by_client.items()),
        reverse=True,
    )
    largest_count, clone_src, clone_bins = ranked[0]
    second_count = ranked[1][0] if len(ranked) > 1 else 0
    if largest_count != 7:
        print(
            "E\tclone-partition-count-mismatch\tclone candidate has "
            f"{largest_count} 2a10 bins, expected exactly 7 partitions "
            "(a capture hole degrades attribution); attribute manually"
        )
        raise SystemExit(2)
    if largest_count == second_count or second_count >= largest_count - 1:
        print(
            "E\tambiguous-attribution\tclone candidate is not uniquely dominant: "
            f"largest={largest_count} second={second_count}"
        )
        raise SystemExit(2)

    output_dir.mkdir(parents=True, exist_ok=True)
    copied = 0
    for src, paths in sorted(bins_by_client.items()):
        if src == clone_src:
            continue
        for index, path in enumerate(paths):
            target = output_dir / f"official-{src.split(':')[1]}-{index:02d}-2a10.payload.bin"
            shutil.copyfile(path, target)
            copied += 1
    print(
        f"S\tclone-flow\t{clone_src}\tbins={len(clone_bins)}\t"
        f"official-flows={len(bins_by_client) - 1}\tofficial-bins={copied}"
    )
    print(f"S\tofficial-export\t{output_dir}")
    if copied == 0:
        print(
            "W\tofficial-empty\tthe capture only contains the clone's 2a10; "
            "run against the merged dual-session capture"
        )


if __name__ == "__main__":
    main()
