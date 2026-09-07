#!/usr/bin/env python3
"""Ablate 311B -> OEM_REPORT compare without editing the owned parity example.

usage:
  ablate-oem-projection.py EXTRACT_DIR CALLBACK_JSONL METADATA_DIR [WINDOW_MS]

Does not decode 2704. It only re-joins already-decoded 311-byte records to
Wine quote_batch callbacks and scores:
  - unmatched reasons
  - bid1/ask1 vs five-level prefix vs ten-level full arrays
  - amount/volume integer vs callback under several divisors
"""
from __future__ import annotations

import json
import os
import struct
import sys
from collections import Counter, defaultdict

WINDOW_MS_DEFAULT = 250


def i32(buf: bytes, off: int) -> int:
    return struct.unpack_from("<i", buf, off)[0]


def i64(buf: bytes, off: int) -> int:
    return struct.unpack_from("<q", buf, off)[0]


def f32_bits(value: float) -> int:
    return struct.unpack("<I", struct.pack("<f", float(value)))[0]


def same_f32(projected: float, callback: float) -> bool:
    return f32_bits(projected) == f32_bits(callback)


def same_prefix(proj: list[float], cb: list[float], n: int) -> bool:
    return all(same_f32(proj[i], cb[i]) for i in range(n))


def parse_callback_ts(value: str) -> int | None:
    if len(value) != 19 or value[4] != "-" or value[7] != "-" or value[10] != " ":
        return None
    try:
        y, m, d = int(value[0:4]), int(value[5:7]), int(value[8:10])
        hh, mm, ss = int(value[11:13]), int(value[14:16]), int(value[17:19])
    except ValueError:
        return None
    year = y - int(m <= 2)
    era = year // 400
    yoe = year - era * 400
    shifted = m + (-3 if m > 2 else 9)
    doy = (153 * shifted + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    days = era * 146_097 + doe - 719_468
    seconds = days * 86400 + hh * 3600 + mm * 60 + ss - 8 * 3600
    if seconds < 0:
        return None
    return seconds


def load_metadata(directory: str) -> dict[tuple[str, int], tuple[str, str, float]]:
    out: dict[tuple[str, int], tuple[str, str, float]] = {}
    for name in os.listdir(directory):
        if not name.endswith("0104.code-table.json"):
            continue
        table = json.loads(open(os.path.join(directory, name), "rb").read())
        market = bytes(table["market"]).decode()
        for rec in table["records"]:
            if not rec.get("code"):
                continue
            tail = rec["opaque_tail"]
            places = tail[0] if isinstance(tail, list) else tail[0]
            if places > 6:
                continue
            scale = float(10**places)
            out[(market, rec["symbol_index"])] = (rec["code"], rec.get("name") or "", scale)
    return out


def load_decoded(extract_dir: str):
    manifest = json.loads(open(os.path.join(extract_dir, "manifest.json"), "rb").read())
    rows = []
    needed = set()
    for entry in manifest:
        if entry.get("wire_kind") != "2704" or not entry.get("decoded_values_file"):
            continue
        path = os.path.join(extract_dir, entry["decoded_values_file"])
        if not os.path.isfile(path):
            continue
        completed = entry.get("completed_at_micros")
        if completed is None:
            continue
        values = json.loads(open(path, "rb").read())
        for value in values:
            idx = value["index"]
            market = bytes(idx["market"]).decode()
            rec = bytes(value["record"])
            if len(rec) < 0x137:
                continue
            rows.append((completed, market, idx["symbol_index"], idx["timestamp"], rec))
            needed.add((market, idx["symbol_index"]))
    return rows, needed


def stream_quotes(callback_jsonl: str, wanted_codes: set[tuple[str, str]]):
    by_key: dict[tuple[str, str], list] = defaultdict(list)
    batches = 0
    seen = set()
    with open(callback_jsonl, "r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            event = json.loads(line)
            seq = event.get("sequence")
            if seq in seen:
                continue
            seen.add(seq)
            batch = event.get("quote_batch")
            if not batch or batch.get("schema") != "quoteNetzipWine.quote_batch.v1":
                continue
            batches += 1
            ts_ms = event["timestamp_ms"]
            for q in batch.get("quotes") or []:
                key = (q["market"], q["code"])
                if key not in wanted_codes:
                    continue
                by_key[key].append(
                    (
                        ts_ms,
                        seq,
                        q.get("datetime") or "",
                        float(q.get("price") or 0),
                        float(q.get("last_close") or 0),
                        float(q.get("open") or 0),
                        float(q.get("high") or 0),
                        float(q.get("low") or 0),
                        float(q.get("volume") or 0),
                        float(q.get("amount") or 0),
                        [float(x) for x in q.get("bid_prices") or [0] * 10],
                        [float(x) for x in q.get("ask_prices") or [0] * 10],
                        [float(x) for x in q.get("bid_volumes") or [0] * 10],
                        [float(x) for x in q.get("ask_volumes") or [0] * 10],
                    )
                )
    for quotes in by_key.values():
        quotes.sort(key=lambda item: (item[0], item[1]))
    return by_key, batches


def nearest(quotes, frame_micros: int, internal_ts: int, window_ms: int):
    window_micros = window_ms * 1000
    lower_ms = max(frame_micros - window_micros, 0) // 1000
    upper_ms = max(frame_micros + window_micros, 0) // 1000
    best = None
    best_key = None
    for q in quotes:
        ts_ms = q[0]
        if ts_ms < lower_ms:
            continue
        if ts_ms > upper_ms:
            break
        delta = ts_ms * 1000 - frame_micros
        if abs(delta) > window_micros:
            continue
        cb_ts = parse_callback_ts(q[2])
        ts_diff = abs(cb_ts - internal_ts) if cb_ts is not None else 2**64 - 1
        key = (ts_diff, abs(delta), ts_ms, q[1])
        if best_key is None or key < best_key:
            best_key = key
            best = (q, delta)
    return best


def score_rows(rows, quotes, metadata, window_ms: int) -> dict:
    stats = Counter()
    amount_div = Counter()
    volume_div = Counter()
    divisors = (1, 10, 100, 1000, 10000, 100000)
    last_close_zero_cb = 0
    price_zero_both = 0
    amount_zero_both = 0
    volume_zero_both = 0
    bid1_zero_both = 0
    bid1_internal_nz = 0
    bid1_callback_nz = 0
    last_close_ratio = Counter()
    oem_tail_nonzero = Counter()

    for completed, market, index, internal_ts, rec in rows:
        stats["decoded"] += 1
        meta = metadata.get((market, index))
        if meta is None:
            stats["missing_metadata"] += 1
            continue
        code, _name, scale = meta
        hit = nearest(quotes.get((market, code), []), completed, internal_ts, window_ms)
        if hit is None:
            stats["no_window"] += 1
            continue
        q, _delta = hit
        stats["matched"] += 1
        (
            _ts_ms,
            _seq,
            datetime,
            cb_price,
            cb_last,
            cb_open,
            cb_high,
            cb_low,
            cb_vol,
            cb_amt,
            cb_bp,
            cb_ap,
            cb_bv,
            cb_av,
        ) = q
        last_price = i32(rec, 0x10)
        last_close = i32(rec, 0x12B)
        open_p = i32(rec, 0x04)
        high_p = i32(rec, 0x08)
        low_p = i32(rec, 0x0C)
        volume = i64(rec, 0x14)
        amount = i64(rec, 0x1C)
        ladder_p = [i32(rec, 0x58 + i * 4) for i in range(10)]
        ladder_v = [i32(rec, 0xA8 + i * 4) for i in range(10)]
        bid_p = [(ladder_p[4 - i] / scale if i < 5 else 0.0) for i in range(10)]
        ask_p = [(ladder_p[5 + i] / scale if i < 5 else 0.0) for i in range(10)]
        bid_v = [float(ladder_v[4 - i] if i < 5 else 0) for i in range(10)]
        ask_v = [float(ladder_v[5 + i] if i < 5 else 0) for i in range(10)]

        if same_f32(last_price / scale, cb_price):
            stats["price"] += 1
        if same_f32(last_close / scale, cb_last):
            stats["last_close"] += 1
        if same_f32(open_p / scale, cb_open):
            stats["open"] += 1
        if same_f32(high_p / scale, cb_high):
            stats["high"] += 1
        if same_f32(low_p / scale, cb_low):
            stats["low"] += 1
        if same_f32(float(volume), cb_vol):
            stats["volume_as_int"] += 1
        if same_f32(float(amount), cb_amt):
            stats["amount_as_int"] += 1
        cb_ts = parse_callback_ts(datetime)
        if cb_ts == internal_ts:
            stats["timestamp"] += 1
        rec_ts = i32(rec, 0x00) & 0xFFFFFFFF
        if cb_ts == rec_ts:
            stats["timestamp_311b"] += 1
        if last_price == 0 and cb_price == 0:
            price_zero_both += 1
        if amount == 0 and cb_amt == 0:
            amount_zero_both += 1
        if volume == 0 and cb_vol == 0:
            volume_zero_both += 1
        if cb_last == 0:
            last_close_zero_cb += 1
        if last_close != 0 and cb_last != 0:
            ratio = (last_close / scale) / cb_last
            if 0.99 <= ratio <= 1.01:
                last_close_ratio["~1"] += 1
            elif 9.9 <= ratio <= 10.1:
                last_close_ratio["~10"] += 1
            elif 99 <= ratio <= 101:
                last_close_ratio["~100"] += 1
            elif 0.099 <= ratio <= 0.101:
                last_close_ratio["~0.1"] += 1
            else:
                last_close_ratio["other"] += 1

        if bid_p[0] != 0:
            bid1_internal_nz += 1
        if cb_bp[0] != 0:
            bid1_callback_nz += 1
        if bid_p[0] == 0 and cb_bp[0] == 0:
            bid1_zero_both += 1
        if same_f32(bid_p[0], cb_bp[0]):
            stats["bid1"] += 1
        if same_f32(ask_p[0], cb_ap[0]):
            stats["ask1"] += 1
        if same_prefix(bid_p, cb_bp, 5):
            stats["bid_px_5"] += 1
        if same_prefix(ask_p, cb_ap, 5):
            stats["ask_px_5"] += 1
        if same_prefix(bid_v, cb_bv, 5):
            stats["bid_vol_5"] += 1
        if same_prefix(ask_v, cb_av, 5):
            stats["ask_vol_5"] += 1
        if same_prefix(bid_p, cb_bp, 10):
            stats["bid_px_10"] += 1
        if same_prefix(ask_p, cb_ap, 10):
            stats["ask_px_10"] += 1

        if any(abs(x) > 0 for x in cb_bp[5:]):
            oem_tail_nonzero["bid_px_6_10"] += 1
        if any(abs(x) > 0 for x in cb_ap[5:]):
            oem_tail_nonzero["ask_px_6_10"] += 1
        if any(abs(x) > 0 for x in cb_bv[5:]):
            oem_tail_nonzero["bid_vol_6_10"] += 1
        if any(abs(x) > 0 for x in cb_av[5:]):
            oem_tail_nonzero["ask_vol_6_10"] += 1

        for div in divisors:
            if amount != 0 and same_f32(amount / div, cb_amt):
                amount_div[div] += 1
            if volume != 0 and same_f32(volume / div, cb_vol):
                volume_div[div] += 1

    n = stats["matched"] or 1
    return {
        "window_ms": window_ms,
        "counts": dict(stats),
        "matched_rates": {k: round(stats[k] / n, 4) for k in [
            "price", "last_close", "open", "high", "low", "volume_as_int",
            "amount_as_int", "timestamp", "bid1", "ask1", "bid_px_5", "ask_px_5",
            "bid_vol_5", "ask_vol_5", "bid_px_10", "ask_px_10",
        ]},
        "price_zero_both": price_zero_both,
        "amount_zero_both": amount_zero_both,
        "volume_zero_both": volume_zero_both,
        "bid1_zero_both": bid1_zero_both,
        "bid1_internal_nonzero": bid1_internal_nz,
        "bid1_callback_nonzero": bid1_callback_nz,
        "last_close_callback_zero": last_close_zero_cb,
        "last_close_ratio_when_both_nonzero": dict(last_close_ratio),
        "oem_levels_6_10_nonzero": dict(oem_tail_nonzero),
        "amount_divisor_hits_nonzero_internal": dict(amount_div),
        "volume_divisor_hits_nonzero_internal": dict(volume_div),
        "nonzero_price_hits": stats["price"] - price_zero_both,
        "nonzero_amount_hits": stats["amount_as_int"] - amount_zero_both,
        "nonzero_volume_hits": stats["volume_as_int"] - volume_zero_both,
    }


def main() -> int:
    if len(sys.argv) < 4:
        print(
            "usage: ablate-oem-projection.py EXTRACT_DIR CALLBACK_JSONL METADATA_DIR "
            "[WINDOW_MS[,WINDOW_MS...]] [MIN_DATETIME]",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, metadata_dir = sys.argv[1:4]
    windows = [
        int(part)
        for part in (sys.argv[4] if len(sys.argv) > 4 else str(WINDOW_MS_DEFAULT)).split(",")
        if part
    ]
    min_datetime = sys.argv[5] if len(sys.argv) > 5 else None
    min_ts = parse_callback_ts(min_datetime) if min_datetime else None
    metadata = load_metadata(metadata_dir)
    rows, needed_idx = load_decoded(extract_dir)
    if min_ts is not None:
        rows = [row for row in rows if row[3] >= min_ts]
    wanted_codes = set()
    for market, index in needed_idx:
        meta = metadata.get((market, index))
        if meta:
            wanted_codes.add((market, meta[0]))
    quotes, n_batches = stream_quotes(callback_jsonl, wanted_codes)
    if min_ts is not None:
        min_ms = min_ts * 1000
        filtered = {}
        for key, items in quotes.items():
            kept = [item for item in items if item[0] >= min_ms]
            if kept:
                filtered[key] = kept
        quotes = filtered
    report = {
        "extract_dir": extract_dir,
        "callback_jsonl": callback_jsonl,
        "metadata_dir": metadata_dir,
        "min_datetime": min_datetime,
        "callback_batches": n_batches,
        "decoded_after_filter": len(rows),
        "windows": [score_rows(rows, quotes, metadata, window_ms) for window_ms in windows],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
