#!/usr/bin/env python3
"""0xe5 vs seed-map consistency as a same-session gate, not a join key.

Night same-session 311B keeps 0104 code at 0xe5. Auction wine-tail 0xe5 is a
third map. Asking whether 0xe5 == symbol_codes[index] can reject a stale 0104
without using the lab 09-03 identity. Residual poison is when they agree and
the ticker is still wrong.

Does not edit official_5188.rs or runtime.

usage:
  score-0xe5-seed-map-gate.py NIGHT_PROBE NIGHT_0104 EXTRACT CALLBACK META_0903 MORNING_0104
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
proj = SourceFileLoader(
    "score_project_needs_0104_row",
    os.path.join(_DIR, "score-project-needs-0104-row.py"),
).load_module()


def rec_code(rec: bytes) -> str:
    raw = rec[0xE5:0xEB]
    if len(raw) == 6 and all(48 <= b <= 57 for b in raw):
        return raw.decode("ascii")
    return ""


def probe_bytes(row: dict) -> bytes | None:
    raw_hex = row.get("record_hex") or ""
    if len(raw_hex) < 0x137 * 2:
        return None
    return bytes.fromhex(raw_hex[: 0x137 * 2])


def index_map(by_code: dict) -> dict:
    out = {}
    for (market, code), row in by_code.items():
        out[(market, row["index"])] = code
    return out


def pick_later(cands):
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def main() -> int:
    if len(sys.argv) != 7:
        print(
            "usage: score-0xe5-seed-map-gate.py NIGHT_PROBE NIGHT_0104 EXTRACT "
            "CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    night_probe, night_0104, extract_dir, callback_jsonl, meta_0903, morning_dir = (
        sys.argv[1:]
    )
    night_by_code = proj.load_largest_0104_full(night_0104)
    night_by_index = index_map(night_by_code)
    night = Counter()
    for row in json.loads(open(night_probe, "rb").read()):
        code = row.get("code") or ""
        market = row.get("market") or ""
        rec = probe_bytes(row)
        if not code or rec is None:
            continue
        mapped = night_by_index.get((market, int(row.get("idx") or -1)))
        wire = rec_code(rec)
        night["probe"] += 1
        if mapped is None:
            night["no_0104"] += 1
            continue
        if not wire:
            night["wire_empty"] += 1
            continue
        if wire == mapped:
            night["wire_eq_mapped"] += 1
            if wire == code:
                night["eq_and_true"] += 1
            else:
                night["eq_but_wrong_true"] += 1
        else:
            night["wire_ne_mapped"] += 1

    meta = seed.load_index_metadata(meta_0903)
    tables = two.load_all_tables(morning_dir)
    symbol_codes, _, _ = two.simulate_runtime_maps(tables)
    cb_by = defaultdict(list)
    for q in ident.stream_callback_quotes(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    rows, _ = seed.load_decoded(extract_dir)
    auc = Counter()
    samples_residual = []
    samples_caught = []
    for row in rows:
        ident_row = meta.get((row["market"], row["index"]))
        if ident_row is None:
            continue
        true_code = ident_row["code"]
        cands = cb_by.get((row["market"], true_code, row["ts"]), [])
        if not cands:
            continue
        auc["joined"] += 1
        mapped = symbol_codes.get((row["market"], row["index"]))
        wire = rec_code(row["rec"])
        if mapped is None:
            auc["no_mapped"] += 1
            continue
        disagree = bool(wire) and wire != mapped
        empty = not wire
        if disagree:
            auc["gate_reject"] += 1
            if mapped != true_code:
                auc["gate_reject_was_wrong"] += 1
            else:
                auc["gate_reject_was_right"] += 1
            if len(samples_caught) < 4:
                samples_caught.append(
                    {
                        "index": row["index"],
                        "wire": wire,
                        "mapped": mapped,
                        "true": true_code,
                    }
                )
            continue
        auc["gate_pass"] += 1
        if empty:
            auc["pass_empty_wire"] += 1
        if mapped == true_code:
            auc["pass_and_right"] += 1
        else:
            auc["pass_but_wrong"] += 1
            if len(samples_residual) < 6:
                samples_residual.append(
                    {
                        "index": row["index"],
                        "wire": wire or "(empty)",
                        "mapped": mapped,
                        "true": true_code,
                    }
                )

    n = auc["joined"] or 1
    p = night["probe"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_0xe5_seed_map_gate.v1",
        "runtime_note": (
            "20:18 reseed_code_tables replaces the decoder in a unit test only; "
            "ShadowReader has no reseed API and still does not compare 0xe5."
        ),
        "gate": "if 0xe5 is six-digit and != symbol_codes[index]: not-ready; else merge",
        "not_a_join_key": True,
        "night_same_session": {
            "probe_rows": night["probe"],
            "wire_eq_mapped": night["wire_eq_mapped"],
            "wire_ne_mapped": night["wire_ne_mapped"],
            "eq_but_wrong_true": night["eq_but_wrong_true"],
            "false_reject": night["wire_ne_mapped"],
            "residual_poison": night["eq_but_wrong_true"],
        },
        "auction_stale_0915_on_wine_tail": {
            "joined": auc["joined"],
            "gate_reject": auc["gate_reject"],
            "gate_reject_was_wrong_ticker": auc["gate_reject_was_wrong"],
            "gate_reject_false_positive": auc["gate_reject_was_right"],
            "gate_pass": auc["gate_pass"],
            "pass_and_right": auc["pass_and_right"],
            "pass_but_wrong_residual_poison": auc["pass_but_wrong"],
            "pass_empty_wire": auc["pass_empty_wire"],
            "rates": {
                "reject": round(auc["gate_reject"] / n, 6),
                "residual_poison_among_joins": round(auc["pass_but_wrong"] / n, 6),
                "residual_among_passes": round(
                    auc["pass_but_wrong"] / (auc["gate_pass"] or 1), 6
                ),
            },
            "samples_residual_poison": samples_residual,
            "samples_caught": samples_caught,
        },
        "product_wiring": [
            "Do not join OEM callbacks on 0xe5.",
            "0xe5 == current-session symbol_codes[index] is a same-session check: night false-reject 0, residual poison 0.",
            "On a stale 0104 it still lets through rows where the leftover 0xe5 happens to equal the stale map.",
            "reseed_code_tables is private and unused by ShadowReader; tearing down the reader is the current live path.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
