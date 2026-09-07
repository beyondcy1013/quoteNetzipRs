#!/usr/bin/env python3
"""Same-day last_close getter + pre-open OEM reset, without touching owned files.

Joins night 311B / 0104 by (market, code), not symbol_index: Wine 01:19 and the
09:24 Rust auction sit on different 0104 index maps.

usage:
  seed-last-close-and-preopen.py EXTRACT_DIR CALLBACK_JSONL META_0903 NIGHT_0104_DIR PROBE_JSON
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


def load_largest_0104(directory: str) -> dict[tuple[str, str], dict]:
    by_market: dict[str, list] = defaultdict(list)
    for name in os.listdir(directory):
        if not name.endswith("0104.code-table.json"):
            continue
        table = json.loads(open(os.path.join(directory, name), "rb").read())
        market = bytes(table["market"]).decode()
        by_market[market].append((len(table["records"]), name, table))
    out: dict[tuple[str, str], dict] = {}
    for market, items in by_market.items():
        items.sort(reverse=True)
        _, _, table = items[0]
        for rec in table["records"]:
            code = rec.get("code") or ""
            if not code:
                continue
            tail = bytes(rec["opaque_tail"])
            places = tail[0] if tail else 0
            scale = float(10 ** places) if 0 < places <= 6 else 0.0
            last_0104 = i32(tail, 11) if len(tail) >= 15 else 0
            high_0104 = i32(tail, 15) if len(tail) >= 19 else 0
            low_0104 = i32(tail, 19) if len(tail) >= 23 else 0
            out[(market, code)] = {
                "index": rec["symbol_index"],
                "places": places,
                "scale": scale,
                "amount_mode": rec.get("amount_mode"),
                "last_0104": last_0104,
                "high_0104": high_0104,
                "low_0104": low_0104,
                "tail": list(tail),
            }
    return out


def load_probe_by_code(path: str) -> dict[tuple[str, str], dict]:
    rows = json.loads(open(path, "rb").read())
    best: dict[tuple[str, str], dict] = {}
    for row in rows:
        code = row.get("code") or ""
        market = row.get("market") or ""
        if not code or market not in ("SH", "SZ"):
            continue
        key = (market, code)
        prev = best.get(key)
        if prev is None or int(row.get("frame_index") or 0) >= int(prev.get("frame_index") or 0):
            best[key] = row
    return best


def load_index_metadata(directory: str) -> dict[tuple[str, int], dict]:
    """Auction extract indexes match 09-03 tables, not the night Wine session."""
    by_code = load_largest_0104(directory)
    out: dict[tuple[str, int], dict] = {}
    for (market, code), meta in by_code.items():
        packed = dict(meta)
        packed["code"] = code
        packed["market"] = market
        out[(market, meta["index"])] = packed
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
            rows.append(
                {
                    "order": order,
                    "completed": completed,
                    "market": market,
                    "index": idx["symbol_index"],
                    "ts": idx["timestamp"],
                    "mask": int(value.get("mask") or 0),
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
                quote = (
                    ts_ms,
                    seq,
                    datetime,
                    parse_callback_ts(datetime),
                    float(q.get("price") or 0),
                    float(q.get("last_close") or 0),
                    float(q.get("open") or 0),
                    float(q.get("high") or 0),
                    float(q.get("low") or 0),
                    float(q.get("volume") or 0),
                    float(q.get("amount") or 0),
                    [float(x) for x in q.get("bid_prices") or [0] * 10],
                    [float(x) for x in q.get("ask_prices") or [0] * 10],
                )
                by_key[key].append(quote)
    for quotes in by_key.values():
        quotes.sort(key=lambda item: (item[0], item[1]))
    return by_key, batches


def callback_last_mode(quotes: dict[tuple[str, str], list]) -> dict[tuple[str, str], float]:
    out = {}
    for key, items in quotes.items():
        lasts = Counter()
        for q in items:
            if q[5] != 0:
                lasts[f32_bits(to_f32(q[5]))] += 1
        if not lasts:
            continue
        bits, _ = lasts.most_common(1)[0]
        out[key] = to_f32(struct.unpack("<f", struct.pack("<I", bits))[0])
    return out


def score_source(name: str, getter, keys, lasts, scales) -> dict:
    hits = 0
    present = 0
    for key in keys:
        value = getter(key)
        if value is None:
            continue
        present += 1
        scale = scales.get(key)
        if not scale:
            continue
        if same_f32(rust_scaled(int(value), scale), lasts[key]):
            hits += 1
    n = len(keys) or 1
    return {
        "name": name,
        "present": present,
        "hits": hits,
        "rate_on_present": round(hits / (present or 1), 4),
        "rate_on_codes": round(hits / n, 4),
    }


def scan_night_raw_offsets(probe: dict, lasts, scales, keys) -> list[dict]:
    offset_hits: Counter = Counter()
    scanned = 0
    for key in keys:
        row = probe.get(key)
        if row is None or key not in lasts:
            continue
        raw_hex = row.get("record_hex") or ""
        if not raw_hex:
            continue
        try:
            raw = bytes.fromhex(raw_hex)
        except ValueError:
            continue
        scale = scales.get(key)
        if not scale:
            continue
        target = round(lasts[key] * scale)
        scanned += 1
        for off in range(0, len(raw) - 3, 4):
            if i32(raw, off) == target:
                offset_hits[off] += 1
    ranked = [
        {"offset": f"0x{off:02x}", "off": off, "hits": hits, "rate": round(hits / (scanned or 1), 4)}
        for off, hits in offset_hits.most_common(12)
    ]
    return [{"scanned": scanned}, *ranked]


class Public:
    __slots__ = ("ts", "open", "high", "low", "close", "volume", "amount", "last_close", "ladder_p")

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


def take_nonzero(old: int, new: int) -> int:
    return new if new != 0 else old


def merge(state: Public, row: dict) -> None:
    rec = row["rec"]
    if (row["mask"] & 0x38) == 0x18:
        if row["ts"]:
            state.ts = row["ts"]
        return
    if row["ts"]:
        state.ts = row["ts"]
    state.open = take_nonzero(state.open, i32(rec, 0x04))
    state.high = take_nonzero(state.high, i32(rec, 0x08))
    state.low = take_nonzero(state.low, i32(rec, 0x0C))
    state.close = take_nonzero(state.close, i32(rec, 0x10))
    state.volume = take_nonzero(state.volume, i64(rec, 0x14))
    state.amount = take_nonzero(state.amount, i64(rec, 0x1C))
    for i in range(10):
        px = i32(rec, 0x58 + i * 4)
        if px != 0:
            state.ladder_p[i] = px


def project(state: Public, scale: float, *, preopen: bool) -> dict:
    close = 0 if preopen else state.close
    open_ = 0 if preopen else state.open
    high = 0 if preopen else state.high
    low = 0 if preopen else state.low
    volume = 0 if preopen else state.volume
    amount = 0 if preopen else state.amount
    bid1 = 0.0 if preopen else rust_scaled(state.ladder_p[4], scale)
    ask1 = 0.0 if preopen else rust_scaled(state.ladder_p[5], scale)
    return {
        "price": rust_scaled(close, scale),
        "last_close": rust_scaled(state.last_close, scale),
        "open": rust_scaled(open_, scale),
        "high": rust_scaled(high, scale),
        "low": rust_scaled(low, scale),
        "volume": to_f32(volume),
        "amount": to_f32(amount),
        "bid1": bid1,
        "ask1": ask1,
    }


def pick_quote(quotes: list, ts: int):
    matched = [q for q in quotes if q[3] == ts]
    if not matched:
        return None
    return matched[0]


def accumulate(stats: Counter, proj: dict, q) -> None:
    stats["matched"] += 1
    pairs = (
        ("price", proj["price"], q[4]),
        ("last_close", proj["last_close"], q[5]),
        ("open", proj["open"], q[6]),
        ("high", proj["high"], q[7]),
        ("low", proj["low"], q[8]),
        ("volume", proj["volume"], q[9]),
        ("amount", proj["amount"], q[10]),
        ("bid1", proj["bid1"], q[11][0]),
        ("ask1", proj["ask1"], q[12][0]),
    )
    for name, left, right in pairs:
        if same_f32(left, right):
            stats[name] += 1
            if name in ("price", "amount", "volume", "last_close") and not (left == 0 and right == 0):
                stats[f"{name}_nonzero"] += 1


def summarize(stats: Counter) -> dict:
    n = stats["matched"] or 1
    return {
        "n": stats["matched"],
        "hits": {
            k: stats[k]
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
            )
        },
        "nonzero": {
            k: stats[k]
            for k in (
                "price_nonzero",
                "last_close_nonzero",
                "volume_nonzero",
                "amount_nonzero",
            )
        },
        "rates": {
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
            )
        },
    }


def replay(rows, metadata, quotes, seed_by_code, *, preopen_cutoff: int | None):
    states: dict[tuple[str, str], Public] = {}
    amount_nz = Counter()
    live_nz = Counter()
    preopen_cb = Counter()
    all_joined = Counter()
    samples = []
    for row in rows:
        meta = metadata.get((row["market"], row["index"]))
        if meta is None or meta["scale"] <= 0:
            continue
        key = (row["market"], meta["code"])
        if key not in states:
            seed = seed_by_code.get(key, meta["last_0104"])
            states[key] = Public(int(seed))
        incoming_amt = i64(row["rec"], 0x1C)
        merge(states[key], row)
        q = pick_quote(quotes.get(key, []), row["ts"])
        if q is None:
            continue
        preopen = False
        if preopen_cutoff is not None and q[3] is not None and q[3] < preopen_cutoff:
            preopen = True
        proj = project(states[key], meta["scale"], preopen=preopen)
        accumulate(all_joined, proj, q)
        if incoming_amt != 0:
            accumulate(amount_nz, proj, q)
            if q[4] == 0 and q[10] == 0:
                accumulate(preopen_cb, proj, q)
            else:
                accumulate(live_nz, proj, q)
                if len(samples) < 8 and not same_f32(proj["amount"], q[10]):
                    samples.append(
                        {
                            "code": meta["code"],
                            "ts": row["ts"],
                            "datetime": q[2],
                            "proj_last": proj["last_close"],
                            "cb_last": q[5],
                            "proj_price": proj["price"],
                            "cb_price": q[4],
                            "proj_amount": proj["amount"],
                            "cb_amount": q[10],
                            "preopen_applied": preopen,
                        }
                    )
    return {
        "all_joined": summarize(all_joined),
        "amount_nonzero_incoming": summarize(amount_nz),
        "amount_nonzero_callback_price_and_amount_zero": summarize(preopen_cb),
        "amount_nonzero_live": summarize(live_nz),
        "live_amount_mismatch_samples": samples,
    }


def main() -> int:
    if len(sys.argv) != 6:
        print(
            "usage: seed-last-close-and-preopen.py EXTRACT CALLBACK META_0903 NIGHT_0104 PROBE_JSON",
            file=sys.stderr,
        )
        return 2
    extract_dir, callback_jsonl, meta_0903, night_0104, probe_json = sys.argv[1:]
    metadata = load_index_metadata(meta_0903)
    night_meta = load_largest_0104(night_0104)
    probe = load_probe_by_code(probe_json)
    rows, needed = load_decoded(extract_dir)
    wanted = set()
    for market, index in needed:
        meta = metadata.get((market, index))
        if meta:
            wanted.add((market, meta["code"]))
    quotes, n_batches = stream_quotes(callback_jsonl, wanted)
    lasts = callback_last_mode(quotes)
    meta_0903_by_code = {(m["market"], m["code"]): m for m in metadata.values()}
    scales = {}
    for key in lasts:
        meta = night_meta.get(key) or meta_0903_by_code.get(key)
        if meta:
            scales[key] = meta["scale"]
    keys = sorted(lasts)
    open_cutoff = parse_callback_ts("2026-09-04 09:25:00")

    def g_night_close(key):
        row = probe.get(key)
        return None if row is None else row.get("close")

    def g_night_last(key):
        row = probe.get(key)
        return None if row is None else row.get("last_close")

    def g_night_0104(key):
        row = night_meta.get(key)
        return None if row is None else row.get("last_0104")

    def g_0903_0104(key):
        row = meta_0903_by_code.get(key)
        return None if row is None else row.get("last_0104")

    sources = [
        score_source("night_311B_close_+0x10_by_code", g_night_close, keys, lasts, scales),
        score_source("night_311B_last_close_+0x12b_by_code", g_night_last, keys, lasts, scales),
        score_source("night_0104_tail_i32_+11_by_code", g_night_0104, keys, lasts, scales),
        score_source("0903_0104_tail_i32_+11_by_code", g_0903_0104, keys, lasts, scales),
    ]

    seed_close = {key: int(probe[key]["close"]) for key in probe if key in lasts}
    seed_last = {key: int(probe[key]["last_close"]) for key in probe if key in lasts}

    report = {
        "extract_dir": extract_dir,
        "callback_jsonl": callback_jsonl,
        "codes_with_callback_last_close": len(keys),
        "callback_batches": n_batches,
        "decoded": len(rows),
        "index_note": (
            "09:24 Rust auction 2704 embedded codes match 09-03 0104 indexes; "
            "Wine 01:19 0104 reshuffled most indexes. Last-close seeds join by code."
        ),
        "last_close_sources": sources,
        "night_raw_i32_offsets_matching_callback_last_ticks": scan_night_raw_offsets(
            probe, lasts, scales, keys
        ),
        "replay_seed_night_close_no_preopen_reset": replay(
            rows, metadata, quotes, seed_close, preopen_cutoff=None
        ),
        "replay_seed_night_close_zero_trade_fields_before_0925": replay(
            rows, metadata, quotes, seed_close, preopen_cutoff=open_cutoff
        ),
        "replay_seed_night_last_close_no_preopen_reset": replay(
            rows, metadata, quotes, seed_last, preopen_cutoff=None
        ),
        "open_cutoff_datetime": "2026-09-04 09:25:00",
        "open_cutoff_unix": open_cutoff,
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
