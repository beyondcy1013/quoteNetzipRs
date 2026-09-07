#!/usr/bin/env python3
"""Project 311B → OEM_REPORT with the frozen recipe and score full-record parity.

Overnight mode uses last_close from +0x12b. Datetime gate is ±1s. Amount gate
is relative <1e-3. STAR 688/689 volumes use round(/100).

usage:
  project-311b-to-oem.py CALLBACK PROBE NIGHT_0104
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone

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
freeze = SourceFileLoader(
    "freeze_oem_projection_recipe",
    os.path.join(_DIR, "freeze-oem-projection-recipe.py"),
).load_module()

TZ8 = timezone(timedelta(hours=8))


def fmt_ts(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%Y-%m-%d %H:%M:%S")


def rel_ok(left: float, right: float, limit: float) -> bool:
    denom = max(abs(left), abs(right))
    if denom == 0:
        return left == 0 and right == 0
    return abs(left - right) / denom < limit


def project(row: dict, meta: dict, name: str) -> dict:
    scale = float(meta["scale"])
    star = row["code"].startswith(("688", "689"))
    prices, volumes = book.parse_ladder(row["record_hex"])
    bid_p, ask_p, bid_v, ask_v = book.project_book(prices, volumes, scale, star_lots=star)
    vol = int(row["volume"])
    return {
        "market": row["market"],
        "code": row["code"],
        "name": name,
        "datetime": fmt_ts(int(row["ts"])),
        "price": seed.rust_scaled(int(row["close"]), scale),
        "last_close": seed.rust_scaled(int(row["last_close"]), scale),
        "open": seed.rust_scaled(int(row["open"]), scale),
        "high": seed.rust_scaled(int(row["high"]), scale),
        "low": seed.rust_scaled(int(row["low"]), scale),
        "volume": seed.to_f32(round(vol / 100.0) if star else vol),
        "amount": seed.to_f32(int(row["amount"])),
        "bid_prices": bid_p + [0.0] * 5,
        "ask_prices": ask_p + [0.0] * 5,
        "bid_volumes": bid_v + [0.0] * 5,
        "ask_volumes": ask_v + [0.0] * 5,
        "source_protocol": "quoteNetzipRs.official_5188_oem_projection.v1",
    }


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: project-311b-to-oem.py CALLBACK PROBE NIGHT_0104", file=sys.stderr)
        return 2
    callback_jsonl, probe_json, night_0104 = sys.argv[1:]
    night_meta = seed.load_largest_0104(night_0104)
    names = freeze.load_0104_names(night_0104)
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

    gates = Counter()
    fail_sets = Counter()
    samples = []
    compared = 0
    for key, row in probe.items():
        oem = post.get(key)
        meta = night_meta.get(key)
        if oem is None or not meta or meta["scale"] <= 0:
            continue
        if book.parse_ladder(row.get("record_hex") or "") is None:
            continue
        compared += 1
        proj = project(row, meta, names.get(key) or row.get("name") or "")
        parsed = seed.parse_callback_ts(oem.get("datetime") or "")
        ts = int(row["ts"])
        failed = []
        if proj["name"] != (oem.get("name") or ""):
            failed.append("name")
        if parsed is None or abs(parsed - ts) > 1:
            failed.append("datetime")
        for field in ("price", "last_close", "open", "high", "low"):
            if not seed.same_f32(proj[field], float(oem.get(field) or 0)):
                failed.append(field)
        if not seed.same_f32(proj["volume"], float(oem.get("volume") or 0)):
            failed.append("volume")
        if not rel_ok(proj["amount"], float(oem.get("amount") or 0), 1e-3):
            failed.append("amount")
        oem_bp = [float(x) for x in (oem.get("bid_prices") or [0] * 10)]
        oem_ap = [float(x) for x in (oem.get("ask_prices") or [0] * 10)]
        oem_bv = [float(x) for x in (oem.get("bid_volumes") or [0] * 10)]
        oem_av = [float(x) for x in (oem.get("ask_volumes") or [0] * 10)]
        if not book.all_same(proj["bid_prices"][:5], oem_bp[:5]) or not book.all_same(
            proj["ask_prices"][:5], oem_ap[:5]
        ):
            failed.append("book_px_5")
        if any(oem_bp[5:]) or any(oem_ap[5:]) or any(oem_bv[5:]) or any(oem_av[5:]):
            failed.append("oem_levels_6_10")
        if not book.all_same(proj["bid_volumes"][:5], oem_bv[:5]) or not book.all_same(
            proj["ask_volumes"][:5], oem_av[:5]
        ):
            failed.append("book_vol_5")
        if not failed:
            gates["full_record"] += 1
        else:
            fail_sets[tuple(failed)] += 1
            if len(samples) < 10:
                samples.append({"code": key[1], "failed": failed, "proj_dt": proj["datetime"], "oem_dt": oem.get("datetime")})
        for item in (
            "name",
            "datetime",
            "price",
            "last_close",
            "open",
            "high",
            "low",
            "volume",
            "amount",
            "book_px_5",
            "book_vol_5",
            "oem_levels_6_10",
        ):
            if item not in failed:
                gates[item] += 1
        core = [f for f in failed if f != "book_vol_5" and f != "datetime"]
        if not core and "datetime" not in failed:
            gates["full_except_book_vol"] += 1
        if not core:
            gates["full_except_book_vol_and_datetime_1s"] += 1

    n = compared or 1
    report = {
        "schema": "quoteNetzipRs.official_5188_oem_projection.parity.v1",
        "mode": "overnight_last_close_0x12b",
        "compared": compared,
        "full_record_parity": gates["full_record"],
        "full_record_rate": round(gates["full_record"] / n, 4),
        "full_except_book_vol": gates["full_except_book_vol"],
        "gates": {k: gates[k] for k in gates},
        "gate_rates": {
            k: round(gates[k] / n, 4)
            for k in (
                "name",
                "datetime",
                "price",
                "last_close",
                "open",
                "high",
                "low",
                "volume",
                "amount",
                "book_px_5",
                "book_vol_5",
                "oem_levels_6_10",
            )
        },
        "top_fail_sets": [[list(k), v] for k, v in fail_sets.most_common(8)],
        "samples": samples,
        "acceptance": (
            "Night same-session OEM parity under frozen recipe. "
            "full_record requires datetime ±1s, amount rel<1e-3, STAR lots, five-level books."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
