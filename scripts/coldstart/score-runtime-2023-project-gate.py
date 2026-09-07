#!/usr/bin/env python3
"""Runtime 20:23 calls project() as an is_err gate and discards the quote.

Schema-valid stale 0104 rows still project Ok with the wrong ticker.
Negative-amount records pass the pre-merge seed project, get merged, then
fail the post-merge project without un-merge. Snapshot still has no
PublicQuote.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-project-gate.py EXTRACT CALLBACK META_0903 MORNING_0104 \\
    NIGHT_PROBE NIGHT_0104
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
two = SourceFileLoader(
    "score_runtime_2010_two_step_seed",
    os.path.join(_DIR, "score-runtime-2010-two-step-seed.py"),
).load_module()


def six_digit(code: str) -> bool:
    return len(code) == 6 and code.isdigit()


def probe_bytes(row: dict) -> bytes | None:
    raw_hex = row.get("record_hex") or ""
    if len(raw_hex) < 0x137 * 2:
        return None
    return bytes.fromhex(raw_hex[: 0x137 * 2])


def project_schema_ok(mapped_row: dict, rec: bytes) -> tuple[bool, str]:
    if mapped_row.get("scale", 0) <= 0:
        return False, "bad_scale"
    if not six_digit(mapped_row.get("code") or ""):
        return False, "code_not_six_digit"
    amount = seed.i64(rec, 0x1C)
    if amount < 0:
        return False, "negative_amount"
    return True, "ok"


def main() -> int:
    if len(sys.argv) != 7:
        print(
            "usage: score-runtime-2023-project-gate.py EXTRACT CALLBACK "
            "META_0903 MORNING_0104 NIGHT_PROBE NIGHT_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir, night_probe, night_0104 = (
        sys.argv[1:]
    )
    meta = seed.load_index_metadata(meta_0903)
    tables = two.load_all_tables(morning_dir)
    symbol_codes, seeds, _ = two.simulate_runtime_maps(tables)
    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    rows, _ = seed.load_decoded(extract_dir)
    hits = Counter()
    schema_fail = Counter()
    unique_ok_wrong = set()
    unique_ok_right = set()
    unique_schema_err = set()
    samples_ok_wrong = []
    samples_schema_fail = []
    for row in rows:
        ident_row = meta.get((row["market"], row["index"]))
        if ident_row is None:
            continue
        true_code = ident_row["code"]
        cands = cb_by.get((row["market"], true_code, row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        mapped_code = symbol_codes.get((row["market"], row["index"]))
        if mapped_code is None:
            hits["gate_missing_index"] += 1
            continue
        mapped_row = dict(seeds.get((row["market"], mapped_code)) or {})
        mapped_row["code"] = mapped_code
        ok, why = project_schema_ok(mapped_row, row["rec"])
        right = mapped_code == true_code
        if not ok:
            hits["gate_schema_err"] += 1
            schema_fail[why] += 1
            unique_schema_err.add(row["index"])
            if right:
                hits["schema_err_was_right_ticker"] += 1
            else:
                hits["schema_err_was_wrong_ticker"] += 1
            if why == "negative_amount":
                hits["dirty_merge_then_project_err"] += 1
            if len(samples_schema_fail) < 4:
                samples_schema_fail.append(
                    {
                        "index": row["index"],
                        "mapped": mapped_code,
                        "true": true_code,
                        "why": why,
                    }
                )
            continue
        hits["gate_project_ok"] += 1
        if right:
            hits["ok_and_right_ticker"] += 1
            unique_ok_right.add(row["index"])
        else:
            hits["ok_but_wrong_ticker"] += 1
            unique_ok_wrong.add(row["index"])
            if len(samples_ok_wrong) < 6:
                samples_ok_wrong.append(
                    {
                        "index": row["index"],
                        "mapped": mapped_code,
                        "true": true_code,
                    }
                )

    night_tables = two.load_all_tables(night_0104)
    night_codes, night_seeds, _ = two.simulate_runtime_maps(night_tables)
    night = Counter()
    night_fail = Counter()
    for row in json.loads(open(night_probe, "rb").read()):
        rec = probe_bytes(row)
        market = row.get("market") or ""
        true_code = row.get("code") or ""
        idx = int(row.get("idx") or -1)
        if rec is None or not true_code:
            continue
        night["probe"] += 1
        mapped_code = night_codes.get((market, idx))
        if mapped_code is None:
            night["missing_index"] += 1
            continue
        mapped_row = dict(night_seeds.get((market, mapped_code)) or {})
        mapped_row["code"] = mapped_code
        ok, why = project_schema_ok(mapped_row, rec)
        if not ok:
            night["schema_err"] += 1
            night_fail[why] += 1
            continue
        night["schema_ok"] += 1
        if mapped_code == true_code:
            night["ok_and_right"] += 1
        else:
            night["ok_but_wrong"] += 1

    n = hits["joined"] or 1
    p = night["probe"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_project_gate.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "facts": {
            "calls_project_from_code_table_metadata": True,
            "discards_PublicQuote": True,
            "snapshot_has_no_quotes": True,
            "post_merge_project_err_does_not_unmerge": True,
            "schema_gate_not_session_gate": True,
        },
        "compile_202434_claim": (
            "Main session said merge happens only when the same-session 0104 "
            "row can construct and project. project().is_err is six-digit/"
            "scale/amount schema, not session identity."
        ),
        "night_same_session": {
            "probe_rows": night["probe"],
            "schema_ok": night["schema_ok"],
            "schema_err": night["schema_err"],
            "schema_err_kinds": dict(night_fail),
            "ok_and_right_ticker": night["ok_and_right"],
            "ok_but_wrong_ticker": night["ok_but_wrong"],
            "missing_index": night["missing_index"],
            "rates": {
                "schema_ok": round(night["schema_ok"] / p, 6),
                "right_ticker_among_ok": round(
                    night["ok_and_right"] / (night["schema_ok"] or 1), 6
                ),
            },
        },
        "auction_exact_second": {
            "joined": hits["joined"],
            "project_schema_ok": hits["gate_project_ok"],
            "project_schema_err": hits["gate_schema_err"],
            "schema_err_kinds": dict(schema_fail),
            "schema_err_was_right_ticker": hits["schema_err_was_right_ticker"],
            "schema_err_was_wrong_ticker": hits["schema_err_was_wrong_ticker"],
            "dirty_merge_then_project_err": hits["dirty_merge_then_project_err"],
            "ok_and_right_ticker": hits["ok_and_right_ticker"],
            "ok_but_wrong_ticker": hits["ok_but_wrong_ticker"],
            "unique_index_ok_right": len(unique_ok_right),
            "unique_index_ok_wrong": len(unique_ok_wrong),
            "unique_index_schema_err": len(unique_schema_err),
            "rates": {
                "schema_ok": round(hits["gate_project_ok"] / n, 6),
                "wrong_ticker_among_ok": round(
                    hits["ok_but_wrong_ticker"] / (hits["gate_project_ok"] or 1), 6
                ),
                "wrong_ticker_among_joins": round(hits["ok_but_wrong_ticker"] / n, 6),
                "false_reject_of_right_ticker": round(
                    hits["schema_err_was_right_ticker"] / n, 6
                ),
            },
            "samples_ok_wrong_ticker": samples_ok_wrong,
            "samples_schema_fail": samples_schema_fail,
        },
        "product_wiring": [
            "20:23 project() is a six-digit/scale/amount schema check. Stale 0104 A-share rows pass.",
            "The returned Official5188PublicQuote is discarded; ShadowSnapshot still has no quotes.",
            "Negative-amount 2704 rows pass the seed-state project, merge into oem_states, then fail the post-merge project without rollback.",
            "Do not read missing==0 or project-Ok as proof that the ticker is from the decode-session 0104.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
