#!/usr/bin/env python3
"""STAR book-volume rounding ablation on the night same-session fixture.

The frozen recipe leaves 97 book_vol_5 fails after round(/100). This compares
rounding modes per level without touching official_5188.rs.

usage:
  ablate-star-book-lots.py CALLBACK PROBE NIGHT_0104
"""
from __future__ import annotations

import json
import math
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


def rust_f32_round_div100(value: int) -> float:
    return seed.to_f32(round(seed.to_f32(value) / seed.to_f32(100.0)))


def py_round_div100(value: int) -> float:
    return seed.to_f32(round(value / 100.0))


def half_up(value: int) -> float:
    return seed.to_f32((value + 50) // 100 if value >= 0 else -((-value + 50) // 100))


def ceil_div100(value: int) -> float:
    if value <= 0:
        return seed.to_f32(value // 100)
    return seed.to_f32((value + 99) // 100)


def floor_div100(value: int) -> float:
    return seed.to_f32(value // 100)


def trunc_div100(value: int) -> float:
    return seed.to_f32(int(value / 100))


def raw_f32(value: int) -> float:
    return seed.to_f32(value)


def ceil_if_small(value: int) -> float:
    if 0 < value < 100:
        return 1.0
    return py_round_div100(value)


MODES = {
    "py_round": py_round_div100,
    "rust_f32_round": rust_f32_round_div100,
    "half_up": half_up,
    "ceil": ceil_div100,
    "floor": floor_div100,
    "trunc": trunc_div100,
    "raw": raw_f32,
    "ceil_if_lt100": ceil_if_small,
}


def levels_ok(internal: list[int], oem: list[float], fn) -> bool:
    return all(seed.same_f32(fn(internal[i]), oem[i]) for i in range(5))


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: ablate-star-book-lots.py CALLBACK PROBE NIGHT_0104", file=sys.stderr)
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
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch or int(ev.get("sequence") or 0) < 21:
                continue
            for q in batch.get("quotes") or []:
                post[(q["market"], q["code"])] = q

    mode_hits = {name: 0 for name in MODES}
    compared = 0
    star = 0
    fail_round = 0
    remainders = Counter()
    oem_minus_floor = Counter()
    samples = []
    volume_fails = []
    mixed = Counter()
    for key, row in probe.items():
        oem = post.get(key)
        meta = night_meta.get(key)
        if oem is None or not meta or meta["scale"] <= 0:
            continue
        parsed = book.parse_ladder(row.get("record_hex") or "")
        if parsed is None:
            continue
        compared += 1
        prices, volumes = parsed
        _, _, bid_v, ask_v = book.project_book(
            prices, volumes, float(meta["scale"]), star_lots=key[1].startswith(("688", "689"))
        )
        oem_bv = [float(x) for x in (oem.get("bid_volumes") or [0] * 10)[:5]]
        oem_av = [float(x) for x in (oem.get("ask_volumes") or [0] * 10)[:5]]
        star_code = key[1].startswith(("688", "689"))
        if star_code:
            star += 1
        internal_bid = [volumes[4 - i] for i in range(5)]
        internal_ask = [volumes[5 + i] for i in range(5)]
        current_ok = book.all_same(bid_v, oem_bv) and book.all_same(ask_v, oem_av)
        if star_code and not current_ok:
            fail_round += 1
            for src, dst in ((internal_bid, oem_bv), (internal_ask, oem_av)):
                for raw, got in zip(src, dst):
                    if raw == 0 and got == 0:
                        continue
                    remainders[raw % 100] += 1
                    oem_minus_floor[int(got) - (raw // 100)] += 1
            if len(samples) < 12:
                samples.append(
                    {
                        "code": key[1],
                        "internal_bid": internal_bid,
                        "oem_bid": oem_bv,
                        "proj_round_bid": [py_round_div100(v) for v in internal_bid],
                        "internal_ask": internal_ask,
                        "oem_ask": oem_av,
                        "proj_round_ask": [py_round_div100(v) for v in internal_ask],
                    }
                )
        for name, fn in MODES.items():
            if levels_ok(internal_bid, oem_bv, fn) and levels_ok(internal_ask, oem_av, fn):
                mode_hits[name] += 1
        if star_code:
            which = []
            for name, fn in MODES.items():
                if levels_ok(internal_bid, oem_bv, fn) and levels_ok(internal_ask, oem_av, fn):
                    which.append(name)
            mixed[tuple(which) or ("none",)] += 1
        vol = int(row["volume"])
        oem_vol = float(oem.get("volume") or 0)
        proj_vol = seed.to_f32(round(vol / 100.0) if star_code else vol)
        if not seed.same_f32(proj_vol, oem_vol):
            volume_fails.append(
                {
                    "code": key[1],
                    "star": star_code,
                    "internal": vol,
                    "proj_round": proj_vol,
                    "oem": oem_vol,
                    "floor": seed.to_f32(vol // 100) if star_code else None,
                    "ceil": seed.to_f32((vol + 99) // 100) if star_code and vol > 0 else None,
                    "raw": seed.to_f32(vol),
                }
            )

    n = compared or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_star_book_lots.v1",
        "compared": compared,
        "star_codes": star,
        "current_round_book_fails_on_star": fail_round,
        "mode_full_book_hits": mode_hits,
        "mode_full_book_rates": {k: round(v / n, 4) for k, v in mode_hits.items()},
        "star_fail_remainder_histogram": dict(sorted(remainders.items())),
        "star_fail_oem_minus_floor": dict(sorted(oem_minus_floor.items())),
        "star_mode_combo_on_star_codes": {
            ",".join(k): v for k, v in mixed.most_common(12)
        },
        "cumulative_volume_fails": volume_fails,
        "samples": samples,
        "note": (
            "Modes are scored on all compared codes. STAR fails isolate the 97. "
            "Do not feed this into bitstream token tables."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
