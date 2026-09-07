#!/usr/bin/env python3
"""Replay Wine public-state merge and OEM getters without editing owned files.

Capture-order public state per (market, code), then join by business second
plus collapsed callback state-groups. First scores incoming amount≠0 records.

usage:
  replay-public-state-oem.py EXTRACT_DIR CALLBACK_JSONL METADATA_DIR [NIGHT_MEM_JSON]
"""
from __future__ import annotations

import json
import os
import struct
import sys
from collections import Counter, defaultdict


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


class Meta:
    __slots__ = ("code", "name", "places", "hint", "scale_places", "last_close_0104")

    def __init__(self, code: str, name: str, tail: list[int]):
        self.code = code
        self.name = name or ""
        self.places = int(tail[0])
        self.hint = int(tail[1]) if len(tail) > 1 else 0
        self.scale_places = float(10 ** self.places) if 0 < self.places <= 6 else 0.0
        raw = bytes(tail[11:15]) if len(tail) >= 15 else b"\0\0\0\0"
        self.last_close_0104 = struct.unpack_from("<i", raw, 0)[0]


def load_metadata(directory: str) -> dict[tuple[str, int], Meta]:
    out: dict[tuple[str, int], Meta] = {}
    for name in os.listdir(directory):
        if not name.endswith("0104.code-table.json"):
            continue
        table = json.loads(open(os.path.join(directory, name), "rb").read())
        market = bytes(table["market"]).decode()
        for rec in table["records"]:
            if not rec.get("code"):
                continue
            meta = Meta(rec["code"], rec.get("name") or "", rec["opaque_tail"])
            if meta.scale_places <= 0:
                continue
            out[(market, rec["symbol_index"])] = meta
    return out


def load_night(path: str | None) -> dict[tuple[str, int], int]:
    if not path or not os.path.isfile(path):
        return {}
    raw = json.loads(open(path, "rb").read())
    out: dict[tuple[str, int], int] = {}
    for key, row in raw.items():
        market, _, index = key.partition(":")
        try:
            out[(market, int(index))] = int(row["last_close"])
        except (KeyError, TypeError, ValueError):
            continue
    return out


def load_decoded(extract_dir: str):
    manifest = json.loads(open(os.path.join(extract_dir, "manifest.json"), "rb").read())
    rows = []
    needed = set()
    order = 0
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
            header = value.get("header") or {}
            rows.append(
                {
                    "order": order,
                    "completed": completed,
                    "market": market,
                    "index": idx["symbol_index"],
                    "ts": idx["timestamp"],
                    "mask": int(value.get("mask") or 0),
                    "clear_ladder": bool(header.get("clear_ladder")),
                    "special_path": bool(header.get("special_path")),
                    "has_book": bool(header.get("has_book")),
                    "rec": rec,
                }
            )
            needed.add((market, idx["symbol_index"]))
            order += 1
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
                quote = (
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
                by_key[key].append(quote)
    for quotes in by_key.values():
        quotes.sort(key=lambda item: (item[0], item[1]))
    return by_key, batches


def quote_fingerprint(q) -> tuple:
    return (
        f32_bits(q[4]),
        f32_bits(q[5]),
        f32_bits(q[9]),
        f32_bits(q[10]),
        f32_bits(q[11][0]),
        f32_bits(q[12][0]),
    )


def collapse_groups(quotes: list) -> dict[int, list]:
    """(business_ts -> unique state groups ordered by first poll)."""
    by_ts: dict[int, dict[tuple, dict]] = defaultdict(dict)
    for q in quotes:
        cb_ts = q[3]
        if cb_ts is None:
            continue
        fp = quote_fingerprint(q)
        group = by_ts[cb_ts].get(fp)
        if group is None:
            by_ts[cb_ts][fp] = {
                "first_ms": q[0],
                "n": 1,
                "quote": q,
            }
        else:
            group["n"] += 1
            if q[0] < group["first_ms"]:
                group["first_ms"] = q[0]
                group["quote"] = q
    out: dict[int, list] = {}
    for ts, groups in by_ts.items():
        ordered = sorted(groups.values(), key=lambda g: (g["first_ms"], g["quote"][1]))
        out[ts] = ordered
    return out


class Public:
    __slots__ = (
        "ts",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "amount",
        "last_close",
        "ladder_p",
        "ladder_v",
    )

    def __init__(self, last_close: int):
        self.ts = 0
        self.open = 0
        self.high = 0
        self.low = 0
        self.close = 0
        self.volume = 0
        self.amount = 0
        self.last_close = last_close
        self.ladder_p = [0] * 10
        self.ladder_v = [0] * 10


def take_nonzero(old: int, new: int) -> int:
    return new if new != 0 else old


def merge(state: Public, row: dict) -> None:
    rec = row["rec"]
    mask = row["mask"]
    mask_class = mask & 0x38
    incoming_ts = row["ts"]
    incoming_open = i32(rec, 0x04)
    incoming_high = i32(rec, 0x08)
    incoming_low = i32(rec, 0x0C)
    incoming_close = i32(rec, 0x10)
    incoming_vol = i64(rec, 0x14)
    incoming_amt = i64(rec, 0x1C)
    incoming_p = [i32(rec, 0x58 + i * 4) for i in range(10)]
    incoming_v = [i32(rec, 0xA8 + i * 4) for i in range(10)]

    # mask&0x38==0x18: Wine returns before value fields; timestamp/identity only.
    if mask_class == 0x18:
        if incoming_ts:
            state.ts = incoming_ts
        return

    if incoming_ts:
        state.ts = incoming_ts
    state.open = take_nonzero(state.open, incoming_open)
    state.high = take_nonzero(state.high, incoming_high)
    state.low = take_nonzero(state.low, incoming_low)
    state.close = take_nonzero(state.close, incoming_close)
    state.volume = take_nonzero(state.volume, incoming_vol)
    state.amount = take_nonzero(state.amount, incoming_amt)
    # last_close stays on the 0104/night seed. 0x12b is the decoder reference
    # price and is not the OEM last getter.

    # Sparse ladder copyback: zero incoming slots keep the previous public book.
    # clear_ladder on the decoded record already zeroed the 311B copy; copying
    # those zeros would wipe the OEM book (SH603059 fixture).
    for i in range(10):
        if incoming_p[i] != 0:
            state.ladder_p[i] = incoming_p[i]
        if incoming_v[i] != 0:
            state.ladder_v[i] = incoming_v[i]


def project(state: Public, scale: float) -> dict:
    bid_p = [(rust_scaled(state.ladder_p[4 - i], scale) if i < 5 else 0.0) for i in range(10)]
    ask_p = [(rust_scaled(state.ladder_p[5 + i], scale) if i < 5 else 0.0) for i in range(10)]
    bid_v = [to_f32(state.ladder_v[4 - i] if i < 5 else 0) for i in range(10)]
    ask_v = [to_f32(state.ladder_v[5 + i] if i < 5 else 0) for i in range(10)]
    return {
        "price": rust_scaled(state.close, scale),
        "last_close": rust_scaled(state.last_close, scale),
        "open": rust_scaled(state.open, scale),
        "high": rust_scaled(state.high, scale),
        "low": rust_scaled(state.low, scale),
        "volume": to_f32(state.volume),
        "amount": to_f32(state.amount),
        "bid1": bid_p[0],
        "ask1": ask_p[0],
        "bid_p": bid_p,
        "ask_p": ask_p,
        "bid_v": bid_v,
        "ask_v": ask_v,
    }


def pick_group(groups: list, frame_micros: int, decoded_index_in_second: int):
    if not groups:
        return None, "no_exact_ts"
    if len(groups) == 1:
        return groups[0]["quote"], "unique_state"
    if decoded_index_in_second < len(groups):
        return groups[decoded_index_in_second]["quote"], "order_in_second"
    best = None
    best_key = None
    for group in groups:
        delta = abs(group["first_ms"] * 1000 - frame_micros)
        key = (delta, group["first_ms"], group["quote"][1])
        if best_key is None or key < best_key:
            best_key = key
            best = group["quote"]
    return best, "nearest_after_collapse"


def score_pair(proj: dict, q, incoming_amt: int) -> dict:
    cb_bp = q[11]
    cb_ap = q[12]
    hits = {
        "price": same_f32(proj["price"], q[4]),
        "last_close": same_f32(proj["last_close"], q[5]),
        "open": same_f32(proj["open"], q[6]),
        "high": same_f32(proj["high"], q[7]),
        "low": same_f32(proj["low"], q[8]),
        "volume": same_f32(proj["volume"], q[9]),
        "amount": same_f32(proj["amount"], q[10]),
        "bid1": same_f32(proj["bid1"], cb_bp[0]),
        "ask1": same_f32(proj["ask1"], cb_ap[0]),
        "bid_px_5": all(same_f32(proj["bid_p"][i], cb_bp[i]) for i in range(5)),
        "ask_px_5": all(same_f32(proj["ask_p"][i], cb_ap[i]) for i in range(5)),
        "price_zero_both": proj["price"] == 0 and q[4] == 0,
        "amount_zero_both": proj["amount"] == 0 and q[10] == 0,
        "volume_zero_both": proj["volume"] == 0 and q[9] == 0,
        "bid1_both_nz": proj["bid1"] != 0 and cb_bp[0] != 0,
        "incoming_amount_nz": incoming_amt != 0,
    }
    return hits


def accumulate(stats: Counter, hits: dict) -> None:
    stats["matched"] += 1
    for key in (
        "price",
        "last_close",
        "open",
        "high",
        "low",
        "volume",
        "amount",
        "bid1",
        "ask1",
        "bid_px_5",
        "ask_px_5",
    ):
        if hits[key]:
            stats[key] += 1
    if hits["price"] and not hits["price_zero_both"]:
        stats["price_nonzero"] += 1
    if hits["amount"] and not hits["amount_zero_both"]:
        stats["amount_nonzero"] += 1
    if hits["volume"] and not hits["volume_zero_both"]:
        stats["volume_nonzero"] += 1
    if hits["price_zero_both"]:
        stats["price_zero_both"] += 1
    if hits["bid1_both_nz"]:
        stats["bid1_both_nz"] += 1
        if hits["bid1"]:
            stats["bid1_both_nz_hit"] += 1


def summarize(stats: Counter) -> dict:
    n = stats["matched"] or 1
    return {
        "counts": dict(stats),
        "nonzero_price_hits": stats["price_nonzero"],
        "nonzero_amount_hits": stats["amount_nonzero"],
        "nonzero_volume_hits": stats["volume_nonzero"],
        "bid1_both_nonzero_hit_rate": round(stats["bid1_both_nz_hit"] / stats["bid1_both_nz"], 4)
        if stats["bid1_both_nz"]
        else None,
        "rates_on_matched": {
            k: round(stats[k] / n, 4)
            for k in (
                "price",
                "last_close",
                "open",
                "high",
                "low",
                "volume",
                "amount",
                "bid1",
                "ask1",
                "bid_px_5",
                "ask_px_5",
            )
        },
    }


def last_close_source_report(wanted, metadata, night, quotes) -> dict:
    codes = {}
    for key, items in quotes.items():
        lasts = Counter()
        for q in items:
            if q[5] != 0:
                lasts[f32_bits(to_f32(q[5]))] += 1
        if lasts:
            bits, n = lasts.most_common(1)[0]
            codes[key] = (to_f32(struct.unpack("<f", struct.pack("<I", bits))[0]), n, len(items))
    stats = Counter()
    for (market, index), meta in metadata.items():
        key = (market, meta.code)
        if key not in wanted or key not in codes:
            continue
        cb_last, _, _ = codes[key]
        stats["codes"] += 1
        scale = meta.scale_places
        hint_scale = float(meta.hint) if meta.hint in (1, 10, 100) else scale
        src_0104 = rust_scaled(meta.last_close_0104, scale)
        src_0104_hint = rust_scaled(meta.last_close_0104, hint_scale)
        night_i = night.get((market, index))
        if same_f32(src_0104, cb_last):
            stats["0104_places"] += 1
        if same_f32(src_0104_hint, cb_last):
            stats["0104_hint"] += 1
        if night_i is not None:
            stats["night_present"] += 1
            if same_f32(rust_scaled(night_i, scale), cb_last):
                stats["night_places"] += 1
            if same_f32(rust_scaled(night_i, hint_scale), cb_last):
                stats["night_hint"] += 1
    n = stats["codes"] or 1
    return {
        "codes_with_callback_last_close": stats["codes"],
        "hits": dict(stats),
        "rates": {
            "0104_places": round(stats["0104_places"] / n, 4),
            "0104_hint": round(stats["0104_hint"] / n, 4),
            "night_places": round(stats["night_places"] / (stats["night_present"] or 1), 4),
            "night_hint": round(stats["night_hint"] / (stats["night_present"] or 1), 4),
        },
        "note": (
            "0104 here is 2026-09-03 verified-4068 (T-2 close for the 09-04 auction). "
            "Night memory is 09-04 01:19 Wine 311B last_close (T-1 close)."
        ),
    }


def main() -> int:
    if len(sys.argv) < 4:
        print(
            "usage: replay-public-state-oem.py EXTRACT_DIR CALLBACK_JSONL METADATA_DIR [NIGHT_MEM_JSON]",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, metadata_dir = sys.argv[1:4]
    night_path = sys.argv[4] if len(sys.argv) > 4 else None
    metadata = load_metadata(metadata_dir)
    night = load_night(night_path)
    rows, needed_idx = load_decoded(extract_dir)
    wanted_codes = set()
    for market, index in needed_idx:
        meta = metadata.get((market, index))
        if meta:
            wanted_codes.add((market, meta.code))
    quotes, n_batches = stream_quotes(callback_jsonl, wanted_codes)
    grouped = {key: collapse_groups(items) for key, items in quotes.items()}

    states: dict[tuple[str, str], Public] = {}
    per_code_ts_count: dict[tuple[str, str, int], int] = defaultdict(int)
    raw_multi = 0
    collapsed_multi = 0
    join_kind = Counter()
    amount_nz = Counter()
    carry = Counter()
    all_matched = Counter()
    live_nz = Counter()
    preopen_nz = Counter()
    samples = []

    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None:
            continue
        key = (row["market"], meta.code)
        if key not in states:
            last_close = night.get((row["market"], row["index"]), meta.last_close_0104)
            states[key] = Public(last_close)
        incoming_amt = i64(row["rec"], 0x1C)
        merge(states[key], row)
        proj = project(states[key], meta.scale_places)
        groups = grouped.get(key, {}).get(row["ts"], [])
        raw_at_ts = sum(1 for q in quotes.get(key, []) if q[3] == row["ts"])
        if raw_at_ts > 1:
            raw_multi += 1
        if len(groups) > 1:
            collapsed_multi += 1
        slot = per_code_ts_count[(key[0], key[1], row["ts"])]
        per_code_ts_count[(key[0], key[1], row["ts"])] += 1
        picked, kind = pick_group(groups, row["completed"], slot)
        if picked is None:
            join_kind[kind] += 1
            continue
        join_kind[kind] += 1
        hits = score_pair(proj, picked, incoming_amt)
        accumulate(all_matched, hits)
        if incoming_amt != 0:
            accumulate(amount_nz, hits)
            if picked[4] == 0 and picked[10] == 0:
                accumulate(preopen_nz, hits)
            else:
                accumulate(live_nz, hits)
            if len(samples) < 12 and not hits["amount"] and not (
                picked[4] == 0 and picked[10] == 0
            ):
                samples.append(
                    {
                        "market": row["market"],
                        "code": meta.code,
                        "ts": row["ts"],
                        "mask": row["mask"],
                        "clear_ladder": row["clear_ladder"],
                        "join": kind,
                        "internal_amount": incoming_amt,
                        "proj_amount": proj["amount"],
                        "cb_amount": picked[10],
                        "proj_price": proj["price"],
                        "cb_price": picked[4],
                        "proj_last_close": proj["last_close"],
                        "cb_last_close": picked[5],
                        "proj_bid1": proj["bid1"],
                        "cb_bid1": picked[11][0],
                    }
                )
        else:
            accumulate(carry, hits)

    report = {
        "extract_dir": extract_dir,
        "callback_jsonl": callback_jsonl,
        "metadata_dir": metadata_dir,
        "night_mem": night_path,
        "callback_batches": n_batches,
        "decoded": len(rows),
        "merge_rules": {
            "order": "capture-order per (market, code)",
            "amount_volume_ohlc_price": "keep previous when incoming integer is 0",
            "last_close": "sticky seed from night 311B last_close, else 0104 tail i32@+11; never 2704 0x12b",
            "ladder": "sparse copyback: nonzero incoming levels only",
            "mask_class_0x18": "timestamp only",
            "scale": "10 ** 0104.opaque_tail[0] as f32, matching OEM pointNum / Rust price_scale_hint",
            "getter": "(i32 as f32) / (scale as f32) for prices; i64 as f32 for volume/amount",
        },
        "last_close_source_vs_callback_mode": last_close_source_report(
            wanted_codes, metadata, night, quotes
        ),
        "multi_candidate": {
            "exact_ts_rows_with_raw_duplicate_callbacks": raw_multi,
            "exact_ts_rows_with_multiple_state_groups_after_collapse": collapsed_multi,
            "join_kind": dict(join_kind),
        },
        "amount_nonzero_incoming": summarize(amount_nz),
        "amount_nonzero_preopen_callback_price_and_amount_zero": summarize(preopen_nz),
        "amount_nonzero_live_callback_has_price_or_amount": summarize(live_nz),
        "carry_forward_incoming_amount0": summarize(carry),
        "all_joined": summarize(all_matched),
        "amount_mismatch_samples": samples,
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
