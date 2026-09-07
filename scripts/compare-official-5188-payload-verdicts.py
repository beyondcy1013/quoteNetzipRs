#!/usr/bin/env python3
"""Compare 2704 payload verdicts across capture windows by exact payload bytes."""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
from pathlib import Path


def load_window(manifest_path: Path, start: str, end: str) -> dict[str, dict[str, object]]:
    manifest = json.loads(manifest_path.read_text())
    groups: dict[str, dict[str, object]] = {}
    for frame in manifest:
        if frame.get("wire_kind") != "2704":
            continue
        timestamp = str(frame["completed_at_micros"])
        clock = __import__("datetime").datetime.fromtimestamp(
            int(timestamp) / 1_000_000
        ).strftime("%H:%M:%S")
        if not (start <= clock < end):
            continue
        payload = manifest_path.parent / str(frame["payload_file"])
        digest = hashlib.sha256(payload.read_bytes()).hexdigest()
        group = groups.setdefault(
            digest,
            {
                "payload_len": int(frame["payload_len"]),
                "frames": 0,
                "clean": 0,
                "failed": 0,
                "stages": collections.Counter(),
            },
        )
        failed = frame.get("decoded_value_error") is not None
        group["frames"] += 1
        group["failed" if failed else "clean"] += 1
        if failed:
            text = str(frame["decoded_value_error"])
            marker = "stage="
            stage = text.split(marker, 1)[1].split()[0].rstrip(":") if marker in text else "unparsed"
            group["stages"][stage] += 1
    return groups


def load_shapes(manifest_path: Path, start: str, end: str) -> dict[tuple[int, int], dict[str, int]]:
    manifest = json.loads(manifest_path.read_text())
    shapes: dict[tuple[int, int], dict[str, int]] = {}
    for frame in manifest:
        if frame.get("wire_kind") != "2704":
            continue
        clock = __import__("datetime").datetime.fromtimestamp(
            int(frame["completed_at_micros"]) / 1_000_000
        ).strftime("%H:%M:%S")
        if not (start <= clock < end):
            continue
        shape = (int(frame["payload_len"]), int(frame.get("delta_index_count") or 0))
        counts = shapes.setdefault(
            shape, {"frames": 0, "clean": 0, "failed": 0, "stages": collections.Counter()}
        )
        counts["frames"] += 1
        error = frame.get("decoded_value_error")
        counts["failed" if error else "clean"] += 1
        if error:
            text = str(error)
            marker = "stage="
            stage = text.split(marker, 1)[1].split()[0].rstrip(":") if marker in text else "unparsed"
            counts["stages"][stage] += 1
    return shapes


def serialise_shapes(shapes: dict[tuple[int, int], dict[str, object]]) -> dict[str, object]:
    return {
        f"{shape[0]}:{shape[1]}": {
            **{key: value for key, value in counts.items() if key != "stages"},
            "stages": dict(counts["stages"]),
        }
        for shape, counts in shapes.items()
    }


def serialise(groups: dict[str, dict[str, object]]) -> dict[str, object]:
    return {
        digest: {
            **{key: value for key, value in group.items() if key != "stages"},
            "stages": dict(group["stages"]),
        }
        for digest, group in groups.items()
    }


def serialise_changes(changes: dict[str, dict[str, dict[str, object]]]) -> dict[str, object]:
    return {
        digest: {
            "pre": {
                **{key: value for key, value in entry["pre"].items() if key != "stages"},
                "stages": dict(entry["pre"]["stages"]),
            },
            "post": {
                **{key: value for key, value in entry["post"].items() if key != "stages"},
                "stages": dict(entry["post"]["stages"]),
            },
        }
        for digest, entry in changes.items()
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--pre-start", default="14:57:56")
    parser.add_argument("--pre-end", default="15:00:00")
    parser.add_argument("--post-start", default="15:00:00")
    parser.add_argument("--post-end", default="15:02:00")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    pre = load_window(args.manifest, args.pre_start, args.pre_end)
    post = load_window(args.manifest, args.post_start, args.post_end)
    pre_shapes = load_shapes(args.manifest, args.pre_start, args.pre_end)
    post_shapes = load_shapes(args.manifest, args.post_start, args.post_end)
    common = set(pre) & set(post)
    pre_mixed = {
        digest: pre[digest]
        for digest in pre
        if pre[digest]["clean"] > 0 and pre[digest]["failed"] > 0
    }
    post_mixed = {
        digest: post[digest]
        for digest in post
        if post[digest]["clean"] > 0 and post[digest]["failed"] > 0
    }
    verdict_changes = {
        digest: {"pre": pre[digest], "post": post[digest]}
        for digest in common
        if (pre[digest]["failed"] > 0) != (post[digest]["failed"] > 0)
    }
    report = {
        "schema": "quoteNetzipRs.official_5188_payload_verdicts.v1",
        "manifest": str(args.manifest),
        "windows": {
            "pre": {"start": args.pre_start, "end_exclusive": args.pre_end},
            "post": {"start": args.post_start, "end_exclusive": args.post_end},
        },
        "pre_unique_payloads": len(pre),
        "post_unique_payloads": len(post),
        "common_unique_payloads": len(common),
        "pre_mixed_verdict_payloads": len(pre_mixed),
        "post_mixed_verdict_payloads": len(post_mixed),
        "verdict_change_payloads": len(verdict_changes),
        "common_shape_count": len(set(pre_shapes) & set(post_shapes)),
        "shape_comparison": [
            {
                "payload_len": shape[0],
                "delta_index_count": shape[1],
                "pre": serialise_shapes({shape: pre_shapes[shape]})[f"{shape[0]}:{shape[1]}"],
                "post": serialise_shapes({shape: post_shapes[shape]})[f"{shape[0]}:{shape[1]}"],
            }
            for shape in sorted(set(pre_shapes) & set(post_shapes))
        ],
        "pre": serialise(pre),
        "post": serialise(post),
        "verdict_changes": serialise_changes(verdict_changes),
    }
    args.output.write_text(json.dumps(report, ensure_ascii=True, indent=2) + "\n")


if __name__ == "__main__":
    main()
