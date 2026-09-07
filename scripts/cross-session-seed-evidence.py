#!/usr/bin/env python3
"""Cross-session 0104 seed evidence for the two-map separation requirement.

Usage:
  cross-session-seed-evidence.py SESSION_A_EXTRACT_DIR SESSION_B_EXTRACT_DIR OUTPUT_JSON

Both inputs are official_5188_extract output directories containing
*-0104.code-table.json files (one per market per session). The experiment
quantifies, between two independent login lifecycles:

1. (market, symbol_index) -> code drift: how many table ordinals resolve to a
   different code in session B. Nonzero drift means a decode session's
   (market,index)->code map MUST come from the same session's 0104 and must be
   dropped at day-cut/reconnect/restart (gap: seed lifecycle).
2. (market, code) -> metadata stability: name / decimals / previous-close
   agreement for codes present in both sessions. High stability means the
   metadata map keyed by CODE can legitimately be rebuilt from whichever
   session's 0104 is current (gap: two-map separation).
3. previous-close tail coverage per session (seed availability).

No quote content beyond code/name/metadata comparisons is emitted.
"""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path


def load_tables(extract_dir: Path) -> dict:
    tables = {}
    for path in sorted(extract_dir.glob("*-0104.code-table.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        market = "".join(chr(c) for c in data["market"])
        rows = {}
        for record in data["records"]:
            tail = record["opaque_tail"]
            rows[record["symbol_index"]] = {
                "code": record["code"],
                "name": record["name"],
                "decimals": tail[0],
                "prev_close_raw": struct.unpack_from("<i", bytes(tail[11:15]), 0)[0]
                if len(tail) >= 15
                else None,
            }
        tables[market] = rows
    return tables


def main() -> None:
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    dir_a = Path(sys.argv[1])
    dir_b = Path(sys.argv[2])
    output = Path(sys.argv[3])

    tables_a = load_tables(dir_a)
    tables_b = load_tables(dir_b)

    result = {
        "schema": "netzip.cross-session-seed-evidence.v1",
        "session_a": str(dir_a),
        "session_b": str(dir_b),
        "markets": {},
    }

    for market in sorted(set(tables_a) & set(tables_b)):
        rows_a = tables_a[market]
        rows_b = tables_b[market]
        indices = sorted(set(rows_a) & set(rows_b))
        codes_a = {row["code"] for row in rows_a.values()}
        codes_b = {row["code"] for row in rows_b.values()}
        shared_codes = codes_a & codes_b

        index_drift = sum(
            1 for index in indices if rows_a[index]["code"] != rows_b[index]["code"]
        )
        # tail-growth: symbols present only in the later table
        b_only_codes = len(codes_b - codes_a)
        a_only_codes = len(codes_a - codes_b)

        name_match = decimals_match = prev_match = 0
        prev_change_examples = []
        # code->row maps for shared-code analysis
        by_code_a = {row["code"]: row for row in rows_a.values()}
        by_code_b = {row["code"]: row for row in rows_b.values()}
        for code in sorted(shared_codes):
            row_a = by_code_a[code]
            row_b = by_code_b[code]
            if row_a["name"] == row_b["name"]:
                name_match += 1
            if row_a["decimals"] == row_b["decimals"]:
                decimals_match += 1
            if row_a["prev_close_raw"] == row_b["prev_close_raw"]:
                prev_match += 1
            elif len(prev_change_examples) < 5:
                prev_change_examples.append(
                    {"code": code, "a": row_a["prev_close_raw"], "b": row_b["prev_close_raw"]}
                )

        nonzero_prev_a = sum(1 for row in rows_a.values() if row["prev_close_raw"])
        nonzero_prev_b = sum(1 for row in rows_b.values() if row["prev_close_raw"])

        result["markets"][market] = {
            "table_ordinals_a": len(rows_a),
            "table_ordinals_b": len(rows_b),
            "shared_ordinals": len(indices),
            "index_to_code_drift": index_drift,
            "index_to_code_drift_rate": round(index_drift / len(indices), 6)
            if indices
            else None,
            "codes_only_in_a": a_only_codes,
            "codes_only_in_b": b_only_codes,
            "shared_codes": len(shared_codes),
            "code_keyed_metadata": {
                "name_match": name_match,
                "decimals_match": decimals_match,
                "prev_close_match": prev_match,
                "prev_close_changed_examples": prev_change_examples,
            },
            "prev_close_tail_coverage": {
                "session_a": f"{nonzero_prev_a}/{len(rows_a)}",
                "session_b": f"{nonzero_prev_b}/{len(rows_b)}",
            },
        }

    output.write_text(json.dumps(result, indent=1), encoding="utf-8")
    for market, data in result["markets"].items():
        print(
            f"{market}: ordinals {data['table_ordinals_a']}->{data['table_ordinals_b']} "
            f"index_drift={data['index_to_code_drift']}/{data['shared_ordinals']} "
            f"codes +{data['codes_only_in_b']}/-{data['codes_only_in_a']} | "
            f"shared {data['shared_codes']}: name={data['code_keyed_metadata']['name_match']} "
            f"dec={data['code_keyed_metadata']['decimals_match']} "
            f"prev={data['code_keyed_metadata']['prev_close_match']}"
        )
    print(f"written: {output}")


if __name__ == "__main__":
    main()
