#!/usr/bin/env python3
"""Entry-level 2a10 byte-parity compare for the synchronized dual-session capture.

Usage:
  compare-2a10-sync.py CAPTURE_PCAP A_PAYLOAD_DIR|CAPTURE_PCAP_2 OUTPUT_JSON [META_A_DIR META_B_DIR]

With the optional `META_A_DIR META_B_DIR` pair, the script also performs a
semantic compare: each wire value is resolved through that side's own 0104
`*-0104.code-table.json` files (`wire_value & 0x7fff` -> symbol index -> code).
This separates real subscription membership differences from index drift caused
by two sessions receiving different 0104 table versions.
"""

from __future__ import annotations

import json
import struct
import sys
from collections import OrderedDict
from pathlib import Path

KIND_2A10 = 0x102A
FRAME_HEADER_LEN = 8


def load_uplink_streams(path: Path) -> list[bytes]:
    data = path.read_bytes()
    if len(data) < 24:
        raise SystemExit(f"{path}: too small to be a pcap")
    streams: OrderedDict[int, bytearray] = OrderedDict()
    offset = 24
    while offset + 16 <= len(data):
        _ts, _tus, incl, _orig = struct.unpack_from("<IIII", data, offset)
        offset += 16
        packet = data[offset : offset + incl]
        offset += incl
        if len(packet) < 20 or packet[0] >> 4 != 4:
            continue
        ihl = (packet[0] & 0xF) * 4
        if packet[9] != 6 or len(packet) < ihl:
            continue
        sport, dport = struct.unpack_from(">HH", packet, ihl)
        data_offset = (packet[ihl + 12] >> 4) * 4
        payload = packet[ihl + data_offset :]
        if payload and dport == 5188:
            streams.setdefault(sport, bytearray()).extend(payload)
    return [bytes(stream) for stream in streams.values()]


def entries_from_payload(payload: bytes) -> list[tuple[str, int]]:
    entries = []
    for off in range(10, len(payload), 6):
        market = payload[off : off + 2].decode("ascii", "replace")
        value = struct.unpack_from("<I", payload, off + 2)[0]
        entries.append((market, value))
    return entries


def partitions_from_capture(path: Path) -> list[list[tuple[str, int]]]:
    partitions: list[list[tuple[str, int]]] = []
    seen: set[bytes] = set()
    for stream in load_uplink_streams(path):
        offset = 0
        while offset + FRAME_HEADER_LEN <= len(stream):
            kind, payload_len = struct.unpack_from("<HH", stream, offset)
            end = offset + FRAME_HEADER_LEN + payload_len
            if end > len(stream):
                break
            if kind == KIND_2A10:
                payload = stream[offset + FRAME_HEADER_LEN : end]
                if payload not in seen:
                    seen.add(payload)
                    partitions.append(entries_from_payload(payload))
            offset = end
    return partitions


def partitions_from_bin_dir(directory: Path) -> list[list[tuple[str, int]]]:
    partitions = []
    for path in sorted(directory.glob("*-2a10.payload.bin")):
        raw = path.read_bytes()
        if len(raw) < 10 or (len(raw) - 10) % 6:
            raise SystemExit(f"{path}: invalid 2a10 payload length {len(raw)}")
        declared = struct.unpack_from("<I", raw, 4)[0]
        actual = (len(raw) - 10) // 6
        if declared != actual:
            raise SystemExit(f"{path}: declared {declared} entries but carries {actual}")
        partitions.append(entries_from_payload(raw))
    return partitions


def load_code_maps(directory: Path) -> dict[str, dict[int, str]]:
    maps: dict[str, dict[int, str]] = {}
    for path in sorted(directory.glob("*-0104.code-table.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        market = "".join(chr(value) for value in data["market"])
        maps[market] = {
            int(row["symbol_index"]): row["code"] for row in data["records"]
        }
    if not maps:
        raise SystemExit(f"{directory}: no *-0104.code-table.json files")
    return maps


def resolve_partition_codes(
    partitions: list[list[tuple[str, int]]], maps: dict[str, dict[int, str]]
) -> list[list[tuple[str, str]]]:
    resolved = []
    for partition in partitions:
        current = []
        for market, wire_value in partition:
            index = wire_value & 0x7FFF
            code = maps.get(market, {}).get(index)
            if code is None:
                raise SystemExit(
                    f"0104 map missing {market} index {index} (wire {wire_value})"
                )
            current.append((market, code))
        resolved.append(current)
    return resolved


def semantic_compare(
    parts_a: list[list[tuple[str, str]]], parts_b: list[list[tuple[str, str]]]
) -> dict:
    used: set[int] = set()
    mapping = []
    for bi, part_b in enumerate(parts_b):
        candidates = [
            (len(set(parts_a[ai]) & set(part_b)), ai)
            for ai in range(len(parts_a))
            if ai not in used
        ]
        if not candidates:
            mapping.append({"official_index": bi, "semantic_live_index": None})
            continue
        overlap, ai = max(candidates)
        used.add(ai)
        part_a = parts_a[ai]
        positional = sum(x == y for x, y in zip(part_a, part_b))
        mapping.append(
            {
                "official_index": bi,
                "semantic_live_index": ai,
                "overlap": overlap,
                "size_b": len(part_b),
                "positional_match": positional,
                "first_diff": next(
                    (j for j, (x, y) in enumerate(zip(part_a, part_b)) if x != y),
                    None,
                ),
                "semantic_equal": positional == len(part_b) == len(part_a),
            }
        )
    total = sum(item.get("size_b", 0) for item in mapping)
    matched = sum(item.get("positional_match", 0) for item in mapping)
    set_a = {entry for part in parts_a for entry in part}
    set_b = {entry for part in parts_b for entry in part}
    return {
        "sizes_a": [len(part) for part in parts_a],
        "sizes_b": [len(part) for part in parts_b],
        "mapping": mapping,
        "positional_match": matched,
        "total_entries_b": total,
        "match_rate": round(matched / total, 6) if total else None,
        "a_only": len(set_a - set_b),
        "b_only": len(set_b - set_a),
        "semantic_byte_equal": (
            len(parts_a) == len(parts_b)
            and all(item.get("semantic_equal") for item in mapping)
            and set_a == set_b
        ),
    }


def main() -> None:
    if len(sys.argv) not in (4, 6):
        raise SystemExit(__doc__)
    side_a = Path(sys.argv[1])
    side_b = Path(sys.argv[2])
    output = Path(sys.argv[3])
    meta_a = load_code_maps(Path(sys.argv[4])) if len(sys.argv) == 6 else None
    meta_b = load_code_maps(Path(sys.argv[5])) if len(sys.argv) == 6 else None

    parts_a = partitions_from_capture(side_a) if side_a.suffix in (".pcap",) else partitions_from_bin_dir(side_a)
    parts_b = partitions_from_bin_dir(side_b) if side_b.is_dir() else partitions_from_capture(side_b)

    result = {
        "schema": "netzip.2a10-sync-compare.v2",
        "side_a": str(side_a),
        "side_b": str(side_b),
        "sizes_a": [len(p) for p in parts_a],
        "sizes_b": [len(p) for p in parts_b],
    }

    set_a: set[tuple[str, int]] = set()
    set_b: set[tuple[str, int]] = set()
    for partition in parts_a:
        set_a.update(partition)
    for partition in parts_b:
        set_b.update(partition)
    result["set_a"] = len(set_a)
    result["set_b"] = len(set_b)
    result["a_only"] = len(set_a - set_b)
    result["b_only"] = len(set_b - set_a)

    mapping = []
    used: set[int] = set()
    for bi, part_b in enumerate(parts_b):
        best = None
        for ai, part_a in enumerate(parts_a):
            if ai in used:
                continue
            overlap = len(set(part_a) & set(part_b))
            if best is None or overlap > best[1]:
                best = (ai, overlap)
        if best is None:
            mapping.append({"official_index": bi, "live_index": None, "overlap": 0})
            continue
        used.add(best[0])
        part_a = parts_a[best[0]]
        positional = sum(1 for x, y in zip(part_a, part_b) if x == y)
        first_diff = next(
            (j for j, (x, y) in enumerate(zip(part_a, part_b)) if x != y), None
        )
        mapping.append(
            {
                "official_index": bi,
                "live_index": best[0],
                "overlap": best[1],
                "size_b": len(part_b),
                "positional_match": positional,
                "first_diff": first_diff,
                "byte_equal": positional == len(part_b) == len(part_a),
            }
        )
    result["partition_mapping"] = mapping

    total = sum(m["size_b"] for m in mapping if m["live_index"] is not None)
    matched = sum(m["positional_match"] for m in mapping if m["live_index"] is not None)
    byte_equal = all(m.get("byte_equal") for m in mapping) and len(parts_a) == len(parts_b)
    result["positional_match"] = matched
    result["total_entries_b"] = total
    result["match_rate"] = round(matched / total, 6) if total else None

    if byte_equal and result["a_only"] == 0 and result["b_only"] == 0:
        result["verdict"] = "byte-equal: all partitions positionally identical, sets identical"
    elif byte_equal:
        result["verdict"] = "positionally equal but set-differing entries exist outside partitions"
    else:
        result["verdict"] = (
            "not byte-equal: check partition_mapping (ordering/table-version drift) "
            "and a_only/b_only membership deltas"
        )

    if meta_a is not None and meta_b is not None:
        semantic = semantic_compare(
            resolve_partition_codes(parts_a, meta_a),
            resolve_partition_codes(parts_b, meta_b),
        )
        result["semantic_compare"] = semantic
        result["semantic_verdict"] = (
            "semantic-byte-equal: same market/code sequence"
            if semantic["semantic_byte_equal"]
            else "semantic-different: real membership/order difference remains"
        )

    output.write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(
        f"sizes_a={result['sizes_a']} sizes_b={result['sizes_b']} "
        f"match={matched}/{total} ({result['match_rate']}) a_only={result['a_only']} b_only={result['b_only']}"
    )
    print(f"verdict: {result['verdict']}")
    if "semantic_verdict" in result:
        print(f"semantic_verdict: {result['semantic_verdict']}")
    print(f"written: {output}")


if __name__ == "__main__":
    main()
