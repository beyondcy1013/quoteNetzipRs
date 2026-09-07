#!/usr/bin/env python3
"""Same-session night OEM vs 311B dump. Auction live-914 cannot do this.

Seq 16 is pre-dump stale public state (2026-07-24). Seq 21+ are after the
initial 2704 dump (datetime 2026-09-03 15:00). Compare latest post-dump OEM
to probe-decoded 311B using several last_close getters.

usage:
  night-oem-vs-311b.py CALLBACK_JSONL PROBE_JSON NIGHT_0104_DIR
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


def bucket(err: float | None) -> str:
    if err is None:
        return "undefined"
    if err == 0:
        return "0"
    if err < 1e-6:
        return "<1e-6"
    if err < 1e-4:
        return "<1e-4"
    if err < 1e-3:
        return "<1e-3"
    if err < 1e-2:
        return "<1e-2"
    if err < 0.05:
        return "<5%"
    return ">=5%"


def load_latest_oem(path: str, *, min_seq: int) -> tuple[dict, Counter]:
    latest: dict[tuple[str, str], dict] = {}
    batch_n = Counter()
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch or batch.get("schema") != "quoteNetzipWine.quote_batch.v1":
                continue
            seq = int(ev.get("sequence") or 0)
            if seq < min_seq:
                continue
            quotes = batch.get("quotes") or []
            batch_n[seq] = len(quotes)
            for q in quotes:
                key = (q["market"], q["code"])
                latest[key] = {
                    "seq": seq,
                    "datetime": q.get("datetime") or "",
                    "price": float(q.get("price") or 0),
                    "last_close": float(q.get("last_close") or 0),
                    "open": float(q.get("open") or 0),
                    "high": float(q.get("high") or 0),
                    "low": float(q.get("low") or 0),
                    "volume": float(q.get("volume") or 0),
                    "amount": float(q.get("amount") or 0),
                    "bid1": float((q.get("bid_prices") or [0])[0] or 0),
                    "ask1": float((q.get("ask_prices") or [0])[0] or 0),
                    "bid_p": [float(x) for x in (q.get("bid_prices") or [0] * 10)[:5]],
                    "ask_p": [float(x) for x in (q.get("ask_prices") or [0] * 10)[:5]],
                }
    return latest, batch_n


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: night-oem-vs-311b.py CALLBACK PROBE NIGHT_0104", file=sys.stderr)
        return 2
    callback_jsonl, probe_json, night_0104 = sys.argv[1:]
    probe_rows = json.loads(open(probe_json, "rb").read())
    probe = {}
    for row in probe_rows:
        code = row.get("code") or ""
        market = row.get("market") or ""
        if not code:
            continue
        key = (market, code)
        prev = probe.get(key)
        if prev is None or int(row.get("frame_index") or 0) >= int(prev.get("frame_index") or 0):
            probe[key] = row
    night_meta = seed.load_largest_0104(night_0104)
    seq16 = {}
    post = {}
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
        for line in fh:
            ev = json.loads(line)
            batch = ev.get("quote_batch")
            if not batch:
                continue
            seq = int(ev.get("sequence") or 0)
            for q in batch.get("quotes") or []:
                key = (q["market"], q["code"])
                rec = {
                    "seq": seq,
                    "datetime": q.get("datetime") or "",
                    "price": float(q.get("price") or 0),
                    "last_close": float(q.get("last_close") or 0),
                    "open": float(q.get("open") or 0),
                    "high": float(q.get("high") or 0),
                    "low": float(q.get("low") or 0),
                    "volume": float(q.get("volume") or 0),
                    "amount": float(q.get("amount") or 0),
                    "bid1": float((q.get("bid_prices") or [0])[0] or 0),
                    "ask1": float((q.get("ask_prices") or [0])[0] or 0),
                }
                if seq == 16:
                    seq16[key] = rec
                elif seq >= 21:
                    post[key] = rec

    def score(oem_map: dict, label: str) -> dict:
        hits = Counter()
        last_src = Counter()
        samples = []
        for key, row in probe.items():
            oem = oem_map.get(key)
            if oem is None:
                hits["probe_without_oem"] += 1
                continue
            meta = night_meta.get(key)
            scale = float((meta["scale"] if meta else 0) or row.get("scale") or 0)
            if scale <= 0:
                hits["no_scale"] += 1
                continue
            hits["compared"] += 1
            px = seed.rust_scaled(int(row["close"]), scale)
            last_12b = seed.rust_scaled(int(row["last_close"]), scale)
            last_close_field = px  # +0x10 as last
            open_ = seed.rust_scaled(int(row["open"]), scale)
            high = seed.rust_scaled(int(row["high"]), scale)
            low = seed.rust_scaled(int(row["low"]), scale)
            vol = seed.to_f32(int(row["volume"]))
            star_vol = seed.to_f32(round(int(row["volume"]) / 100.0))
            amt = seed.to_f32(int(row["amount"]))
            bid1 = seed.rust_scaled(int(row["bid1"]), scale)
            ask1 = seed.rust_scaled(int(row["ask1"]), scale)
            pairs = {
                "price_vs_close": (px, oem["price"]),
                "last_vs_0x12b": (last_12b, oem["last_close"]),
                "last_vs_close": (last_close_field, oem["last_close"]),
                "open": (open_, oem["open"]),
                "high": (high, oem["high"]),
                "low": (low, oem["low"]),
                "volume": (vol, oem["volume"]),
                "amount": (amt, oem["amount"]),
                "bid1": (bid1, oem["bid1"]),
                "ask1": (ask1, oem["ask1"]),
            }
            for name, (left, right) in pairs.items():
                if seed.same_f32(left, right):
                    hits[name] += 1
            if seed.same_f32(vol, oem["volume"]) or (
                key[1].startswith(("688", "689")) and seed.same_f32(star_vol, oem["volume"])
            ):
                hits["volume_with_star_lots"] += 1
            if seed.same_f32(last_12b, oem["last_close"]):
                last_src["0x12b"] += 1
            elif seed.same_f32(last_close_field, oem["last_close"]):
                last_src["close_+0x10"] += 1
            else:
                last_src["neither"] += 1
            hits["amount_rel_" + bucket(rel_err(amt, oem["amount"]))] += 1
            hits["price_rel_" + bucket(rel_err(px, oem["price"]))] += 1
            if len(samples) < 8 and not seed.same_f32(amt, oem["amount"]):
                samples.append(
                    {
                        "code": key[1],
                        "oem_seq": oem["seq"],
                        "oem_dt": oem["datetime"],
                        "close": row["close"],
                        "last_close_311": row["last_close"],
                        "proj_price": px,
                        "oem_price": oem["price"],
                        "proj_last_12b": last_12b,
                        "proj_last_close": last_close_field,
                        "oem_last": oem["last_close"],
                        "proj_amount": amt,
                        "oem_amount": oem["amount"],
                        "proj_vol": vol,
                        "oem_vol": oem["volume"],
                        "proj_bid1": bid1,
                        "oem_bid1": oem["bid1"],
                    }
                )
        n = hits["compared"] or 1
        rate = {
            k: round(hits[k] / n, 4)
            for k in (
                "price_vs_close",
                "last_vs_0x12b",
                "last_vs_close",
                "open",
                "high",
                "low",
                "volume",
                "volume_with_star_lots",
                "amount",
                "bid1",
                "ask1",
            )
        }
        return {
            "label": label,
            "oem_codes": len(oem_map),
            "probe_codes": len(probe),
            "compared": hits["compared"],
            "probe_without_oem": hits["probe_without_oem"],
            "scale": "0104 opaque_tail[0] as 10**places; probe.scale is unsafe for STAR",
            "hits": {k: hits[k] for k in rate},
            "rates": rate,
            "last_close_source": dict(last_src),
            "amount_rel": {k[11:]: hits[k] for k in hits if k.startswith("amount_rel_")},
            "price_rel": {k[10:]: hits[k] for k in hits if k.startswith("price_rel_")},
            "amount_mismatch_samples": samples,
        }

    sh603 = {
        "probe": {
            k: probe[("SH", "603059")][k]
            for k in ("close", "last_close", "amount", "volume", "bid1", "ask1", "ts")
        }
        if ("SH", "603059") in probe
        else None,
        "seq16": seq16.get(("SH", "603059")),
        "post_dump": post.get(("SH", "603059")),
    }
    report = {
        "callback_jsonl": callback_jsonl,
        "probe_json": probe_json,
        "sh603059": sh603,
        "seq16_pre_dump_stale_july24": score(seq16, "seq16 stale OEM before 2704 dump"),
        "seq21plus_after_dump": score(post, "seq21+ latest OEM after 2704 dump"),
        "interpretation": (
            "Seq16 is stale 2026-07-24 public state before the dump. Seq21+ is the "
            "post-dump OEM. Overnight last_close is 311B 0x12b; price is +0x10 close. "
            "Next-day auction last_close is the snapshotted previous close. STAR 688/689 "
            "OEM volume is internal/100 (lots). Amount is i64 as f32 within 1e-4."
        ),
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
