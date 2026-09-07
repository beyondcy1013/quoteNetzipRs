#!/usr/bin/env python3
"""Runtime 20:14 seeds OemState.record from 0104 metadata before merge.

from_code_table_metadata copies code/tail into the 311B record. merge still
does not copy 0xe5/0x12b. If project() were turned on with the current
index→code map from a stale table, the published ticker would be wrong.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2014-metadata-seed.py EXTRACT CALLBACK META_0903 MORNING_0104
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
two = SourceFileLoader(
    "score_runtime_2010_two_step_seed",
    os.path.join(_DIR, "score-runtime-2010-two-step-seed.py"),
).load_module()


def star(code: str) -> bool:
    return code.startswith(("688", "689"))


def pick_later(cands):
    return max(cands, key=lambda q: (q["seq"], q["ts_ms"]))


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2014-metadata-seed.py EXTRACT CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    meta = seed.load_index_metadata(meta_0903)
    morning_by_code = proj.load_largest_0104_full(morning_dir)
    tables = two.load_all_tables(morning_dir)
    symbol_codes, seeds, census = two.simulate_runtime_maps(tables)

    oem_name = {}
    cb_by = defaultdict(list)
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
        seen = set()
        for line in fh:
            ev = json.loads(line)
            seq = ev.get("sequence")
            if seq in seen:
                continue
            seen.add(seq)
            batch = ev.get("quote_batch") or {}
            if batch.get("schema") != "quoteNetzipWine.quote_batch.v1":
                continue
            ts_ms = int(ev.get("timestamp_ms") or 0)
            for q in batch.get("quotes") or []:
                oem_name[(q["market"], q["code"])] = q.get("name") or ""
                cb_ts = seed.parse_callback_ts(q.get("datetime") or "")
                if cb_ts is None:
                    continue
                cb_by[(q["market"], q["code"], cb_ts)].append(
                    {
                        "seq": int(seq or 0),
                        "ts_ms": ts_ms,
                        "code": q["code"],
                        "name": q.get("name") or "",
                        "last_close": float(q.get("last_close") or 0),
                    }
                )

    rows, _ = seed.load_decoded(extract_dir)
    hits = Counter()
    unique_wrong = set()
    unique_ok = set()
    samples = []
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
        mapped = symbol_codes.get((row["market"], row["index"]))
        if mapped is None:
            hits["missing"] += 1
            continue
        mapped_row = seeds.get((row["market"], mapped))
        if mapped_row is None or mapped_row["scale"] <= 0:
            hits["no_scale"] += 1
            continue
        # 20:14: record 0x12b and previous_close both come from mapped 0104
        pub_last = seed.rust_scaled(mapped_row["last"], mapped_row["scale"])
        pub_code = mapped
        pub_name = mapped_row["name"]
        key = (row["market"], row["index"])
        if pub_code == picked["code"]:
            hits["pub_code_eq_oem"] += 1
            unique_ok.add(key)
        else:
            hits["pub_code_ne_oem"] += 1
            unique_wrong.add(key)
        if pub_name == picked["name"]:
            hits["pub_name_eq_oem"] += 1
        if seed.same_f32(pub_last, picked["last_close"]):
            hits["pub_last_eq_oem"] += 1
        if star(pub_code) != star(true_code):
            hits["star_cross"] += 1
        # correct two-map: true code's morning row
        good = morning_by_code.get((row["market"], true_code))
        if good and good["scale"] > 0:
            if seed.same_f32(
                seed.rust_scaled(good["last_0104"], good["scale"]),
                picked["last_close"],
            ):
                hits["correct_last"] += 1
            if (good.get("name") or "") == picked["name"] and true_code == picked["code"]:
                hits["correct_identity"] += 1
        if pub_code != picked["code"] and len(samples) < 6:
            samples.append(
                {
                    "index": row["index"],
                    "true": true_code,
                    "oem_name": picked["name"],
                    "published_code": pub_code,
                    "published_name": pub_name,
                    "published_last": pub_last,
                    "oem_last": picked["last_close"],
                }
            )

    n = hits["joined"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2014_metadata_seed.v1",
        "runtime_mtime": "2026-09-04 20:14",
        "crate_mtime": "2026-09-04 20:11",
        "facts": {
            "from_code_table_metadata_copies_code_and_tail_into_record": True,
            "merge_still_skips_0xe5_and_0x12b": True,
            "runtime_calls_from_code_table_metadata": True,
            "runtime_still_does_not_call_project": True,
            "without_project_record_0x12b_is_now_0104_last_not_zero": True,
        },
        "morning_maps": {
            "files": len(tables),
            "symbol_codes": census["symbol_codes"],
            "seed_rows": census["seed_rows"],
        },
        "auction_exact_second": {
            "joined": hits["joined"],
            "stale_two_step_published_code_eq_oem": hits["pub_code_eq_oem"],
            "stale_two_step_published_code_ne_oem": hits["pub_code_ne_oem"],
            "stale_two_step_published_name_eq_oem": hits["pub_name_eq_oem"],
            "stale_two_step_published_last_eq_oem": hits["pub_last_eq_oem"],
            "star_prefix_cross": hits["star_cross"],
            "correct_two_map_last": hits["correct_last"],
            "correct_two_map_code_and_name": hits["correct_identity"],
            "unique_indexes_wrong_ticker": len(unique_wrong),
            "unique_indexes_right_ticker": len(unique_ok),
            "rates": {
                "wrong_ticker": round(hits["pub_code_ne_oem"] / n, 6),
                "right_last_stale": round(hits["pub_last_eq_oem"] / n, 6),
                "right_last_correct_map": round(hits["correct_last"] / n, 6),
            },
            "samples_wrong_ticker": samples,
        },
        "product_wiring": [
            "20:14 fills OemState.record from the mapped 0104 row, so skipping project() no longer yields last_close=0.",
            "Turning on project_from_code_table_metadata with a stale index→code map would publish the wrong ticker, name, last_close, and STAR lots.",
            "Keep index→code bound to the decode session; look up last/scale/name by (market, true code).",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
