#!/usr/bin/env python3
"""Do the 5 first-print close hits also match OEM volume/amount/bid1?

Section 60: only 5/5,421 OEM-live persist-clean rows print close from 0.
Matching last-OEM close is not a public quote. This scores persist open,
volume, amount, bid1 against last OEM, split by the section-60 buckets
plus close-mismatch and live-but-zero.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-first-print-fields.py EXTRACT CALLBACK META_0903 MORNING_0104
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
two = SourceFileLoader(
    "score_runtime_2010_two_step_seed",
    os.path.join(_DIR, "score-runtime-2010-two-step-seed.py"),
).load_module()
combo = SourceFileLoader(
    "score_runtime_2023_two_map_plus_rollback",
    os.path.join(_DIR, "score-runtime-2023-two-map-plus-rollback.py"),
).load_module()
ident = SourceFileLoader(
    "disambiguate_586_batch_identity",
    os.path.join(_DIR, "disambiguate-586-batch-identity.py"),
).load_module()
coin = SourceFileLoader(
    "score_runtime_2023_auction_65_coincidence",
    os.path.join(_DIR, "score-runtime-2023-auction-65-coincidence.py"),
).load_module()
joined = SourceFileLoader(
    "score_joined_auction_fields",
    os.path.join(_DIR, "score-joined-auction-fields.py"),
).load_module()


def last_oem_full(path: str, min_seq: int = 0) -> dict:
    out = {}
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
            if seq_i < min_seq:
                continue
            for q in batch.get("quotes") or []:
                key = (q["market"], q["code"])
                prev = out.get(key)
                if prev is not None and (seq_i, ts_ms) < (prev["seq"], prev["ts_ms"]):
                    continue
                out[key] = {
                    "seq": seq_i,
                    "ts_ms": ts_ms,
                    "price": float(q.get("price") or 0),
                    "last_close": float(q.get("last_close") or 0),
                    "open": float(q.get("open") or 0),
                    "volume": float(q.get("volume") or 0),
                    "amount": float(q.get("amount") or 0),
                    "bid1": float((q.get("bid_prices") or [0])[0] or 0),
                }
    return out


def project_state(st: bytes, meta: dict) -> dict:
    scale = meta.get("scale") or 1.0
    code = meta["code"]
    vol_i = seed.i64(st, 0x14)
    star = code.startswith(("688", "689"))
    return {
        "open": seed.rust_scaled(seed.i32(st, 0x04), scale),
        "close": seed.rust_scaled(coin.close_i(st), scale),
        "volume": joined.star_lots(vol_i) if star else seed.to_f32(vol_i),
        "amount": seed.to_f32(seed.i64(st, 0x1C)),
        "bid1": seed.rust_scaled(seed.i32(st, 0x68), scale),
        "last": seed.rust_scaled(meta.get("last") or 0, scale),
        "amount_i": seed.i64(st, 0x1C),
        "close_i": coin.close_i(st),
        "scale": scale,
        "code": code,
    }


def empty_hits():
    return Counter(
        n=0,
        close=0,
        open=0,
        volume=0,
        amount=0,
        bid1=0,
        last=0,
        all_six=0,
    )


def score_one(proj, q, hits):
    hits["n"] += 1
    close_ok = seed.same_f32(proj["close"], q["price"])
    open_ok = seed.same_f32(proj["open"], q["open"])
    vol_ok = seed.same_f32(proj["volume"], q["volume"])
    amt_ok = joined.rel_ok(proj["amount"], q["amount"], 1e-3)
    bid_ok = seed.same_f32(proj["bid1"], q["bid1"])
    last_ok = seed.same_f32(proj["last"], q["last_close"])
    if close_ok:
        hits["close"] += 1
    if open_ok:
        hits["open"] += 1
    if vol_ok:
        hits["volume"] += 1
    if amt_ok:
        hits["amount"] += 1
    if bid_ok:
        hits["bid1"] += 1
    if last_ok:
        hits["last"] += 1
    if close_ok and open_ok and vol_ok and amt_ok and bid_ok and last_ok:
        hits["all_six"] += 1
    return {
        "close": close_ok,
        "open": open_ok,
        "volume": vol_ok,
        "amount": amt_ok,
        "bid1": bid_ok,
        "last": last_ok,
    }


def pack(hits: Counter) -> dict:
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
            "close": round(hits["close"] / n, 6),
            "volume": round(hits["volume"] / n, 6),
            "amount": round(hits["amount"] / n, 6),
            "bid1": round(hits["bid1"] / n, 6),
            "all_six": round(hits["all_six"] / n, 6),
        },
    }


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: score-runtime-2023-first-print-fields.py EXTRACT "
            "CALLBACK META_0903 MORNING_0104",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir = sys.argv[1:]
    identity = seed.load_index_metadata(meta_0903)
    symbol_codes = {key: row["code"] for key, row in identity.items()}
    _, morning_seeds, _ = two.simulate_runtime_maps(two.load_all_tables(morning_dir))
    rows, _ = seed.load_decoded(extract_dir)
    oem = last_oem_full(callback_jsonl)
    states, mapped = combo.persist_states(rows, symbol_codes, morning_seeds, False)

    live_keys = []
    for key, st in states.items():
        if seed.i64(bytes(st), 0x1C) < 0:
            continue
        meta = mapped[key]
        q = oem.get((key[0], meta["code"]))
        if q is None or q["price"] == 0:
            continue
        live_keys.append(key)
    hist = coin.history(rows, set(live_keys))

    groups = defaultdict(empty_hits)
    samples = defaultdict(list)
    market_live = Counter()
    market_close_eq = Counter()
    for key in live_keys:
        st = bytes(states[key])
        meta = mapped[key]
        q = oem[(key[0], meta["code"])]
        proj = project_state(st, meta)
        market_live[key[0]] += 1
        seq = hist.get(key) or [0]
        if proj["amount_i"] == 0 and proj["close_i"] == 0:
            bucket = "oem_live_state_zero"
        elif seed.same_f32(proj["close"], q["price"]):
            market_close_eq[key[0]] += 1
            bucket = coin.bucket_of(proj["close"], proj["last"], seq[0], proj["scale"])
        else:
            bucket = "close_mismatch"
        flags = score_one(proj, q, groups[bucket])
        if bucket == "first_print_in_extract" and len(samples[bucket]) < 5:
            samples[bucket].append(
                {
                    "market": key[0],
                    "index": key[1],
                    "code": meta["code"],
                    "flags": flags,
                    "state": {
                        "close": proj["close"],
                        "open": proj["open"],
                        "volume": proj["volume"],
                        "amount": proj["amount"],
                        "bid1": proj["bid1"],
                    },
                    "oem": {
                        "price": q["price"],
                        "open": q["open"],
                        "volume": q["volume"],
                        "amount": q["amount"],
                        "bid1": q["bid1"],
                    },
                }
            )

    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_first_print_fields.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "oem_live": sum(market_live.values()),
        "market_live": dict(market_live),
        "market_close_eq": dict(market_close_eq),
        "groups": {name: pack(hits) for name, hits in sorted(groups.items())},
        "samples_first_print": samples.get("first_print_in_extract") or [],
        "product_wiring": [
            "Matching last-OEM close on a first-print-from-zero row is not a full public quote.",
            "Publish only when the 311B is the current OEM state (night dump), not a wine-tail leftover.",
            "Do not treat auction field misses as bitstream loss.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
