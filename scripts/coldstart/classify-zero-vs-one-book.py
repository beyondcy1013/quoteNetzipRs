#!/usr/bin/env python3
"""Classify the 22 non-STAR book levels where internal volume is 0 and OEM is 1.

usage:
  classify-zero-vs-one-book.py CALLBACK PROBE NIGHT_0104
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
book = SourceFileLoader(
    "night_oem_book_and_amount",
    os.path.join(_DIR, "night-oem-book-and-amount.py"),
).load_module()


def star_lots(value: int) -> float:
    if value <= 0:
        return 0.0
    lots = (value + 50) // 100
    if lots == 0:
        lots = 1
    return seed.to_f32(lots)


def project_vol(value: int, star: bool) -> float:
    return star_lots(value) if star else seed.to_f32(value)


def fill_zero_price(value: int, price: int, star: bool) -> float:
    if value == 0 and price != 0:
        return 1.0
    return project_vol(value, star)


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: classify-zero-vs-one-book.py CALLBACK PROBE NIGHT_0104",
            file=sys.stderr,
        )
        return 2
    callback_jsonl, probe_json, night_0104 = sys.argv[1:]
    night_meta = seed.load_largest_0104(night_0104)
    probe = {}
    for row in json.loads(open(probe_json, "rb").read()):
        code = row.get("code") or ""
        if not code:
            continue
        key = (row["market"], code)
        prev = probe.get(key)
        if prev is None or int(row.get("frame_index") or 0) >= int(prev.get("frame_index") or 0):
            probe[key] = row
    post = {}
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch or int(ev.get("sequence") or 0) < 21:
                continue
            for q in batch.get("quotes") or []:
                post[(q["market"], q["code"])] = q

    misses = []
    compared = 0
    fill_hits = 0
    raw_hits = 0
    prefixes = Counter()
    modes = Counter()
    miss_kind = Counter()
    for key, row in probe.items():
        oem = post.get(key)
        meta = night_meta.get(key)
        if oem is None or not meta or meta["scale"] <= 0:
            continue
        parsed = book.parse_ladder(row.get("record_hex") or "")
        if parsed is None:
            continue
        compared += 1
        star = key[1].startswith(("688", "689"))
        prices, volumes = parsed
        bid_p = [prices[4 - i] for i in range(5)]
        ask_p = [prices[5 + i] for i in range(5)]
        bid_v = [volumes[4 - i] for i in range(5)]
        ask_v = [volumes[5 + i] for i in range(5)]
        oem_bv = [float(x) for x in (oem.get("bid_volumes") or [0] * 10)[:5]]
        oem_av = [float(x) for x in (oem.get("ask_volumes") or [0] * 10)[:5]]
        oem_bp = [float(x) for x in (oem.get("bid_prices") or [0] * 10)[:5]]
        oem_ap = [float(x) for x in (oem.get("ask_prices") or [0] * 10)[:5]]
        scale = float(meta["scale"])
        raw_ok = all(
            seed.same_f32(project_vol(a, star), b)
            for a, b in zip(bid_v + ask_v, oem_bv + oem_av)
        )
        fill_ok = all(
            seed.same_f32(fill_zero_price(a, p, star), b)
            for a, p, b in zip(bid_v + ask_v, bid_p + ask_p, oem_bv + oem_av)
        )
        if raw_ok:
            raw_hits += 1
        if fill_ok:
            fill_hits += 1
        if raw_ok:
            continue
        diffs = []
        for side, iv, ip, ov, op in (
            ("bid", bid_v, bid_p, oem_bv, oem_bp),
            ("ask", ask_v, ask_p, oem_av, oem_ap),
        ):
            for level in range(5):
                proj = project_vol(iv[level], star)
                if seed.same_f32(proj, ov[level]):
                    continue
                kind = (
                    "zero_vs_one"
                    if iv[level] == 0 and ov[level] == 1.0
                    else "other"
                )
                miss_kind[kind] += 1
                diffs.append(
                    {
                        "side": side,
                        "level": level + 1,
                        "internal_vol": iv[level],
                        "internal_px": ip[level],
                        "oem_vol": ov[level],
                        "oem_px": op[level],
                        "scaled_internal_px": seed.rust_scaled(ip[level], scale),
                        "price_nonzero": ip[level] != 0,
                    }
                )
        prefixes[key[1][:3]] += 1
        modes[str(meta.get("amount_mode"))] += 1
        misses.append(
            {
                "market": key[0],
                "code": key[1],
                "amount_mode": meta.get("amount_mode"),
                "fill_ok": fill_ok,
                "diffs": diffs,
            }
        )

    report = {
        "schema": "quoteNetzipRs.official_5188_zero_vs_one_book.v1",
        "compared": compared,
        "raw_book_hits": raw_hits,
        "fill_zero_if_price_hits": fill_hits,
        "misses": len(misses),
        "prefixes": dict(prefixes),
        "amount_modes": dict(modes),
        "miss_kinds": dict(miss_kind),
        "price_nonzero_on_miss_levels": sum(
            1 for row in misses for d in row["diffs"] if d["price_nonzero"]
        ),
        "price_zero_on_miss_levels": sum(
            1 for row in misses for d in row["diffs"] if not d["price_nonzero"]
        ),
        "all_misses": misses,
        "note": (
            "If every miss is internal vol=0, OEM vol=1, and the same level has a "
            "non-zero price, Wine displays a 1-lot placeholder. That is OEM "
            "presentation, not a 2704 bitstream defect."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
