#!/usr/bin/env python3
"""Create a structurally useful pcapng without retaining application payloads."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from datetime import datetime, timezone
from pathlib import Path

from scapy.all import Ether, IP, IPv6, Raw, TCP, PcapNgWriter


def iter_packet_blocks(path: Path):
    data = path.read_bytes()
    offset = 0
    endian = "<"
    packet_index = 0
    while offset + 12 <= len(data):
        block_type = data[offset : offset + 4]
        if block_type == b"\x0a\x0d\x0d\x0a":
            magic = data[offset + 8 : offset + 12]
            endian = "<" if magic == b"\x4d\x3c\x2b\x1a" else ">"
            total = struct.unpack_from(endian + "I", data, offset + 4)[0]
            if total < 12 or offset + total > len(data):
                break
            offset += total
            continue

        block_id = struct.unpack_from(endian + "I", data, offset)[0]
        total = struct.unpack_from(endian + "I", data, offset + 4)[0]
        if total < 12 or offset + total > len(data):
            break
        if block_id == 6 and total >= 32:  # Enhanced Packet Block.
            timestamp_hi, timestamp_lo, captured_len = struct.unpack_from(
                endian + "III", data, offset + 12
            )
            raw = data[offset + 28 : offset + 28 + captured_len]
            packet_index += 1
            yield offset, packet_index, (timestamp_hi << 32) | timestamp_lo, raw
        elif block_id == 3 and total >= 16:  # Simple Packet Block.
            captured_len = total - 16
            raw = data[offset + 12 : offset + 12 + captured_len]
            packet_index += 1
            yield offset, packet_index, None, raw
        offset += total


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--client-ip", required=True)
    parser.add_argument("--ports", required=True, help="Comma-separated TCP ports")
    parser.add_argument("--remote-ips", required=True, help="Comma-separated server IPs")
    return parser.parse_args()


def timestamp_text(raw_ticks: int | None) -> str | None:
    if raw_ticks is None:
        return None
    return datetime.fromtimestamp(raw_ticks / 1_000_000, timezone.utc).isoformat()


def main() -> None:
    args = parse_args()
    ports = {int(value) for value in args.ports.split(",") if value}
    remote_ips = {value.strip() for value in args.remote_ips.split(",") if value.strip()}
    stream_bases: dict[tuple[str, int, str, int], int] = {}
    records: list[dict[str, object]] = []

    args.output.parent.mkdir(parents=True, exist_ok=True)
    writer = PcapNgWriter(str(args.output))
    try:
        for raw_offset, packet_index, raw_ticks, raw_frame in iter_packet_blocks(args.input):
            try:
                packet = Ether(raw_frame)
            except Exception:
                continue
            if TCP not in packet:
                continue
            if IP in packet:
                ip = packet[IP]
            elif IPv6 in packet:
                ip = packet[IPv6]
            else:
                continue
            tcp = packet[TCP]
            src_port, dst_port = int(tcp.sport), int(tcp.dport)
            if src_port not in ports and dst_port not in ports:
                continue
            if ip.src != args.client_ip and ip.dst != args.client_ip:
                continue
            server_ip = ip.dst if ip.src == args.client_ip else ip.src
            if remote_ips and server_ip not in remote_ips:
                continue

            payload = bytes(tcp.payload)
            flow = (ip.src, src_port, ip.dst, dst_port)
            direction = "outbound" if ip.src == args.client_ip else "inbound"
            # Handshake-only packets do not advance the application stream.
            # Anchor offsets at the first segment carrying payload so the
            # retained 7100 prefix maps to the actual wire data.
            if payload and flow not in stream_bases:
                stream_bases[flow] = int(tcp.seq)
            stream_base = stream_bases.get(flow, int(tcp.seq))
            stream_offset = max(0, int(tcp.seq) - stream_base)
            allowed_prefix = 0
            if 7100 in (src_port, dst_port):
                allowed_prefix = 168 if direction == "outbound" else 74
            keep_start = max(0, min(len(payload), allowed_prefix - stream_offset))
            masked = payload[:keep_start] + (b"X" * (len(payload) - keep_start))
            if payload:
                packet[TCP].remove_payload()
                packet[TCP].add_payload(Raw(masked))
                packet[TCP].chksum = None
                if IP in packet:
                    packet[IP].chksum = None
                elif IPv6 in packet:
                    packet[IPv6].plen = None
            packet.time = raw_ticks / 1_000_000 if raw_ticks is not None else packet.time
            writer.write(packet)
            records.append(
                {
                    "packet_index": packet_index,
                    "raw_block_offset": raw_offset,
                    "timestamp": timestamp_text(raw_ticks),
                    "direction": direction,
                    "src": f"{ip.src}:{src_port}",
                    "dst": f"{ip.dst}:{dst_port}",
                    "tcp_flags": str(tcp.flags),
                    "tcp_seq": int(tcp.seq),
                    "tcp_ack": int(tcp.ack),
                    "wire_length": len(raw_frame),
                    "payload_length": len(payload),
                    "retained_prefix": keep_start,
                    "raw_payload_sha256": hashlib.sha256(payload).hexdigest(),
                    "sanitized_payload_sha256": hashlib.sha256(masked).hexdigest(),
                }
            )
    finally:
        writer.close()

    metadata = {
        "input": str(args.input),
        "input_sha256": hashlib.sha256(args.input.read_bytes()).hexdigest(),
        "output": str(args.output),
        "output_sha256": hashlib.sha256(args.output.read_bytes()).hexdigest(),
        "payload_policy": "retain only the fixed 7100 outer/header prefix; replace all other TCP payload bytes with 0x58",
        "frame_count": len(records),
        "frames": records,
    }
    args.metadata.write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    metadata["output_sha256"] = hashlib.sha256(args.output.read_bytes()).hexdigest()
    args.metadata.write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: metadata[key] for key in ("input_sha256", "output_sha256", "frame_count")}, ensure_ascii=False))


if __name__ == "__main__":
    main()
