#!/usr/bin/env python3
"""Sanitized 0104 -> 2a10 partition alignment audit.

Decodes the four server 0104 zlib envelopes and the client 2a10 frames from a
5188 capture's extracted payload directory (produced by
``diagnostics/20260831-netzip-windows-vs-rust/analysis/extract_5188_payloads.py``),
then verifies that the five primary 1024-entry partitions equal the first 5120
eligible SH/SZ records of the current connection's 0104 tables
(``0x10000 | u16 ordinal``).  Only market and code are decoded; no raw payload
or credential is emitted.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import struct
import zlib
from collections import Counter
from typing import Any


CODE_TABLE_HEADER_LEN = 98
CODE_TABLE_RECORD_LEN = 68
PARTITION_LEN = 1024
PRIMARY_PARTITION_COUNT = 5
ELIGIBLE_NEEDED = PRIMARY_PARTITION_COUNT * PARTITION_LEN
INDEX_BASE = 0x1_0000


def decode_0104_table(path: str) -> dict[str, Any]:
    if path.endswith(".code-table.json"):
        with open(path, encoding="utf-8") as input_file:
            document = json.load(input_file)
        market = bytes(document["market"]).decode("ascii")
        codes = [
            (int(record["symbol_index"]), str(record.get("code") or ""))
            for record in document["records"]
        ]
        return {"market": market, "codes": codes}
    data = open(path, "rb").read()
    if len(data) < 8:
        raise ValueError(f"{path}: too short for zlib envelope")
    uncompressed_len, compressed_len = struct.unpack("<II", data[:8])
    if 8 + compressed_len != len(data) or data[8] != 0x78:
        raise ValueError(f"{path}: not a closing zlib envelope")
    decoded = zlib.decompress(data[8 : 8 + compressed_len])
    if len(decoded) != uncompressed_len:
        raise ValueError(
            f"{path}: length mismatch declared {uncompressed_len}, got {len(decoded)}"
        )
    market = decoded[12:14].decode("latin1")
    record_count = (len(decoded) - CODE_TABLE_HEADER_LEN) // CODE_TABLE_RECORD_LEN
    codes: list[tuple[int, str]] = []
    for ordinal in range(record_count):
        start = CODE_TABLE_HEADER_LEN + ordinal * CODE_TABLE_RECORD_LEN
        record = decoded[start : start + CODE_TABLE_RECORD_LEN]
        symbol_index = struct.unpack("<H", record[:2])[0]
        code = record[2:12].split(b"\x00")[0].decode("ascii", "ignore")
        codes.append((symbol_index, code))
    return {"market": market, "codes": codes}


def is_primary_sh(code: str) -> bool:
    return len(code) == 6 and code.isdigit() and "600000" <= code <= "699999"


def is_primary_sz(code: str) -> bool:
    if len(code) != 6 or not code.isdigit():
        return False
    return code[:3] in ("000", "001", "002", "003", "300", "301")


def eligible_sequence(tables: dict[str, list[tuple[int, str]]]) -> list[tuple[str, int]]:
    seq: list[tuple[str, int]] = []
    for market, codes in (("SH", "SH"), ("SZ", "SZ")):
        for symbol_index, code in tables[market]:
            eligible = is_primary_sh(code) if market == "SH" else is_primary_sz(code)
            if eligible:
                seq.append((market, INDEX_BASE | symbol_index))
    return seq


def load_2a10_frames(client_dir: str) -> list[list[tuple[str, int]]]:
    frames: list[list[tuple[str, int]]] = []
    paths = glob.glob(os.path.join(client_dir, "*_2a10_*.bin"))
    paths += glob.glob(os.path.join(client_dir, "*-2a10.payload.bin"))
    for path in sorted(set(paths)):
        with open(path, "rb") as input_file:
            payload = input_file.read()
        # Accept both raw-payload dumps (10B prefix + 6B entries) and framed
        # dumps (8-byte kind/len/metadata header before the payload).
        kind = struct.unpack("<H", payload[:2])[0] if len(payload) >= 2 else 0
        declared_len = struct.unpack("<H", payload[2:4])[0] if len(payload) >= 4 else 0
        if kind == 0x102A or (declared_len and declared_len + 8 == len(payload)):
            payload = payload[8:]
        if len(payload) < 10 or (len(payload) - 10) % 6:
            raise ValueError(f"{path}: invalid 2a10 payload length {len(payload)}")
        entry_count = (len(payload) - 10) // 6
        entries: list[tuple[str, int]] = []
        for index in range(entry_count):
            entry = payload[10 + index * 6 : 16 + index * 6]
            market = entry[0:2].decode("latin1")
            if market not in ("SH", "SZ"):
                raise ValueError(f"{path}: invalid market {market!r} at entry {index}")
            symbol_value = struct.unpack("<I", entry[2:6])[0]
            entries.append((market, symbol_value))
        frames.append(entries)
    return frames


def audit(server_dir: str, client_dir: str) -> dict[str, Any]:
    tables: dict[str, list[tuple[int, str]]] = {}
    table_paths = sorted(glob.glob(os.path.join(server_dir, "*_0104_*.bin")))
    table_paths += sorted(glob.glob(os.path.join(server_dir, "*-0104.code-table.json")))
    if len(table_paths) < 2:
        raise ValueError(f"expected at least two 0104 tables in {server_dir}, got {len(table_paths)}")
    # Extractor output can contain repeated/incremental tables. Keep the
    # largest complete table for each market rather than concatenating them.
    for path in table_paths:
        table = decode_0104_table(path)
        current = tables.get(table["market"])
        if current is None or len(table["codes"]) > len(current):
            tables[table["market"]] = table["codes"]

    missing = {"SH", "SZ"} - tables.keys()
    if missing:
        raise ValueError(f"missing 0104 markets: {sorted(missing)}")

    seq = eligible_sequence(tables)
    expected = [seq[index * PARTITION_LEN : (index + 1) * PARTITION_LEN] for index in range(PRIMARY_PARTITION_COUNT)]
    frames = load_2a10_frames(client_dir)

    rows: list[dict[str, Any]] = []
    for index, partition in enumerate(frames[: len(frames)]):
        markets = Counter(market for market, _ in partition)
        row: dict[str, Any] = {
            "partition": index + 1,
            "entry_count": len(partition),
            "markets": dict(markets),
        }
        if index < PRIMARY_PARTITION_COUNT:
            matches = partition == expected[index]
            mismatch: Any = None
            if not matches:
                for pos, (left, right) in enumerate(zip(expected[index], partition)):
                    if left != right:
                        mismatch = {"pos": pos, "expected": left, "actual": right}
                        break
            row["equals_eligible_prefix"] = matches
            row["first_mismatch"] = mismatch
        rows.append(row)

    total_entries = sum(len(partition) for partition in frames)
    primary_match = all(
        row["entry_count"] == PARTITION_LEN
        and row.get("equals_eligible_prefix") is True
        for row in rows[:PRIMARY_PARTITION_COUNT]
    )
    return {
        "schema": "quoteNetzipRs.official_5188_partition_alignment.v1",
        "tables": {market: len(codes) for market, codes in tables.items()},
        "eligible_total": len(seq),
        "eligible_per_market": dict(Counter(market for market, _ in seq)),
        "primary_partitions_match_eligible_prefix": primary_match,
        "total_2a10_entries": total_entries,
        "partitions": rows,
    }


def read_receive_list(path: str) -> set[str]:
    """Reads the vendor 只接收股票代码表.csv enabled SH/SZ codes.

    The file is UTF-16LE TAB; enabled rows carry a literal ``1`` in the fourth
    column and a market-qualified symbol in the second (e.g. ``SH510010``).
    Returns market+code symbols (``SH510010``).
    """
    text = open(path, "rb").read().decode("utf-16-le", "replace")
    out: set[str] = set()
    for line in text.splitlines():
        fields = [field.strip() for field in line.split("\t")]
        if len(fields) < 4 or fields[3] != "1":
            continue
        symbol = fields[1].upper()
        if len(symbol) == 8 and symbol[:2] in ("SH", "SZ"):
            out.add(symbol)
    return out


def load_0104_by_local_port(
    server_dir: str, local_ip: str = "192.168.3.2"
) -> dict[str, dict[str, list[str]]]:
    """Groups decoded SH/SZ/B$/SF tables by the local source port."""
    per_port: dict[str, dict[str, list[str]]] = {}
    for path in glob.glob(os.path.join(server_dir, "*_0104_*.bin")):
        parts = os.path.basename(path).split("_")
        try:
            local_index = parts.index(local_ip)
        except ValueError:
            continue
        local_port = parts[local_index + 1]
        table = decode_0104_table(path)
        per_port.setdefault(local_port, {}).setdefault(
            table["market"], [code for _, code in table["codes"]]
        )
    return per_port


def audit_receive_list_universe(
    server_dir: str, client_dir: str, receive_list_path: str, local_ip: str
) -> dict[str, Any]:
    """Confirms every decoded 2a10 entry is in 接收清单 ∩ the flow's 0104."""
    tables = load_0104_by_local_port(server_dir, local_ip)
    receive = read_receive_list(receive_list_path)

    total = 0
    hit = 0
    not_in_table = 0
    not_in_receive = 0
    per_partition: list[dict[str, int]] = []
    for path in sorted(glob.glob(os.path.join(client_dir, "*_2a10_*.bin"))):
        parts = os.path.basename(path).split("_")
        try:
            local_index = parts.index(local_ip)
        except ValueError:
            continue
        local_port = parts[local_index + 1]
        flow_tables = tables.get(local_port, {})
        payload = open(path, "rb").read()
        entry_count = (len(payload) - 10) // 6
        entries = 0
        hits = 0
        for index in range(entry_count):
            entry = payload[10 + index * 6 : 16 + index * 6]
            market = entry[0:2].decode("latin1")
            ordinal = struct.unpack("<I", entry[2:6])[0] - INDEX_BASE
            pool = flow_tables.get(market)
            entries += 1
            total += 1
            if pool is None or ordinal >= len(pool):
                not_in_table += 1
                continue
            code = pool[ordinal]
            if (market + code) in receive:
                hit += 1
                hits += 1
            else:
                not_in_receive += 1
        per_partition.append({"entries": entries, "receive_hits": hits})

    return {
        "schema": "quoteNetzipRs.official_5188_receive_list_universe.v1",
        "receive_list_enabled": len(receive),
        "total_2a10_entries": total,
        "receive_hits": hit,
        "not_in_receive_list": not_in_receive,
        "not_in_flow_0104": not_in_table,
        "all_subscribed_in_receive_intersection": total > 0 and hit == total and not_in_table == 0,
        "partitions": per_partition,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--server-dir", required=True, help="extracted server payload directory")
    parser.add_argument("--client-dir", required=True, help="extracted client payload directory")
    parser.add_argument(
        "--receive-list",
        default=None,
        help="optional 只接收股票代码表.csv to verify the receive-list universe",
    )
    parser.add_argument(
        "--local-ip",
        default="192.168.3.2",
        help="local host IP embedded in extracted file names",
    )
    args = parser.parse_args()
    report = audit(args.server_dir, args.client_dir)
    if args.receive_list:
        report["receive_list_universe"] = audit_receive_list_universe(
            args.server_dir, args.client_dir, args.receive_list, args.local_ip
        )
    print(__import__("json").dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
