#!/usr/bin/env python3
"""Are the 6 exact-second live close+bid1 hits a full public OEM quote?

Section 65: 46 live close-eq, 6 also bid1-eq, 19 bid1-eq among 5,039 live.
This scores close/open/volume/amount/bid1/last on those subsets.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-joined-live-all-six.py EXTRACT CALLBACK META_0903 MORNING_0104
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
joined = SourceFileLoader(
    "score_joined_auction_fields",
    os.path.join(_DIR, "score-joined-auction-fields.py"),
).load_module()
lv = SourceFileLoader(
    "score_runtime_2023_joined_live_bid1_leftover",
    os.path.join(_DIR, "score-runtime-2023-joined-live-bid1-leftover.py"),
).load_module()


def stream_full(path: str):
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
                    "last_close": float(q.get("last_close") or 0),
                    "open": float(q.get("open") or 0),
                    "volume": float(q.get("volume") or 0),
                    "amount": float(q.get("amount") or 0),
                    "bid1": float((q.get("bid_prices") or [0])[0] or 0),
                }


def flags_of(rec: bytes, meta: dict, morn: dict | None, q: dict) -> dict:
    scale = float(meta.get("scale") or 0)
    code = meta["code"]
    star = code.startswith(("688", "689"))
    vol_i = seed.i64(rec, 0x14)
    close_f = seed.rust_scaled(seed.i32(rec, 0x10), scale)
    open_f = seed.rust_scaled(seed.i32(rec, 0x04), scale)
    vol_f = joined.star_lots(vol_i) if star else seed.to_f32(vol_i)
    amt_f = seed.to_f32(seed.i64(rec, 0x1C))
    bid_f = seed.rust_scaled(seed.i32(rec, 0x68), scale)
    last_f = None
    if morn and morn.get("scale", 0) > 0:
        last_f = seed.rust_scaled(morn["last_0104"], morn["scale"])
    flags = {
        "close": seed.same_f32(close_f, q["price"]),
        "open": seed.same_f32(open_f, q["open"]),
        "volume": seed.same_f32(vol_f, q["volume"]),
        "amount": joined.rel_ok(amt_f, q["amount"], 1e-3),
        "bid1": seed.same_f32(bid_f, q["bid1"]),
        "last": last_f is not None and seed.same_f32(last_f, q["last_close"]),
    }
    proj = {
        "close": close_f,
        "open": open_f,
        "volume": vol_f,
        "amount": amt_f,
        "bid1": bid_f,
        "last": last_f,
    }
    return flags, proj


def pack_counter(hits: Counter) -> dict:
    n = hits["n"] or 1
    return {
        "n": hits["n"],
        "close": hits["close"],
        "open": hits["open"],
        "volume": hits["volume"],
        "amount_rel_1e-3": hits["amount"],
        "bid1": hits["bid1"],
        "last_close": hits["last"],
        "all_six": hits["all_six"],
        "rates": {
            "all_six": round(hits["all_six"] / n, 6),
            "bid1": round(hits["bid1"] / n, 6),
            "close": round(hits["close"] / n, 6),
        },
    }


def add(hits: Counter, flags: dict) -> None:
    hits["n"] += 1
    for name in ("close", "open", "volume", "amount", "bid1", "last"):
        if flags[name]:
            hits[name] += 1
    if all(flags[name] for name in ("close", "open", "volume", "amount", "bid1", "last")):
        hits["all_six"] += 1


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-joined-live-all-six.py EXTRACT "
            "CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    metadata = seed.load_index_metadata(meta_0903)
    morning = seed.load_largest_0104(morning_dir)
    rows, _ = seed.load_decoded(extract_dir)
    cb_by = defaultdict(list)
    for q in stream_full(callback_jsonl):
        if q["cb_ts"] is None:
            continue
        cb_by[(q["market"], q["code"], q["cb_ts"])].append(q)

    live_all = Counter()
    close_eq = Counter()
    bid_eq = Counter()
    close_and_bid = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None or float(meta.get("scale") or 0) <= 0:
            continue
        cands = cb_by.get((row["market"], meta["code"], row["ts"]), [])
        if not cands:
            continue
        q = joined.pick_later(cands)
        if q["price"] == 0:
            continue
        morn = morning.get((row["market"], meta["code"]))
        flags, proj = flags_of(row["rec"], meta, morn, q)
        add(live_all, flags)
        if flags["close"]:
            add(close_eq, flags)
        if flags["bid1"]:
            add(bid_eq, flags)
        if flags["close"] and flags["bid1"]:
            add(close_and_bid, flags)
            if len(samples) < 6:
                samples.append(
                    {
                        "market": row["market"],
                        "index": row["index"],
                        "code": meta["code"],
                        "flags": flags,
                        "incoming": proj,
                        "oem": {
                            "price": q["price"],
                            "open": q["open"],
                            "volume": q["volume"],
                            "amount": q["amount"],
                            "bid1": q["bid1"],
                            "last_close": q["last_close"],
                        },
                    }
                )

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_joined_live_all_six.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "join": "exact business second + max callback sequence",
        "live_all": pack_counter(live_all),
        "live_close_eq": pack_counter(close_eq),
        "live_bid1_eq": pack_counter(bid_eq),
        "live_close_and_bid1": pack_counter(close_and_bid),
        "samples_close_and_bid1": samples,
        "product_wiring": [
            "Even the rare wine-tail rows that match close and bid1 may miss open/volume/amount.",
            "Do not publish persist 311B as auction live. Night dump all_six remains 5166/5166.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
