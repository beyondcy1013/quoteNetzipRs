#!/usr/bin/env python3
"""If ShadowSnapshot were wired to persist oem_states, what would go out?

Runtime 20:23 still discards PublicQuote. This scores the last persist
311B per index after the wine-tail: projectable (amount>=0) quotes using
the current index→code map, vs last Wine OEM callback.

Stale 09:15 map on the auction extract is what runtime would publish
today. Night same-session 0104 is the positive control.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-snapshot-would-publish.py \\
    EXTRACT CALLBACK META_0903 MORNING_0104 \\
    NIGHT_EXTRACT NIGHT_0104 NIGHT_CALLBACK
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter

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
rb = SourceFileLoader(
    "score_runtime_2023_rollback_vs_poison",
    os.path.join(_DIR, "score-runtime-2023-rollback-vs-poison.py"),
).load_module()


def last_oem(path: str, min_seq: int = 0) -> dict:
    out = {}
    for q in ident.stream_callback_quotes(path):
        if q["seq"] < min_seq:
            continue
        key = (q["market"], q["code"])
        prev = out.get(key)
        if prev is None or (q["seq"], q["ts_ms"]) >= (prev["seq"], prev["ts_ms"]):
            out[key] = q
    return out


def persist_states(rows, symbol_codes, seeds):
    states = {}
    mapped = {}
    for row in rows:
        key = (row["market"], row["index"])
        code = symbol_codes.get(key)
        if code is None:
            continue
        meta = dict(seeds.get((row["market"], code)) or {})
        meta["code"] = code
        if not rb.seed_schema_ok(meta):
            continue
        if key not in states:
            states[key] = bytearray(rb.INTERNAL)
            mapped[key] = meta
        rb.merge_into(states[key], row["rec"], int(row["mask"] or 0))
        amount = seed.i64(bytes(states[key]), 0x1C)
        if amount < 0:
            continue
    return states, mapped


def score(states, mapped, identity, oem, label: str) -> dict:
    hits = Counter()
    samples_wrong = []
    samples_dirty = []
    for key, st in states.items():
        hits["states"] += 1
        amount = seed.i64(bytes(st), 0x1C)
        meta = mapped[key]
        pub = meta["code"]
        ident_row = identity.get(key)
        true_code = ident_row["code"] if ident_row else None
        if amount < 0:
            hits["still_dirty"] += 1
            if len(samples_dirty) < 3:
                samples_dirty.append(
                    {"market": key[0], "index": key[1], "mapped": pub, "amount": amount}
                )
            continue
        hits["would_publish"] += 1
        if true_code is None:
            hits["no_identity"] += 1
            continue
        right = pub == true_code
        if right:
            hits["right_ticker"] += 1
        else:
            hits["wrong_ticker"] += 1
        pub_last = seed.rust_scaled(meta.get("last") or 0, meta.get("scale") or 1.0)
        oem_true = oem.get((key[0], true_code))
        oem_pub = oem.get((key[0], pub))
        if oem_true is None:
            hits["no_oem_true"] += 1
        else:
            hits["has_oem_true"] += 1
            if seed.same_f32(pub_last, oem_true["last_close"]):
                hits["last_eq_true_oem"] += 1
            if right:
                hits["right_and_last_eq"] += 1
        if oem_pub is not None and seed.same_f32(pub_last, oem_pub["last_close"]):
            hits["last_eq_published_oem"] += 1
        if (not right) and len(samples_wrong) < 6:
            samples_wrong.append(
                {
                    "market": key[0],
                    "index": key[1],
                    "published": pub,
                    "true": true_code,
                    "published_last": round(pub_last, 6),
                    "oem_true_last": None
                    if oem_true is None
                    else round(oem_true["last_close"], 6),
                    "oem_pub_last": None
                    if oem_pub is None
                    else round(oem_pub["last_close"], 6),
                    "name": meta.get("name") or "",
                }
            )
    n = hits["would_publish"] or 1
    w = hits["has_oem_true"] or 1
    return {
        "label": label,
        "states": hits["states"],
        "would_publish": hits["would_publish"],
        "still_dirty": hits["still_dirty"],
        "no_identity": hits["no_identity"],
        "right_ticker": hits["right_ticker"],
        "wrong_ticker": hits["wrong_ticker"],
        "has_oem_true": hits["has_oem_true"],
        "no_oem_true": hits["no_oem_true"],
        "last_eq_true_oem": hits["last_eq_true_oem"],
        "last_eq_published_oem": hits["last_eq_published_oem"],
        "right_and_last_eq": hits["right_and_last_eq"],
        "rates": {
            "publishable": round(hits["would_publish"] / (hits["states"] or 1), 6),
            "wrong_among_publishable": round(hits["wrong_ticker"] / n, 6),
            "last_eq_true_among_oem": round(hits["last_eq_true_oem"] / w, 6),
            "self_consistent_wrong_last": round(
                hits["last_eq_published_oem"] / n, 6
            ),
        },
        "samples_wrong_ticker": samples_wrong,
        "samples_still_dirty": samples_dirty,
    }


def main() -> int:
    if len(sys.argv) != 8:
        print(
            "usage: score-runtime-2023-snapshot-would-publish.py EXTRACT CALLBACK "
            "META_0903 MORNING_0104 NIGHT_EXTRACT NIGHT_0104 NIGHT_CALLBACK",
            file=sys.stderr,
        )
        return 2
    (
        extract_dir,
        callback_jsonl,
        meta_0903,
        morning_dir,
        night_extract,
        night_0104,
        night_callback,
    ) = sys.argv[1:]

    morning_tables = two.load_all_tables(morning_dir)
    symbol_codes, seeds, _ = two.simulate_runtime_maps(morning_tables)
    identity = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)
    states, mapped = persist_states(rows, symbol_codes, seeds)
    oem = last_oem(callback_jsonl)
    auction = score(states, mapped, identity, oem, "auction_stale_0915")

    night_tables = two.load_all_tables(night_0104)
    night_codes, night_seeds, _ = two.simulate_runtime_maps(night_tables)
    night_identity = {}
    for (market, idx), code in night_codes.items():
        night_identity[(market, idx)] = {"code": code}
    night_rows, _ = seed.load_decoded(night_extract)
    night_states, night_mapped = persist_states(night_rows, night_codes, night_seeds)
    night_oem = last_oem(night_callback, min_seq=21)
    night = score(
        night_states, night_mapped, night_identity, night_oem, "night_same_session"
    )

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_snapshot_would_publish.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "facts": {
            "snapshot_still_has_no_quotes": True,
            "this_is_hypothetical_if_wired_to_persist_oem_states": True,
        },
        "auction_stale_0915": auction,
        "night_same_session_seq_ge_21": night,
        "product_wiring": [
            "ShadowSnapshot still has no PublicQuote. These counts are what would emit if persist oem_states were published.",
            "Stale 09:15 index→code makes most end-of-stream quotes the wrong ticker; last_close often matches the WRONG stock's OEM last, so the row looks internally consistent.",
            "Night same-session 0104 is the positive control for identity. Wiring quotes without session-bound 0104 is still unsafe.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
