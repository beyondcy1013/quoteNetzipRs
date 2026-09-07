#!/usr/bin/env python3
"""Audit redacted 2a10-to-2704 flow associations from frame manifests."""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path


def reverse_flow(flow: str) -> str:
    left, right = flow.split("->", 1)
    return f"{right}->{left}"


def audit(client_path: Path, server_path: Path) -> dict[str, object]:
    clients = json.loads(client_path.read_text(encoding="utf-8"))
    servers = json.loads(server_path.read_text(encoding="utf-8"))
    server_by_flow: dict[str, list[dict[str, object]]] = {}
    for frame in servers:
        server_by_flow.setdefault(str(frame["flow"]), []).append(frame)
    rows = []
    unmatched = 0
    for frame in clients:
        if frame.get("wire_kind") != "2a10":
            continue
        flow = reverse_flow(str(frame["flow"]))
        related = [item for item in server_by_flow.get(flow, []) if item.get("wire_kind") == "2704"]
        if not related:
            unmatched += 1
        rows.append(
            {
                "subscription_payload_len": frame.get("payload_len"),
                "server_2704_count": len(related),
                "server_2704_payload_lengths": sorted(
                    int(item["payload_len"]) for item in related
                ),
            }
        )
    return {
        "client_manifest": str(client_path),
        "server_manifest": str(server_path),
        "subscription_flow_count": len(rows),
        "unmatched_subscription_flows": unmatched,
        "rows": rows,
        "2704_count_distribution": dict(
            Counter(int(row["server_2704_count"]) for row in rows)
        ),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("client_manifest", type=Path)
    parser.add_argument("server_manifest", type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(args.client_manifest, args.server_manifest), indent=2))


if __name__ == "__main__":
    main()
