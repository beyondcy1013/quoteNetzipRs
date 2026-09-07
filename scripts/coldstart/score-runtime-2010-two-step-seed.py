#!/usr/bin/env python3
"""Read-only check of runtime 20:10 two-step seeds vs stale-table poison.

Runtime now maps (market, index)->code then (market, code)->full 0104 row.
Both maps are filled from the SAME code_tables slice, last insert wins.
project() is still not called.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2010-two-step-seed.py EXTRACT CALLBACK META_0903 MORNING_0104
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
ident = SourceFileLoader(
    "disambiguate_586_batch_identity",
    os.path.join(_DIR, "disambiguate-586-batch-identity.py"),
).load_module()
proj = SourceFileLoader(
    "score_project_needs_0104_row",
    os.path.join(_DIR, "score-project-needs-0104-row.py"),
).load_module()


def rec_code(rec: bytes) -> str:
    raw = rec[0xE5:0xEB]
    if len(raw) == 6 and all(48 <= b <= 57 for b in raw):
        return raw.decode("ascii")
    return ""


def star(code: str) -> bool:
    return code.startswith(("688", "689"))


def load_all_tables(directory: str) -> list[tuple[str, str, dict]]:
    """(filename, market, table) in directory listing order."""
    out = []
    for name in sorted(os.listdir(directory)):
        if not name.endswith("0104.code-table.json"):
            continue
        table = json.loads(open(os.path.join(directory, name), "rb").read())
        market = bytes(table["market"]).decode()
        out.append((name, market, table))
    return out


def simulate_runtime_maps(tables: list[tuple[str, str, dict]]):
    symbol_codes = {}
    seeds = {}
    index_conflict = 0
    code_conflict = 0
    index_samples = []
    code_samples = []
    for _, market, table in tables:
        for rec in table["records"]:
            code = rec.get("code") or ""
            if not code:
                continue
            idx = rec["symbol_index"]
            prev_code = symbol_codes.get((market, idx))
            if prev_code is not None and prev_code != code:
                index_conflict += 1
                if len(index_samples) < 6:
                    index_samples.append(
                        {
                            "market": market,
                            "index": idx,
                            "was": prev_code,
                            "now": code,
                        }
                    )
            symbol_codes[(market, idx)] = code
            tail = bytes(rec["opaque_tail"])
            last = seed.i32(tail, 11) if len(tail) >= 15 else 0
            prev = seeds.get((market, code))
            if prev is not None and prev["last"] != last:
                code_conflict += 1
                if len(code_samples) < 6:
                    code_samples.append(
                        {
                            "market": market,
                            "code": code,
                            "was": prev["last"],
                            "now": last,
                        }
                    )
            places = tail[0] if tail else 0
            seeds[(market, code)] = {
                "last": last,
                "places": places,
                "scale": float(10**places) if 0 < places <= 6 else 0.0,
                "name": rec.get("name") or "",
                "index": idx,
            }
    return symbol_codes, seeds, {
        "index_code_conflicts": index_conflict,
        "code_last_conflicts": code_conflict,
        "symbol_codes": len(symbol_codes),
        "seed_rows": len(seeds),
        "samples_index": index_samples,
        "samples_code": code_samples,
    }


def pick_later(cands):
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2010-two-step-seed.py EXTRACT CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    meta = seed.load_index_metadata(meta_0903)
    morning_by_code = proj.load_largest_0104_full(morning_dir)
    tables = load_all_tables(morning_dir)
    symbol_codes, seeds, census = simulate_runtime_maps(tables)

    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    rows, _ = seed.load_decoded(extract_dir)
    hits = Counter()
    star_cross = []
    samples_stale = []
    for row in rows:
        ident_row = meta.get((row["market"], row["index"]))
        if ident_row is None:
            continue
        true_code = ident_row["code"]
        cands = cb_by.get((row["market"], true_code, row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        picked = pick_later(cands)
        # Path A: runtime 20:10 with ONLY the morning tables (stale vs this extract)
        mapped = symbol_codes.get((row["market"], row["index"]))
        if mapped is None:
            hits["two_step_missing"] += 1
        else:
            meta_row = seeds.get((row["market"], mapped))
            if meta_row is None or meta_row["scale"] <= 0:
                hits["two_step_no_scale"] += 1
            else:
                projected = seed.rust_scaled(meta_row["last"], meta_row["scale"])
                if seed.same_f32(projected, picked["last_close"]):
                    hits["two_step_stale_table_last"] += 1
                elif len(samples_stale) < 6:
                    samples_stale.append(
                        {
                            "index": row["index"],
                            "true": true_code,
                            "mapped": mapped,
                            "mapped_last": projected,
                            "oem": picked["last_close"],
                        }
                    )
                if mapped == true_code:
                    hits["two_step_mapped_eq_true"] += 1
                if star(mapped) != star(true_code):
                    hits["star_prefix_cross"] += 1
                    if len(star_cross) < 6:
                        star_cross.append(
                            {"index": row["index"], "true": true_code, "mapped": mapped}
                        )
        # Path B: identity 09-03 + morning last by CODE
        morn = morning_by_code.get((row["market"], true_code))
        if morn and morn["scale"] > 0:
            good = seed.rust_scaled(morn["last_0104"], morn["scale"])
            if seed.same_f32(good, picked["last_close"]):
                hits["identity_plus_morning_by_code"] += 1
        wire = rec_code(row["rec"])
        if wire and star(wire) != star(true_code):
            hits["wire_star_cross"] += 1

    n = hits["joined"] or 1
    empty_decoded = 0
    for row in rows:
        empty_decoded += 1
    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2010_two_step_seed.v1",
        "runtime_mtime": "2026-09-04 20:10",
        "crate_helpers": {
            "OemState.from_code_table_metadata": True,
            "OemState.project_from_code_table_metadata": True,
            "runtime_calls_project": False,
        },
        "morning_all_tables": {
            "files": len(tables),
            **census,
        },
        "auction_exact_second": {
            "joined": hits["joined"],
            "two_step_with_stale_morning_table": hits["two_step_stale_table_last"],
            "two_step_mapped_code_eq_true": hits["two_step_mapped_eq_true"],
            "two_step_missing_index": hits["two_step_missing"],
            "identity_0903_plus_morning_by_code": hits["identity_plus_morning_by_code"],
            "star_prefix_cross_mapped_vs_true": hits["star_prefix_cross"],
            "wire_0xe5_star_cross_vs_true": hits["wire_star_cross"],
            "rates": {
                "stale_two_step": round(hits["two_step_stale_table_last"] / n, 6),
                "correct_two_map": round(hits["identity_plus_morning_by_code"] / n, 6),
            },
            "samples_stale": samples_stale,
            "samples_star_cross": star_cross,
        },
        "decoded_records_in_extract": empty_decoded,
        "note": (
            "20:10 two-step with a single stale 0104 is the same poison as "
            "index-only: mapped code is the stale table's code at that index. "
            "Correct path is identity from the decode session plus last/scale/name "
            "from the same-session 0104 by (market, code). Runtime still never "
            "calls project_from_code_table_metadata."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
