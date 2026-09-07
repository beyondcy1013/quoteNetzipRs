#!/usr/bin/env python3
"""Build a deterministic day-cut last_close seed report from 0104 tables."""

from __future__ import annotations

import argparse
import glob
import json
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("extract_dir", type=Path)
    parser.add_argument("report", type=Path)
    args = parser.parse_args()

    rows: dict[str, dict[str, int | str]] = {}
    files = sorted(glob.glob(str(args.extract_dir / "*-0104.code-table.json")))
    invalid = 0
    duplicate = 0
    for filename in files:
        payload = json.loads(Path(filename).read_text())
        market = bytes(payload["market"]).decode("ascii")
        for record in payload.get("records", []):
            tail = record.get("opaque_tail", [])
            if len(tail) != 23:
                invalid += 1
                continue
            code = str(record["code"])
            key = f"{market}:{code}"
            value = int.from_bytes(bytes(tail[11:15]), "little", signed=True)
            if key in rows and rows[key]["last_close_i32"] != value:
                duplicate += 1
                continue
            rows[key] = {
                "market": market,
                "code": code,
                "last_close_i32": value,
                "source": filename,
            }

    result = {
        "schema": "quoteNetzipRs.official_5188_last_close_seed.v1",
        "source_dir": str(args.extract_dir),
        "table_files": len(files),
        "seed_count": len(rows),
        "invalid_tail_count": invalid,
        "conflicting_duplicate_count": duplicate,
        "key": "market+code",
        "tail_offset": 11,
        "tail_width": 4,
        "seeds": sorted(rows.values(), key=lambda row: (row["market"], row["code"])),
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({k: result[k] for k in result if k != "seeds"}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
