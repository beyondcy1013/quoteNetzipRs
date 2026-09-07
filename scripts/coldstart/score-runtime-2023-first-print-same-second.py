#!/usr/bin/env python3
"""Join the 5 first-print live 2704s to same-second max-seq OEM.

Section 63 compared incoming bid1 to last OEM. If the print's business
second already has OEM bid1 15.50 while the 2704 has leftover −2e6, the
mismatch is not 'wrong last snapshot'. Join is exact second + max sequence.

Does not edit official_5188.rs or runtime.

usage:
  score-runtime-2023-first-print-same-second.py \\
    EXTRACT CALLBACK META_0903 MORNING_0104 FIRST_PRINT_JSON
"""
from __future__ import annotations

import json
import os
import sys
from datetime import datetime, timezone, timedelta

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
joined = SourceFileLoader(
    "score_joined_auction_fields",
    os.path.join(_DIR, "score-joined-auction-fields.py"),
).load_module()
inc = SourceFileLoader(
    "score_runtime_2023_first_print_incoming_bid1",
    os.path.join(_DIR, "score-runtime-2023-first-print-incoming-bid1.py"),
).load_module()

TZ8 = timezone(timedelta(hours=8))


def fmt_ts(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%Y-%m-%d %H:%M:%S")


def pick_later(cands: list) -> tuple:
    return max(cands, key=lambda q: (q[1], q[0]))


def main() -> int:
    if len(sys.argv) != 6:
        print(
            "usage: score-runtime-2023-first-print-same-second.py EXTRACT "
            "CALLBACK META_0903 MORNING_0104 FIRST_PRINT_JSON",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, morning_dir, first_json = sys.argv[1:]
    samples = json.loads(open(first_json, "rb").read()).get("samples_first_print") or []
    want = {(s["market"], s["index"]): s for s in samples}
    identity = seed.load_index_metadata(meta_0903)
    _, morning_seeds, _ = two.simulate_runtime_maps(two.load_all_tables(morning_dir))
    wanted_codes = set()
    for key, sample in want.items():
        ident_row = identity.get(key) or {}
        code = ident_row.get("code") or sample["code"]
        wanted_codes.add((key[0], code))
    quotes, _ = seed.stream_quotes(callback_jsonl, wanted_codes)
    by_sec = {}
    for (market, code), items in quotes.items():
        for q in items:
            if q[3] is None:
                continue
            by_sec.setdefault((market, code, q[3]), []).append(q)

    rows, _ = seed.load_decoded(extract_dir)
    live_row = {}
    for row in rows:
        key = (row["market"], row["index"])
        if key not in want:
            continue
        if seed.i32(row["rec"], 0x10) == 0:
            continue
        live_row[key] = row

    out_rows = []
    hits = {
        "joined": 0,
        "unmatched": 0,
        "close": 0,
        "open": 0,
        "volume": 0,
        "amount": 0,
        "bid1": 0,
        "last": 0,
        "all_six": 0,
        "bid1_leftover_vs_same_second": 0,
    }
    for key, sample in want.items():
        ident_row = identity.get(key) or {}
        code = ident_row.get("code") or sample["code"]
        meta = dict(morning_seeds.get((key[0], code)) or {})
        meta["code"] = code
        scale = meta.get("scale") or 1.0
        row = live_row.get(key)
        if row is None:
            out_rows.append({"code": code, "verdict": "no_live_row"})
            continue
        rec = row["rec"]
        close_f = seed.rust_scaled(seed.i32(rec, 0x10), scale)
        open_f = seed.rust_scaled(seed.i32(rec, 0x04), scale)
        vol_i = seed.i64(rec, 0x14)
        star = code.startswith(("688", "689"))
        vol_f = joined.star_lots(vol_i) if star else seed.to_f32(vol_i)
        amt_f = seed.to_f32(seed.i64(rec, 0x1C))
        bid_f = seed.rust_scaled(inc.bid1_i(rec), scale)
        last_f = seed.rust_scaled(meta.get("last") or 0, scale)
        cands = by_sec.get((key[0], code, row["ts"]), [])
        if not cands:
            hits["unmatched"] += 1
            out_rows.append(
                {
                    "market": key[0],
                    "index": key[1],
                    "code": code,
                    "internal_ts": fmt_ts(int(row["ts"])),
                    "verdict": "no_same_second_oem",
                    "incoming_close": close_f,
                    "incoming_bid1": bid_f,
                    "last_oem_bid1": sample["oem"]["bid1"],
                }
            )
            continue
        q = pick_later(cands)
        hits["joined"] += 1
        flags = {
            "close": seed.same_f32(close_f, q[4]),
            "open": seed.same_f32(open_f, q[6]),
            "volume": seed.same_f32(vol_f, q[9]),
            "amount": joined.rel_ok(amt_f, q[10], 1e-3),
            "bid1": seed.same_f32(bid_f, q[11][0]),
            "last": seed.same_f32(last_f, q[5]),
        }
        for name, ok in flags.items():
            if ok:
                hits[name] += 1
        if all(flags.values()):
            hits["all_six"] += 1
        leftover_book = abs(bid_f) > 1_000_000 or bid_f < -1
        if leftover_book and not flags["bid1"]:
            hits["bid1_leftover_vs_same_second"] += 1
        out_rows.append(
            {
                "market": key[0],
                "index": key[1],
                "code": code,
                "internal_ts": fmt_ts(int(row["ts"])),
                "oem_datetime": q[2],
                "oem_seq": q[1],
                "n_same_second": len(cands),
                "flags": flags,
                "incoming": {
                    "close": close_f,
                    "open": open_f,
                    "volume": vol_f,
                    "amount": amt_f,
                    "bid1": bid_f,
                },
                "same_second_oem": {
                    "price": q[4],
                    "open": q[6],
                    "volume": q[9],
                    "amount": q[10],
                    "bid1": q[11][0],
                    "last_close": q[5],
                },
                "last_oem_bid1": sample["oem"]["bid1"],
            }
        )

    n = hits["joined"] or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_runtime_2023_first_print_same_second.v1",
        "runtime_mtime": "2026-09-04 20:23",
        "join": "exact business second + max callback sequence",
        "hits": hits,
        "rates_among_joined": {
            "close": round(hits["close"] / n, 6),
            "bid1": round(hits["bid1"] / n, 6),
            "all_six": round(hits["all_six"] / n, 6),
        },
        "rows": out_rows,
        "product_wiring": [
            "Same-second OEM bid1 is the public book at the print; leftover incoming bid1 is not a later-snapshot artifact.",
            "Do not publish persist 311B as auction live. Night dump all_six remains 5166/5166.",
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
