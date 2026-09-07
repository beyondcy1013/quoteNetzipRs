#!/usr/bin/env python3
"""Analyze the evidence-backed source of official 5188 ``2d10`` words.

The market code-table header stores the same protocol/group/version fields as
the client ``2d10`` triplet. This tool reports only those structural fields;
it does not choose a runtime connection group or send protocol traffic.
"""
from __future__ import annotations

import argparse
import csv
import datetime as dt
import struct
from pathlib import Path


TABLES = {
    "SH": ("SH\u4ee3\u7801\u8868.dat", b"SH"),
    "SZ": ("SZ\u4ee3\u7801\u8868.dat", b"SZ"),
    "4224": ("B$\u4ee3\u7801\u8868.dat", b"B$"),
}


def parse_hex_u32(value: str) -> int:
    return int(value.removeprefix("0x"), 16)


def load_csv(path: Path) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    with path.open(newline="", encoding="utf-8") as source:
        for raw in csv.DictReader(source):
            word2 = parse_hex_u32(raw["word2_hex"])
            word3 = parse_hex_u32(raw["word3_hex"])
            rows.append(
                {
                    **raw,
                    "unix_seconds": float(raw["unix_seconds"]),
                    "version_seconds": ((word3 & 0xFFFF) << 16) | (word2 >> 16),
                }
            )
    return rows


def firsts(rows: list[dict[str, object]]) -> dict[tuple[str, str], dict[str, object]]:
    result: dict[tuple[str, str], dict[str, object]] = {}
    for row in rows:
        key = (str(row["word0_hex"]), str(row["group"]))
        if key not in result or float(row["unix_seconds"]) < float(result[key]["unix_seconds"]):
            result[key] = row
    return result


def format_time(unix_seconds: int) -> str:
    return dt.datetime.fromtimestamp(unix_seconds).astimezone().isoformat(timespec="seconds")


def compare_csv(baseline: Path, current: Path) -> None:
    base = firsts(load_csv(baseline))
    cur = firsts(load_csv(current))
    print("market,group,capture_delta,version_delta,baseline_version,current_version")
    for key in sorted(base):
        if key not in cur:
            continue
        before = base[key]
        after = cur[key]
        capture_delta = float(after["unix_seconds"]) - float(before["unix_seconds"])
        version_delta = int(after["version_seconds"]) - int(before["version_seconds"])
        print(
            f"{key[0]},{key[1]},{capture_delta:.6f},{version_delta},"
            f"{int(before['version_seconds'])},{int(after['version_seconds'])}"
        )


def read_code_table(path: Path, expected_market: bytes) -> dict[str, int]:
    data = path.read_bytes()
    if len(data) < 22:
        raise ValueError(f"{path}: code table is shorter than the 22-byte outer/header prefix")
    if data[:2] != b"\x01\x04":
        raise ValueError(f"{path}: expected 0104 outer object")
    declared = int.from_bytes(data[2:6], "little")
    if declared + 8 != len(data):
        raise ValueError(f"{path}: declared body {declared} does not match file length {len(data)}")
    protocol, group_tag, version_low, version_high = struct.unpack_from("<4H", data, 8)
    market = data[20:22]
    if market != expected_market:
        raise ValueError(f"{path}: expected market {expected_market!r}, got {market!r}")
    return {
        "protocol": protocol,
        "group_tag": group_tag,
        "version_seconds": (version_high << 16) | version_low,
    }


def inspect_code_tables(directory: Path, trading_day: int | None) -> None:
    print(
        "market,protocol,group_tag,version_seconds,version_time,"
        "word0,word1,word2,word3"
    )
    for label, (filename, market) in TABLES.items():
        fields = read_code_table(directory / filename, market)
        protocol = fields["protocol"]
        group_tag = fields["group_tag"]
        version_seconds = fields["version_seconds"]
        word0 = int.from_bytes(market + protocol.to_bytes(2, "little"), "little")
        word1 = "unresolved"
        if trading_day is not None:
            word1 = f"{(((trading_day & 0xFFFF) << 16) | group_tag):08x}"
        word2 = ((version_seconds & 0xFFFF) << 16) | 0x0135
        word3 = version_seconds >> 16
        print(
            f"{label},{protocol:04x},{group_tag:04x},{version_seconds},"
            f"{format_time(version_seconds)},{word0:08x},{word1},{word2:08x},{word3:08x}"
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("baseline", type=Path, nargs="?")
    parser.add_argument("current", type=Path, nargs="?")
    parser.add_argument("--code-table-dir", type=Path)
    parser.add_argument("--trading-day", type=int)
    args = parser.parse_args()

    if args.code_table_dir is not None:
        if args.baseline is not None or args.current is not None:
            parser.error("CSV operands and --code-table-dir are mutually exclusive")
        inspect_code_tables(args.code_table_dir, args.trading_day)
        return
    if args.baseline is None or args.current is None:
        parser.error("provide BASELINE CURRENT CSV files or --code-table-dir")
    compare_csv(args.baseline, args.current)


if __name__ == "__main__":
    main()
