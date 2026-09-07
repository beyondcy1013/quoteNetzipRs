#!/usr/bin/env python3
"""Join decoded 311B records to Wine callbacks by business timestamp.

Does not decode 2704 and does not edit the owned parity example.
Primary key: (market, code, internal timestamp == callback datetime).
Wall-clock is only a tie-break among same-timestamp candidates, never a gate.

usage:
  join-by-business-timestamp.py EXTRACT_DIR CALLBACK_JSONL METADATA_DIR
"""
from __future__ import annotations

import json
import math
import os
import struct
import sys
from collections import Counter, defaultdict

WINDOW_MS_CONTROL = 250


def i32(buf: bytes, off: int) -> int:
    return struct.unpack_from("<i", buf, off)[0]


def i64(buf: bytes, off: int) -> int:
    return struct.unpack_from("<q", buf, off)[0]


def to_f32(value: float) -> float:
    return struct.unpack("<f", struct.pack("<f", float(value)))[0]


def f32_bits(value: float) -> int:
    return struct.unpack("<I", struct.pack("<f", float(value)))[0]


def same_f32(projected: float, callback: float) -> bool:
    return f32_bits(projected) == f32_bits(callback)


def rust_scaled(value: int, scale: float) -> float:
    """Match official_5188_callback_parity: (i32 as f32) / scale_f32, then f32."""
    return to_f32(to_f32(value) / to_f32(scale))


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
                datetime = q.get("datetime") or ""
                cb_ts = parse_callback_ts(datetime)
                by_key[key].append(
                    (
                        ts_ms,
                        seq,
                        datetime,
                        cb_ts,
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


def nearest_wall(quotes, frame_micros: int, internal_ts: int, window_ms: int):
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
        cb_ts = q[3]
        ts_diff = abs(cb_ts - internal_ts) if cb_ts is not None else 2**64 - 1
        key = (ts_diff, abs(delta), ts_ms, q[1])
        if best_key is None or key < best_key:
            best_key = key
            best = (q, delta)
    return best


def pick_exact_ts(index, internal_ts: int, frame_micros: int):
    candidates = index.get(internal_ts)
    if not candidates:
        return None, 0
    best = None
    best_key = None
    for q in candidates:
        delta = q[0] * 1000 - frame_micros
        key = (abs(delta), q[0], q[1])
        if best_key is None or key < best_key:
            best_key = key
            best = (q, delta)
    return best, len(candidates)


def percentile(sorted_vals: list[int], p: float) -> int | None:
    if not sorted_vals:
        return None
    if len(sorted_vals) == 1:
        return sorted_vals[0]
    rank = (len(sorted_vals) - 1) * p
    lo = int(math.floor(rank))
    hi = int(math.ceil(rank))
    if lo == hi:
        return sorted_vals[lo]
    weight = rank - lo
    return int(round(sorted_vals[lo] * (1 - weight) + sorted_vals[hi] * weight))


def score_quote(rec: bytes, q, scale: float) -> dict:
    last_price = i32(rec, 0x10)
    last_close = i32(rec, 0x12B)
    open_p = i32(rec, 0x04)
    high_p = i32(rec, 0x08)
    low_p = i32(rec, 0x0C)
    volume = i64(rec, 0x14)
    amount = i64(rec, 0x1C)
    ladder_p = [i32(rec, 0x58 + i * 4) for i in range(10)]
    ladder_v = [i32(rec, 0xA8 + i * 4) for i in range(10)]
    bid1 = rust_scaled(ladder_p[4], scale)
    ask1 = rust_scaled(ladder_p[5], scale)
    cb_price = q[4]
    cb_last = q[5]
    cb_open = q[6]
    cb_high = q[7]
    cb_low = q[8]
    cb_vol = q[9]
    cb_amt = q[10]
    cb_bp = q[11]
    cb_ap = q[12]
    proj_price = rust_scaled(last_price, scale)
    proj_last = rust_scaled(last_close, scale)
    last_ratio = None
    if last_close != 0 and cb_last != 0:
        last_ratio = (last_close / scale) / cb_last
    return {
        "price": same_f32(proj_price, cb_price),
        "price_zero_both": last_price == 0 and cb_price == 0,
        "last_close": same_f32(proj_last, cb_last),
        "last_close_rel_1pct": last_ratio is not None and 0.99 <= last_ratio <= 1.01,
        "last_close_rel_1e4": last_ratio is not None and abs(last_ratio - 1) <= 1e-4,
        "open": same_f32(rust_scaled(open_p, scale), cb_open),
        "high": same_f32(rust_scaled(high_p, scale), cb_high),
        "low": same_f32(rust_scaled(low_p, scale), cb_low),
        "volume": same_f32(float(volume), cb_vol),
        "volume_zero_both": volume == 0 and cb_vol == 0,
        "amount": same_f32(float(amount), cb_amt),
        "amount_zero_both": amount == 0 and cb_amt == 0,
        "amount_zero_internal": amount == 0,
        "bid1": same_f32(bid1, cb_bp[0]),
        "ask1": same_f32(ask1, cb_ap[0]),
        "bid1_internal_nz": bid1 != 0,
        "bid1_callback_nz": cb_bp[0] != 0,
        "bid1_both_nz": bid1 != 0 and cb_bp[0] != 0,
        "last_ratio_bucket": (
            "~1"
            if last_ratio is not None and 0.99 <= last_ratio <= 1.01
            else "~10"
            if last_ratio is not None and 9.9 <= last_ratio <= 10.1
            else "~100"
            if last_ratio is not None and 99 <= last_ratio <= 101
            else "~0.1"
            if last_ratio is not None and 0.099 <= last_ratio <= 0.101
            else "internal_zero"
            if last_close == 0
            else "other"
        ),
    }


def accumulate(stats: Counter, scored: dict) -> None:
    stats["matched"] += 1
    for key in (
        "price",
        "last_close",
        "last_close_rel_1pct",
        "last_close_rel_1e4",
        "open",
        "high",
        "low",
        "volume",
        "amount",
        "bid1",
        "ask1",
    ):
        if scored[key]:
            stats[key] += 1
    if scored["price"] and not scored["price_zero_both"]:
        stats["price_nonzero"] += 1
    if scored["price_zero_both"]:
        stats["price_zero_both"] += 1
    if scored["volume"] and not scored["volume_zero_both"]:
        stats["volume_nonzero"] += 1
    if scored["volume_zero_both"]:
        stats["volume_zero_both"] += 1
    if scored["amount"] and not scored["amount_zero_both"]:
        stats["amount_nonzero"] += 1
    if scored["amount_zero_both"]:
        stats["amount_zero_both"] += 1
    if scored["amount_zero_internal"]:
        stats["carried_forward_amount0"] += 1
    if scored["bid1_internal_nz"]:
        stats["bid1_internal_nz"] += 1
    if scored["bid1_callback_nz"]:
        stats["bid1_callback_nz"] += 1
    if scored["bid1_both_nz"]:
        stats["bid1_both_nz"] += 1
        if scored["bid1"]:
            stats["bid1_both_nz_hit"] += 1
    stats[f"last_ratio_{scored['last_ratio_bucket']}"] += 1


def summarize(stats: Counter, extra: dict | None = None) -> dict:
    n = stats["matched"] or 1
    out = {
        "counts": dict(stats),
        "nonzero_price_hits": stats["price_nonzero"],
        "nonzero_amount_hits": stats["amount_nonzero"],
        "nonzero_volume_hits": stats["volume_nonzero"],
        "bid1_both_nonzero_hit_rate": round(
            stats["bid1_both_nz_hit"] / stats["bid1_both_nz"], 4
        )
        if stats["bid1_both_nz"]
        else None,
        "rates_on_matched": {
            k: round(stats[k] / n, 4)
            for k in (
                "price",
                "last_close",
                "last_close_rel_1pct",
                "last_close_rel_1e4",
                "open",
                "high",
                "low",
                "volume",
                "amount",
                "bid1",
                "ask1",
                "timestamp_exact",
            )
            if k in stats or True
        },
    }
    if extra:
        out.update(extra)
    return out


def main() -> int:
    if len(sys.argv) < 4:
        print(
            "usage: join-by-business-timestamp.py EXTRACT_DIR CALLBACK_JSONL METADATA_DIR",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, metadata_dir = sys.argv[1:4]
    metadata = load_metadata(metadata_dir)
    rows, needed_idx = load_decoded(extract_dir)
    wanted_codes = set()
    for market, index in needed_idx:
        meta = metadata.get((market, index))
        if meta:
            wanted_codes.add((market, meta[0]))
    quotes, n_batches = stream_quotes(callback_jsonl, wanted_codes)

    by_ts: dict[tuple[str, str], dict[int, list]] = {}
    callback_ts_dup = Counter()
    callback_unique_ts = 0
    for key, items in quotes.items():
        grouped: dict[int, list] = defaultdict(list)
        for q in items:
            if q[3] is None:
                continue
            grouped[q[3]].append(q)
        by_ts[key] = grouped
        for ts, group in grouped.items():
            callback_unique_ts += 1
            if len(group) > 1:
                callback_ts_dup["groups"] += 1
                callback_ts_dup["extra_copies"] += len(group) - 1

    decoded_ts_dup = Counter()
    seen_decoded_ts = Counter()
    for _completed, market, index, internal_ts, _rec in rows:
        meta = metadata.get((market, index))
        if meta is None:
            continue
        seen_decoded_ts[(market, meta[0], internal_ts)] += 1
    for count in seen_decoded_ts.values():
        if count > 1:
            decoded_ts_dup["groups"] += 1
            decoded_ts_dup["extra_copies"] += count - 1

    join_stats = Counter()
    wall_stats = Counter()
    both_stats = Counter()
    exact_only_stats = Counter()
    wall_only_stats = Counter()
    amount_nz_stats = Counter()
    amount_z_stats = Counter()
    crosstab = Counter()
    abs_deltas_ms = []
    exact_within_250 = 0
    rec_ts_exact = 0
    no_code_callbacks = 0
    post = {
        "join": Counter(),
        "wall": Counter(),
        "crosstab": Counter(),
        "abs_deltas_ms": [],
    }
    post_cut = parse_callback_ts("2026-09-04 09:25:00")

    for completed, market, index, internal_ts, rec in rows:
        join_stats["decoded"] += 1
        wall_stats["decoded"] += 1
        meta = metadata.get((market, index))
        if meta is None:
            join_stats["missing_metadata"] += 1
            wall_stats["missing_metadata"] += 1
            continue
        code, _name, scale = meta
        key = (market, code)
        items = quotes.get(key, [])
        if not items:
            no_code_callbacks += 1
            join_stats["no_code_callbacks"] += 1
            continue

        exact_hit, n_cand = pick_exact_ts(by_ts.get(key, {}), internal_ts, completed)
        wall_hit = nearest_wall(items, completed, internal_ts, WINDOW_MS_CONTROL)
        rec_ts = i32(rec, 0x00) & 0xFFFFFFFF
        if rec_ts == internal_ts:
            rec_ts_exact += 0  # counted below per match
        after_cut = post_cut is not None and internal_ts >= post_cut

        if exact_hit is None:
            join_stats["no_exact_ts"] += 1
        else:
            q, delta = exact_hit
            scored = score_quote(rec, q, scale)
            scored["timestamp_exact"] = True
            join_stats["timestamp_exact"] += 1
            join_stats["candidate_groups"] += 1
            if n_cand > 1:
                join_stats["multi_candidate_ts"] += 1
            abs_ms = abs(delta) // 1000
            abs_deltas_ms.append(abs_ms)
            if abs_ms <= WINDOW_MS_CONTROL:
                exact_within_250 += 1
            if rec_ts == parse_callback_ts(q[2]):
                rec_ts_exact += 1
            accumulate(join_stats, scored)
            if scored["amount_zero_internal"]:
                accumulate(amount_z_stats, scored)
                amount_z_stats["timestamp_exact"] += 1
            else:
                accumulate(amount_nz_stats, scored)
                amount_nz_stats["timestamp_exact"] += 1
            if after_cut:
                post["join"]["timestamp_exact"] += 1
                accumulate(post["join"], scored)
                post["abs_deltas_ms"].append(abs_ms)

        if wall_hit is None:
            wall_stats["no_window"] += 1
        else:
            q, delta = wall_hit
            scored = score_quote(rec, q, scale)
            cb_ts = q[3]
            if cb_ts == internal_ts:
                wall_stats["timestamp_exact"] += 1
            accumulate(wall_stats, scored)
            if after_cut:
                accumulate(post["wall"], scored)
                if cb_ts == internal_ts:
                    post["wall"]["timestamp_exact"] += 1

        if exact_hit is not None and wall_hit is not None:
            crosstab["both"] += 1
            accumulate(both_stats, score_quote(rec, exact_hit[0], scale))
            both_stats["timestamp_exact"] += 1
            if after_cut:
                post["crosstab"]["both"] += 1
        elif exact_hit is not None:
            crosstab["exact_only"] += 1
            accumulate(exact_only_stats, score_quote(rec, exact_hit[0], scale))
            exact_only_stats["timestamp_exact"] += 1
            if after_cut:
                post["crosstab"]["exact_only"] += 1
        elif wall_hit is not None:
            crosstab["wall_only"] += 1
            scored = score_quote(rec, wall_hit[0], scale)
            accumulate(wall_only_stats, scored)
            if wall_hit[0][3] == internal_ts:
                wall_only_stats["timestamp_exact"] += 1
            if after_cut:
                post["crosstab"]["wall_only"] += 1
        else:
            crosstab["neither"] += 1
            if after_cut:
                post["crosstab"]["neither"] += 1

    abs_deltas_ms.sort()
    post_deltas = sorted(post["abs_deltas_ms"])
    report = {
        "extract_dir": extract_dir,
        "callback_jsonl": callback_jsonl,
        "metadata_dir": metadata_dir,
        "callback_batches": n_batches,
        "decoded": len(rows),
        "callback_unique_code_ts": callback_unique_ts,
        "callback_duplicate_datetime_groups": dict(callback_ts_dup),
        "decoded_duplicate_code_ts_groups": dict(decoded_ts_dup),
        "no_code_callbacks": no_code_callbacks,
        "join_primary_exact_business_ts": summarize(
            join_stats,
            {
                "wall_clock_abs_delta_ms": {
                    "n": len(abs_deltas_ms),
                    "p50": percentile(abs_deltas_ms, 0.50),
                    "p90": percentile(abs_deltas_ms, 0.90),
                    "p95": percentile(abs_deltas_ms, 0.95),
                    "p99": percentile(abs_deltas_ms, 0.99),
                    "max": abs_deltas_ms[-1] if abs_deltas_ms else None,
                    "within_250ms": exact_within_250,
                    "within_250ms_rate": round(exact_within_250 / len(abs_deltas_ms), 4)
                    if abs_deltas_ms
                    else None,
                },
                "record_bytes_ts_also_equals_callback": rec_ts_exact,
            },
        ),
        "control_wall_clock_250ms": summarize(wall_stats),
        "crosstab_exact_ts_vs_250ms_window": dict(crosstab),
        "field_hits_on_both": summarize(both_stats),
        "field_hits_on_exact_only": summarize(exact_only_stats),
        "field_hits_on_wall_only": summarize(wall_only_stats),
        "exact_ts_when_internal_amount_nonzero": summarize(amount_nz_stats),
        "exact_ts_when_internal_amount_zero": summarize(amount_z_stats),
        "post_0925": {
            "crosstab": dict(post["crosstab"]),
            "join_primary_exact_business_ts": summarize(
                post["join"],
                {
                    "wall_clock_abs_delta_ms": {
                        "n": len(post_deltas),
                        "p50": percentile(post_deltas, 0.50),
                        "p90": percentile(post_deltas, 0.90),
                        "p95": percentile(post_deltas, 0.95),
                        "max": post_deltas[-1] if post_deltas else None,
                    }
                },
            ),
            "control_wall_clock_250ms": summarize(post["wall"]),
        },
        "method": {
            "primary": "same market+code and internal_ts == callback datetime unix seconds",
            "tie_break": "nearest callback poll wall-clock among exact-ts candidates; no window gate",
            "control": "existing +/-250ms completed_at vs batch timestamp_ms join",
            "scale": "Rust parity (i32 as f32) / scale_f32",
        },
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
