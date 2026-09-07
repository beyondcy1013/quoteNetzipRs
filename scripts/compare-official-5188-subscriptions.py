#!/usr/bin/env python3
"""Compare sanitized 2a10 subscription shapes across 5188 summary files."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


def subscription_signatures(path: Path) -> list[dict[str, Any]]:
    document = json.loads(path.read_text(encoding="utf-8"))
    signatures = []
    for flow in document.get("flows", []):
        if flow.get("subscription_frame_count", 0) == 0:
            continue
        signature = {
            "frame_count": flow["subscription_frame_count"],
            "entry_count": flow["subscription_entry_count"],
            "ranges": flow.get("subscription_ranges", []),
        }
        canonical = json.dumps(signature, sort_keys=True, separators=(",", ":")).encode()
        signature["shape_sha256"] = hashlib.sha256(canonical).hexdigest()
        signatures.append(signature)
    return sorted(signatures, key=lambda item: item["shape_sha256"])


def compare(paths: list[Path]) -> dict[str, Any]:
    sessions = [
        {"path": str(path), "subscriptions": subscription_signatures(path)} for path in paths
    ]
    baseline = sessions[0]["subscriptions"]
    for session in sessions:
        session["subscription_count"] = len(session["subscriptions"])
        session["entry_count"] = sum(
            item["entry_count"] for item in session["subscriptions"]
        )
        session["matches_baseline"] = session["subscriptions"] == baseline
    return {
        "schema": "quoteNetzipRs.official_5188_subscription_parity.v1",
        "comparison_ignores": ["src", "dst", "local_tcp_port", "capture_timestamp"],
        "all_match": all(session["matches_baseline"] for session in sessions),
        "sessions": sessions,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("summaries", nargs="+", type=Path)
    args = parser.parse_args()
    if len(args.summaries) < 2:
        parser.error("at least two summary files are required")
    print(json.dumps(compare(args.summaries), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
