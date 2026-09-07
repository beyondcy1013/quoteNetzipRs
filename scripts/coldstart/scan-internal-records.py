#!/usr/bin/env python3
"""Scan a memory dump for 0x137-byte 5188 internal records.

Record layout (netzip-fullpull Official5188InternalRecord):
  0x00 ts u32 | 0x04 open | 0x08 high | 0x0c low | 0x10 last (i32)
  0x14 volume i64 | 0x1c amount i64 | 0x58 ladder prices[10] | 0xa8 ladder vols[10]
  0xdf symbol_index u16 | 0xe1 market "SH"/"SZ" | 0x12b last_close i32

usage: scan_wjf_records.py <dump.bin> <out.json> [min_ts]
"""
import json
import struct
import sys

LEN = 0x137
data = open(sys.argv[1], "rb").read()
out = sys.argv[2]
min_ts = int(sys.argv[3]) if len(sys.argv) > 3 else 0
records = {}
pos = 0
n = len(data)
find = data.find
markets = (b"SH", b"SZ")
candidates = 0
for mk in markets:
    pos = 0
    while True:
        pos = find(mk, pos)
        if pos < 0:
            break
        start = pos - 0xE1
        pos += 1
        if start < 0 or start + LEN > n:
            continue
        rec = data[start:start + LEN]
        # heuristics: market bytes are followed by a small region; symbol index < 40000
        idx = struct.unpack_from("<H", rec, 0xDF)[0]
        if idx >= 40000:
            continue
        ts = struct.unpack_from("<I", rec, 0)[0]
        if ts < min_ts or ts > min_ts + 600000:
            continue
        candidates += 1
        key = f"{mk.decode()}:{idx}"
        o, h, l, c = struct.unpack_from("<iiii", rec, 4)
        vol, amt = struct.unpack_from("<qq", rec, 0x14)
        lc = struct.unpack_from("<i", rec, 0x12B)[0]
        prices = list(struct.unpack_from("<10i", rec, 0x58))
        vols = list(struct.unpack_from("<10i", rec, 0xA8))
        item = {
            "addr_off": start, "ts": ts, "open": o, "high": h, "low": l, "close": c,
            "volume": vol, "amount": amt, "last_close": lc, "ladder_prices": prices,
            "ladder_volumes": vols, "raw": rec.hex(),
        }
        # keep the latest timestamp per key; remember duplicates count
        prev = records.get(key)
        if prev is None or ts > prev["ts"]:
            item["dups"] = (prev["dups"] + 1) if prev else 0
            records[key] = item
        else:
            prev["dups"] += 1
json.dump(records, open(out, "w"))
print(f"candidates={candidates} unique_keys={len(records)}")
