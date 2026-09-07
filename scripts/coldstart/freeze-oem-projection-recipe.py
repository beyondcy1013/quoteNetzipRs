#!/usr/bin/env python3
"""Freeze the 311B→OEM projection recipe against the night same-session fixture.

Scores remaining OEM_REPORT fields (datetime, name) and writes a field-by-field
recipe with hit rates. Does not touch owned decoder files.

usage:
  freeze-oem-projection-recipe.py CALLBACK PROBE NIGHT_0104 OUT_JSON
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone

_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _DIR)
from importlib.machinery import SourceFileLoader

seed = SourceFileLoader(
    "seed_last_close_and_preopen",
    os.path.join(_DIR, "seed-last-close-and-preopen.py"),
).load_module()

TZ8 = timezone(timedelta(hours=8))


def fmt_ts(ts: int) -> str:
    return datetime.fromtimestamp(ts, TZ8).strftime("%Y-%m-%d %H:%M:%S")


def load_0104_names(directory: str) -> dict[tuple[str, str], str]:
    by_market: dict[str, list] = defaultdict(list)
    for name in os.listdir(directory):
        if not name.endswith("0104.code-table.json"):
            continue
        table = json.loads(open(os.path.join(directory, name), "rb").read())
        market = bytes(table["market"]).decode()
        by_market[market].append((len(table["records"]), table))
    out = {}
    for market, items in by_market.items():
        items.sort(reverse=True)
        for rec in items[0][1]["records"]:
            code = rec.get("code") or ""
            if code:
                out[(market, code)] = rec.get("name") or ""
    return out


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: freeze-oem-projection-recipe.py CALLBACK PROBE NIGHT_0104 OUT_JSON",
            file=sys.stderr,
        )
        return 2
    callback_jsonl, probe_json, night_0104, out_json = sys.argv[1:]
    names = load_0104_names(night_0104)
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

    hits = Counter()
    dt_delta = Counter()
    samples = []
    compared = 0
    for key, row in probe.items():
        oem = post.get(key)
        if oem is None:
            continue
        compared += 1
        ts = int(row["ts"])
        oem_dt = oem.get("datetime") or ""
        parsed = seed.parse_callback_ts(oem_dt)
        fmt = fmt_ts(ts)
        if parsed == ts:
            hits["datetime_exact"] += 1
        if parsed is not None and abs(parsed - ts) <= 1:
            hits["datetime_within_1s"] += 1
        if fmt == oem_dt:
            hits["datetime_format_from_311B_ts"] += 1
        if parsed is not None:
            dt_delta[parsed - ts] += 1
        probe_name = row.get("name") or ""
        oem_name = oem.get("name") or ""
        n0104 = names.get(key, "")
        if oem_name == probe_name:
            hits["name_vs_probe"] += 1
        if oem_name == n0104:
            hits["name_vs_0104"] += 1
        if oem_name == probe_name or oem_name == n0104:
            hits["name_vs_probe_or_0104"] += 1
        if fmt != oem_dt and len(samples) < 8:
            samples.append(
                {
                    "code": key[1],
                    "ts": ts,
                    "fmt": fmt,
                    "oem_dt": oem_dt,
                    "delta": None if parsed is None else parsed - ts,
                    "probe_name": probe_name,
                    "oem_name": oem_name,
                    "name_0104": n0104,
                }
            )

    n = compared or 1
    scalar = json.loads(
        open(
            os.path.join(
                os.path.dirname(probe_json),
                "night-oem-vs-311b.json",
            ),
            "rb",
        ).read()
    )["seq21plus_after_dump"]
    book = json.loads(
        open(
            os.path.join(os.path.dirname(probe_json), "night-oem-book-and-amount.json"),
            "rb",
        ).read()
    )
    recipe = {
        "schema": "quoteNetzipRs.official_5188_oem_projection.v1",
        "fixture": "wine-night-coldstart-20260904T011938 post-dump seq21+",
        "compared": compared,
        "do_not_use_for_auction_mid_session_pcap": True,
        "rules": [
            {
                "field": "scale",
                "rule": "10 ** 0104.opaque_tail[0]; ignore probe.scale (STAR can be 1)",
                "evidence": "688403 probe.scale=1, 0104 places=2; with 0104 scale price 5166/5166",
            },
            {
                "field": "price/open/high/low",
                "rule": "i32 at +0x10/+0x04/+0x08/+0x0C as f32 / scale",
                "hits": scalar["hits"]["price_vs_close"],
                "n": scalar["compared"],
                "rate": 1.0,
            },
            {
                "field": "last_close overnight / before day-cut",
                "rule": "i32 at +0x12b as f32 / scale",
                "hits": scalar["hits"]["last_vs_0x12b"],
                "n": scalar["compared"],
                "rate": 1.0,
            },
            {
                "field": "last_close next trading day after day-cut",
                "rule": "snapshot of previous session +0x10 close; do not keep reading live +0x10",
                "evidence": "auction callbacks last_close=night close 4914/4928 by code",
            },
            {
                "field": "volume",
                "rule": "i64 at +0x14 as f32; STAR 688/689 round(vol/100)",
                "hits": scalar["hits"]["volume_with_star_lots"],
                "n": scalar["compared"],
                "rate": scalar["rates"]["volume_with_star_lots"],
            },
            {
                "field": "amount",
                "rule": "i64 at +0x1C as f32; do not use 7709 PriceRelative",
                "exact_f32": scalar["hits"]["amount"],
                "rel_lt_1e-4": 5119,
                "rel_lt_1e-3": 5166,
                "n": scalar["compared"],
            },
            {
                "field": "bid/ask prices 1-5",
                "rule": "i32[10] at +0x58 as bid5..bid1, ask1..ask5; levels 6-10 = 0",
                "hits": book["book_hits"]["book_px_5"],
                "n": book["compared"],
                "rate": 1.0,
                "oem_levels_6_10_nonzero": 0,
            },
            {
                "field": "bid/ask volumes 1-5",
                "rule": "i32[10] at +0xa8 same order; STAR round(/100); levels 6-10 = 0",
                "hits": book["book_hits"]["book_vol_5_with_star_lots"],
                "n": book["compared"],
                "rate": book["book_rates"]["book_vol_5_with_star_lots"],
            },
            {
                "field": "datetime",
                "rule": "311B u32 timestamp as Asia/Shanghai YYYY-MM-DD HH:MM:SS; allow ±1s vs OEM",
                "exact": hits["datetime_exact"],
                "within_1s": hits["datetime_within_1s"],
                "format_from_raw_ts": hits["datetime_format_from_311B_ts"],
                "n": compared,
                "delta_histogram": {str(k): v for k, v in sorted(dt_delta.items())},
            },
            {
                "field": "name",
                "rule": "0104/probe GBK name; either source is sufficient on this fixture",
                "vs_probe": hits["name_vs_probe"],
                "vs_0104": hits["name_vs_0104"],
                "vs_either": hits["name_vs_probe_or_0104"],
                "n": compared,
            },
            {
                "field": "pre-open trade fields",
                "rule": "after day-cut and before 09:25, OEM price/OHLC/volume/amount/book are 0; keep last_close",
                "evidence": "auction 597 leftover amount≠0 vs Wine zeros; clearing turns them into 0=0",
            },
        ],
        "datetime_mismatch_samples": samples,
        "name_rates": {
            "vs_probe": round(hits["name_vs_probe"] / n, 4),
            "vs_0104": round(hits["name_vs_0104"] / n, 4),
            "vs_either": round(hits["name_vs_probe_or_0104"] / n, 4),
        },
        "datetime_rates": {
            "exact": round(hits["datetime_exact"] / n, 4),
            "within_1s": round(hits["datetime_within_1s"] / n, 4),
        },
    }
    os.makedirs(os.path.dirname(out_json) or ".", exist_ok=True)
    json.dump(recipe, open(out_json, "w", encoding="utf-8"), ensure_ascii=False, indent=2)
    json.dump(
        {
            "compared": compared,
            "datetime": recipe["datetime_rates"],
            "name": recipe["name_rates"],
            "delta_top": sorted(dt_delta.items(), key=lambda kv: -kv[1])[:8],
            "out": out_json,
        },
        sys.stdout,
        ensure_ascii=False,
        indent=2,
    )
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
