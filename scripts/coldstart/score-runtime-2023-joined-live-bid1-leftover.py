#!/usr/bin/env python3
"""How often does an exact-second live join carry leftover bid1 on the 2704?

Section 64 is n=5 first-prints. This scores the full wine-tail exact-second
+ max-seq join set. Leftover book is abs(i32 at +0x68) > 1e6 — not a real
A-share tick. Night dump bid1 remains 5166/5166 with this getter.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-joined-live-bid1-leftover.py EXTRACT CALLBACK META_0903
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
joined = SourceFileLoader(
    "score_joined_auction_fields",
    os.path.join(_DIR, "score-joined-auction-fields.py"),
).load_module()


def stream_with_bid1(path: str):
    with open(path, "r", encoding="utf-8") as fh:
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
            seq_i = int(seq or 0)
            for q in batch.get("quotes") or []:
                cb_ts = seed.parse_callback_ts(q.get("datetime") or "")
                yield {
                    "seq": seq_i,
                    "ts_ms": ts_ms,
                    "market": q["market"],
                    "code": q["code"],
                    "cb_ts": cb_ts,
                    "price": float(q.get("price") or 0),
                    "bid1": float((q.get("bid_prices") or [0])[0] or 0),
                }


def bid_kind(raw: int, scaled: float, oem_bid: float) -> str:
    if abs(raw) > 1_000_000:
        return "leftover_i32"
    if raw == 0:
        return "zero"
    if seed.same_f32(scaled, oem_bid):
        return "eq_oem"
    return "other_miss"


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: score-runtime-2023-joined-live-bid1-leftover.py EXTRACT "
            "CALLBACK META_0903",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903 = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    rows, _ = seed.load_decoded(extract_dir)
    cb_by = defaultdict(list)
    for q in stream_with_bid1(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    hits = Counter()
    live_close_eq = Counter()
    live_close_miss = Counter()
    leftover_close_eq = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        scale = float(meta.get("scale") or 0)
        if scale <= 0:
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        hits["joined"] += 1
        picked = joined.pick_later(cands)
        rec = row["rec"]
        close_i = seed.i32(rec, 0x10)
        bid_raw = seed.i32(rec, 0x68)
        close_f = seed.rust_scaled(close_i, scale)
        bid_f = seed.rust_scaled(bid_raw, scale)
        kind = bid_kind(bid_raw, bid_f, picked["bid1"])
        live = picked["price"] != 0
        close_eq = seed.same_f32(close_f, picked["price"])
        hits["live" if live else "oem_leftover"] += 1
        if live:
            hits[f"live_bid_{kind}"] += 1
            if close_eq:
                hits["live_close_eq"] += 1
                live_close_eq[kind] += 1
            else:
                hits["live_close_miss"] += 1
                live_close_miss[kind] += 1
            if kind == "leftover_i32" and close_eq and len(samples) < 6:
                samples.append(
                    {
                        "market": row["market"],
                        "index": row["index"],
                        "code": meta["code"],
                        "oem_price": picked["price"],
                        "oem_bid1": picked["bid1"],
                        "incoming_close": close_f,
                        "incoming_bid1": bid_f,
                        "incoming_bid1_raw": bid_raw,
                    }
                )
        elif close_eq:
            leftover_close_eq[kind] += 1

    live_n = hits["live"] or 1
    eq_n = hits["live_close_eq"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_joined_live_bid1_leftover.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "join": "exact business second + max callback sequence",
        "leftover_gate": "abs(i32 at +0x68) > 1e6",
        "joined": hits["joined"],
        "live": hits["live"],
        "oem_leftover": hits["oem_leftover"],
        "live_close_eq": hits["live_close_eq"],
        "live_close_miss": hits["live_close_miss"],
        "live_bid": {
            "leftover_i32": hits["live_bid_leftover_i32"],
            "zero": hits["live_bid_zero"],
            "eq_oem": hits["live_bid_eq_oem"],
            "other_miss": hits["live_bid_other_miss"],
        },
        "among_live_close_eq": dict(live_close_eq),
        "among_live_close_miss": dict(live_close_miss),
        "among_oem_leftover_close_eq": dict(leftover_close_eq),
        "rates": {
            "live_bid_leftover_among_live": round(hits["live_bid_leftover_i32"] / live_n, 6),
            "live_bid_eq_among_live": round(hits["live_bid_eq_oem"] / live_n, 6),
            "leftover_bid_among_live_close_eq": round(
                live_close_eq["leftover_i32"] / eq_n, 6
            ),
            "eq_bid_among_live_close_eq": round(live_close_eq["eq_oem"] / eq_n, 6),
        },
        "samples_live_close_eq_leftover_bid": samples,
        "product_wiring": [
            "Matching same-second close does not mean the 2704 book is OEM.",
            "Leftover i32 bid1 on a wine-tail live join is session leftover, not a +0x68 getter defect.",
            "Do not publish persist 311B as auction live. Night dump all_six remains 5166/5166.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
