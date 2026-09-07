#!/usr/bin/env python3
"""Did merge drop a good live bid1, or did the wine-tail 2704 already have leftover book?

Section 61/62: 5 first-prints match close but persist bid1 is leftover; night
dump bid1 at +0x68 is 5,166/5,166. This walks each incoming 2704 for those 5
indexes and compares incoming bid1 vs persist after sparse merge vs last OEM.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-first-print-incoming-bid1.py EXTRACT META_0903 MORNING_0104 FIRST_PRINT_JSON
"""
from __future__ import annotations

import json
import os
import sys

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()
two = SourceFileLoader(
    "score_runtime_2010_two_step_seed",
    os.path.join(_DIR, "score-runtime-2010-two-step-seed.py"),
).load_module()
rb = SourceFileLoader(
    "score_runtime_2023_rollback_vs_poison",
    os.path.join(_DIR, "score-runtime-2023-rollback-vs-poison.py"),
).load_module()
fp = SourceFileLoader(
    "score_runtime_2023_first_print_fields",
    os.path.join(_DIR, "score-runtime-2023-first-print-fields.py"),
).load_module()


def bid1_i(rec: bytes) -> int:
    return seed.i32(rec, 0x68)


def classify_incoming(incoming_f, oem_bid, persist_f, first_f) -> str:
    if seed.same_f32(incoming_f, oem_bid):
        if seed.same_f32(persist_f, oem_bid):
            return "live_incoming_and_persist_eq_oem"
        return "live_incoming_eq_oem_but_persist_miss"
    if incoming_f == 0:
        if seed.same_f32(persist_f, first_f) and not seed.same_f32(first_f, oem_bid):
            return "live_incoming_bid1_zero_kept_first"
        return "live_incoming_bid1_zero"
    if seed.same_f32(incoming_f, persist_f):
        return "live_incoming_same_as_persist_not_oem"
    return "live_incoming_other_nonzero"


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-first-print-incoming-bid1.py EXTRACT "
            "META_0903 MORNING_0104 FIRST_PRINT_JSON",
            file=sys.stderr,
        )
        return 2
    extract_dir, meta_0903, morning_dir, first_json = sys.argv[1:]
    samples = json.loads(open(first_json, "rb").read()).get("samples_first_print") or []
    want = {(s["market"], s["index"]): s for s in samples}
    identity = seed.load_index_metadata(meta_0903)
    _, morning_seeds, _ = two.simulate_runtime_maps(two.load_all_tables(morning_dir))
    rows, _ = seed.load_decoded(extract_dir)

    states = {}
    traces = {key: [] for key in want}
    for row in rows:
        key = (row["market"], row["index"])
        if key not in want:
            continue
        rec = row["rec"]
        mask = int(row["mask"] or 0)
        if key not in states:
            states[key] = bytearray(rb.INTERNAL)
        prev_bid = bid1_i(bytes(states[key]))
        rb.merge_into(states[key], rec, mask)
        ident_row = identity.get(key) or {}
        code = ident_row.get("code") or want[key]["code"]
        meta = dict(morning_seeds.get((key[0], code)) or {})
        meta["code"] = code
        scale = meta.get("scale") or 1.0
        traces[key].append(
            {
                "mask": mask,
                "ts_only": mask & 0x38 == 0x18,
                "incoming_close": seed.rust_scaled(seed.i32(rec, 0x10), scale),
                "incoming_bid1": seed.rust_scaled(bid1_i(rec), scale),
                "incoming_bid1_raw": bid1_i(rec),
                "incoming_bid1_nonzero_slot": any(rec[0x68:0x6C]),
                "persist_bid1_before": seed.rust_scaled(prev_bid, scale),
                "persist_bid1_after": seed.rust_scaled(bid1_i(bytes(states[key])), scale),
            }
        )

    rows_out = []
    buckets = {}
    for key, sample in want.items():
        steps = traces.get(key) or []
        oem_bid = float(sample["oem"]["bid1"])
        persist_f = seed.rust_scaled(bid1_i(bytes(states.get(key) or rb.INTERNAL)), sample_scale(sample, morning_seeds, identity, key))
        first_f = steps[0]["incoming_bid1"] if steps else 0.0
        live = None
        for step in steps:
            if step["incoming_close"] != 0:
                live = step
        label = "no_live_incoming"
        if live is not None:
            label = classify_incoming(
                live["incoming_bid1"], oem_bid, persist_f, first_f
            )
        buckets[label] = buckets.get(label, 0) + 1
        rows_out.append(
            {
                "market": key[0],
                "index": key[1],
                "code": sample["code"],
                "oem_bid1": oem_bid,
                "persist_bid1": persist_f,
                "n_records": len(steps),
                "verdict": label,
                "steps": steps,
            }
        )

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_first_print_incoming_bid1.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "n": len(rows_out),
        "buckets": buckets,
        "rows": rows_out,
        "product_wiring": [
            "Sparse merge copies a book slot only when the incoming 4 bytes are nonzero.",
            "If the live 2704 bid1 is zero or leftover, persist keeps the first-frame book.",
            "Do not treat leftover wine-tail bid1 as a +0x68 getter defect; night dump is 5166/5166.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


def sample_scale(sample, morning_seeds, identity, key):
    ident_row = identity.get(key) or {}
    code = ident_row.get("code") or sample["code"]
    meta = morning_seeds.get((key[0], code)) or {}
    return meta.get("scale") or 1.0


if __name__ == "__main__":
    raise SystemExit(main())
