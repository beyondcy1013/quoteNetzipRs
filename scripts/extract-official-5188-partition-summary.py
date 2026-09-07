#!/usr/bin/env python3
"""Extract a redacted 2a10 partition summary from a 5188 packet capture.

Accepts classic pcap files (as produced by pcapng_to_pcap.py or pktmon's
etl2pcap, whose payload is raw IPv4). Reassembles client-to-server TCP
streams, walks the 8-byte official application framing, and emits the same
partition JSON shape consumed by
verify-official-5188-partition-summary.py. Only partition topology is
emitted; no packet payload content is printed.
"""

from __future__ import annotations

import argparse
import json
import struct
from collections import OrderedDict
from pathlib import Path

KIND_2A10 = 0x102A
FRAME_HEADER_LEN = 8


def load_packets(path: Path) -> list[tuple[str, int, str, int, bytes]]:
    """Yields (src, sport, dst, dport, payload) from a raw-IPv4 pcap."""
    data = path.read_bytes()
    if len(data) < 24:
        raise ValueError("capture is too short to be a pcap file")
    packets = []
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
        src = ".".join(map(str, packet[12:16]))
        dst = ".".join(map(str, packet[16:20]))
        sport, dport = struct.unpack_from(">HH", packet, ihl)
        data_offset = (packet[ihl + 12] >> 4) * 4
        payload = packet[ihl + data_offset :]
        if payload:
            packets.append((src, sport, dst, dport, payload))
    return packets


def reassemble_uplinks(packets: list[tuple[str, int, str, int, bytes]]) -> list[bytes]:
    streams: OrderedDict[tuple, bytearray] = OrderedDict()
    for src, sport, dst, dport, payload in packets:
        if dport != 5188:
            continue
        streams.setdefault((src, sport, dst, dport), bytearray()).extend(payload)
    return [bytes(stream) for stream in streams.values()]


def extract_partitions(stream: bytes) -> list[dict]:
    partitions: list[dict] = []
    seen: set[bytes] = set()
    offset = 0
    while offset + FRAME_HEADER_LEN <= len(stream):
        kind, payload_len = struct.unpack_from("<HH", stream, offset)
        frame_end = offset + FRAME_HEADER_LEN + payload_len
        if frame_end > len(stream):
            break
        if kind == KIND_2A10:
            payload = stream[offset + FRAME_HEADER_LEN : frame_end]
            # Senders repeat partitions (the official client retransmits each
            # connection's frame, a full re-send replays every partition).
            # Keep first occurrences in transmission order only.
            if payload not in seen:
                seen.add(payload)
                partitions.append(parse_partition_payload(payload))
        offset = frame_end
    return partitions


def parse_partition_payload(payload: bytes) -> dict:
    if len(payload) < 10 or (len(payload) - 10) % 6:
        raise ValueError(f"2a10 payload length {len(payload)} is not a valid partition")
    declared = struct.unpack_from("<I", payload, 4)[0]
    entries = (len(payload) - 10) // 6
    if declared != entries:
        raise ValueError(f"2a10 declared {declared} entries but carries {entries}")
    ranges = []
    previous = None
    for offset in range(10, len(payload), 6):
        market = payload[offset : offset + 2].decode("ascii", "replace")
        value = struct.unpack_from("<I", payload, offset + 2)[0]
        if previous and previous[0] == market and previous[1] + previous[2] == value:
            previous[2] += 1
        else:
            previous = [market, value, 1]
            ranges.append(previous)
    return {"entries": entries, "ranges": ranges}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    partitions = []
    for stream in reassemble_uplinks(load_packets(args.capture)):
        partitions.extend(extract_partitions(stream))
    if not partitions:
        raise SystemExit("no 2a10 frames found in the capture")
    args.output.write_text(json.dumps(partitions, indent=1), encoding="utf-8")
    sizes = [partition["entries"] for partition in partitions]
    print(f"partitions={len(sizes)} sizes={sizes} total={sum(sizes)}")


if __name__ == "__main__":
    main()
