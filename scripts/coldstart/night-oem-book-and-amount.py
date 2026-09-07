#!/usr/bin/env python3
"""Night same-session OEM five-level book + amount-tail classification.

Scalar parity (section 16) used bid1/ask1 only. This compares the five
represented levels and whether OEM 6-10 are zero. Also classifies the 47
amount rows that miss 1e-4.

usage:
  night-oem-book-and-amount.py CALLBACK PROBE NIGHT_0104
"""
from __future__ import annotations

import json
import os
import struct
import sys
from collections import Counter, defaultdict

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()


def rel_err(left: float, right: float) -> float | None:
    denom = max(abs(right), abs(left))
    if denom == 0:
        return 0.0 if left == 0 and right == 0 else None
    return abs(left - right) / denom


def parse_ladder(record_hex: str) -> tuple[list[int], list[int]] | None:
    try:
        raw = bytes.fromhex(record_hex)
    except ValueError:
        return None
    if len(raw) < 0xCC + 4:
        return None
    prices = [seed.i32(raw, 0x58 + i * 4) for i in range(10)]
    volumes = [seed.i32(raw, 0xA8 + i * 4) for i in range(10)]
    return prices, volumes


def project_book(prices: list[int], volumes: list[int], scale: float, *, star_lots: bool):
    bid_p = [seed.rust_scaled(prices[4 - i], scale) for i in range(5)]
    ask_p = [seed.rust_scaled(prices[5 + i], scale) for i in range(5)]
    if star_lots:
        bid_v = [seed.to_f32(round(volumes[4 - i] / 100.0)) for i in range(5)]
        ask_v = [seed.to_f32(round(volumes[5 + i] / 100.0)) for i in range(5)]
    else:
        bid_v = [seed.to_f32(volumes[4 - i]) for i in range(5)]
        ask_v = [seed.to_f32(volumes[5 + i]) for i in range(5)]
    return bid_p, ask_p, bid_v, ask_v


def all_same(left: list[float], right: list[float]) -> bool:
    return all(seed.same_f32(a, b) for a, b in zip(left, right))


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: night-oem-book-and-amount.py CALLBACK PROBE NIGHT_0104", file=sys.stderr)
        return 2
    callback_jsonl, probe_json, night_0104 = sys.argv[1:]
    night_meta = seed.load_largest_0104(night_0104)
    probe = {}
    for row in json.loads(open(probe_json, "rb").read()):
        code = row.get("code") or ""
        market = row.get("market") or ""
        if not code:
            continue
        key = (market, code)
        prev = probe.get(key)
        if prev is None or int(row.get("frame_index") or 0) >= int(prev.get("frame_index") or 0):
            probe[key] = row
    post = {}
    oem_tail_nonzero = Counter()
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch:
                continue
            seq = int(ev.get("sequence") or 0)
            if seq < 21:
                continue
            for q in batch.get("quotes") or []:
                key = (q["market"], q["code"])
                bid_p = [float(x) for x in (q.get("bid_prices") or [0] * 10)]
                ask_p = [float(x) for x in (q.get("ask_prices") or [0] * 10)]
                bid_v = [float(x) for x in (q.get("bid_volumes") or [0] * 10)]
                ask_v = [float(x) for x in (q.get("ask_volumes") or [0] * 10)]
                if any(x != 0 for x in bid_p[5:]):
                    oem_tail_nonzero["bid_p_6_10"] += 1
                if any(x != 0 for x in ask_p[5:]):
                    oem_tail_nonzero["ask_p_6_10"] += 1
                if any(x != 0 for x in bid_v[5:]):
                    oem_tail_nonzero["bid_v_6_10"] += 1
                if any(x != 0 for x in ask_v[5:]):
                    oem_tail_nonzero["ask_v_6_10"] += 1
                oem_tail_nonzero["quotes"] += 1
                post[key] = {
                    "seq": seq,
                    "price": float(q.get("price") or 0),
                    "last_close": float(q.get("last_close") or 0),
                    "amount": float(q.get("amount") or 0),
                    "volume": float(q.get("volume") or 0),
                    "bid_p": bid_p,
                    "ask_p": ask_p,
                    "bid_v": bid_v,
                    "ask_v": ask_v,
                    "datetime": q.get("datetime") or "",
                    "code": q["code"],
                    "market": q["market"],
                }

    book = Counter()
    amount_tail = []
    samples = defaultdict(list)
    compared = 0
    no_hex = 0
    for key, row in probe.items():
        oem = post.get(key)
        if oem is None:
            continue
        meta = night_meta.get(key)
        scale = float((meta["scale"] if meta else 0) or row.get("scale") or 0)
        if scale <= 0:
            continue
        parsed = parse_ladder(row.get("record_hex") or "")
        if parsed is None:
            no_hex += 1
            continue
        prices, volumes = parsed
        star = key[1].startswith(("688", "689"))
        bid_p, ask_p, bid_v, ask_v = project_book(prices, volumes, scale, star_lots=False)
        bid_p_s, ask_p_s, bid_v_s, ask_v_s = project_book(prices, volumes, scale, star_lots=True)
        compared += 1
        if all_same(bid_p, oem["bid_p"][:5]):
            book["bid_px_5"] += 1
        if all_same(ask_p, oem["ask_p"][:5]):
            book["ask_px_5"] += 1
        if all_same(bid_v, oem["bid_v"][:5]):
            book["bid_vol_5"] += 1
        if all_same(ask_v, oem["ask_v"][:5]):
            book["ask_vol_5"] += 1
        vol_ok = all_same(bid_v, oem["bid_v"][:5]) and all_same(ask_v, oem["ask_v"][:5])
        vol_star_ok = all_same(bid_v_s, oem["bid_v"][:5]) and all_same(ask_v_s, oem["ask_v"][:5])
        if vol_ok or (star and vol_star_ok):
            book["book_vol_5_with_star_lots"] += 1
        if all_same(bid_p, oem["bid_p"][:5]) and all_same(ask_p, oem["ask_p"][:5]):
            book["book_px_5"] += 1
        else:
            if len(samples["book_px_miss"]) < 6:
                samples["book_px_miss"].append(
                    {
                        "code": key[1],
                        "scale": scale,
                        "internal_p": prices,
                        "proj_bid": bid_p,
                        "oem_bid": oem["bid_p"][:5],
                        "proj_ask": ask_p,
                        "oem_ask": oem["ask_p"][:5],
                    }
                )
        amt = seed.to_f32(int(row["amount"]))
        err = rel_err(amt, oem["amount"])
        if err is None or err >= 1e-4:
            amount_tail.append(
                {
                    "code": key[1],
                    "star": star,
                    "amount_mode": None if meta is None else meta.get("amount_mode"),
                    "places": None if meta is None else meta.get("places"),
                    "internal_amount": int(row["amount"]),
                    "proj_f32": amt,
                    "oem_amount": oem["amount"],
                    "rel": err,
                    "volume_internal": int(row["volume"]),
                    "oem_volume": oem["volume"],
                }
            )

    n = compared or 1
    report = {
        "compared": compared,
        "probe_without_record_hex": no_hex,
        "oem_levels_6_10_nonzero_across_all_post_dump_quotes": dict(oem_tail_nonzero),
        "book_hits": dict(book),
        "book_rates": {k: round(book[k] / n, 4) for k in book},
        "amount_tail_rel_ge_1e-4": {
            "n": len(amount_tail),
            "star": sum(1 for row in amount_tail if row["star"]),
            "amount_mode": dict(Counter(str(row["amount_mode"]) for row in amount_tail)),
            "rel_buckets": dict(
                Counter(
                    "<1e-3" if (row["rel"] or 1) < 1e-3 else "<1e-2" if (row["rel"] or 1) < 1e-2 else ">=1e-2"
                    for row in amount_tail
                )
            ),
            "samples": amount_tail[:12],
        },
        "book_px_miss_samples": samples["book_px_miss"],
        "note": (
            "Internal ladder is bid5..bid1, ask1..ask5 at +0x58/+0xa8. "
            "STAR lots apply to cumulative volume; this script tests them on book volumes too."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
