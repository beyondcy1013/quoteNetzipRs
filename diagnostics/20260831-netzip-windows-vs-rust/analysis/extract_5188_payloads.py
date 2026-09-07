#!/usr/bin/env python3
"""Extract bounded raw 5188 payload samples for inner-object analysis.

This tool preserves bytes and framing only. It does not guess a codec or map
payloads to quote fields. pcapng input is normalized by tcpdump, then parsed
with the Python standard library so the fixture remains usable without Scapy.
"""
from __future__ import annotations

import argparse
import hashlib
import struct
import subprocess
import tempfile
from collections import defaultdict
from pathlib import Path


def pcap_bytes(path: Path) -> bytes:
    raw = path.read_bytes()
    if raw[:4] != b"\x0a\x0d\x0d\x0a":
        return raw
    with tempfile.NamedTemporaryFile(suffix=".pcap") as output:
        subprocess.run(["tcpdump", "-r", str(path), "-w", output.name], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        output.seek(0)
        return output.read()


def pcap_layout(raw: bytes) -> tuple[str, float, int]:
    layouts = {
        b"\xd4\xc3\xb2\xa1": ("<", 1_000_000.0),
        b"\xa1\xb2\xc3\xd4": (">", 1_000_000.0),
        b"\x4d\x3c\xb2\xa1": ("<", 1_000_000_000.0),
        b"\xa1\xb2\x3c\x4d": (">", 1_000_000_000.0),
    }
    try:
        endian, timestamp_scale = layouts[raw[:4]]
    except KeyError as error:
        raise ValueError("unsupported classic pcap byte order or timestamp format") from error
    if len(raw) < 24:
        raise ValueError("truncated classic pcap header")
    return endian, timestamp_scale, struct.unpack_from(f"{endian}I", raw, 20)[0]


def ipv4_offset(frame: bytes, link_type: int) -> int | None:
    if link_type in (12, 101, 228):  # DLT_RAW / DLT_RAW_BSD / DLT_IPV4
        return 0
    if link_type == 276:  # Linux cooked capture v2
        return 20 if len(frame) >= 20 and frame[:2] == b"\x08\x00" else None
    if link_type == 113:  # Linux cooked capture v1
        return 16 if len(frame) >= 16 and frame[14:16] == b"\x08\x00" else None
    if link_type != 1 or len(frame) < 14:  # Ethernet
        return None
    offset = 14
    ether_type = frame[12:14]
    while ether_type in (b"\x81\x00", b"\x88\xa8") and len(frame) >= offset + 4:
        ether_type = frame[offset + 2:offset + 4]
        offset += 4
    return offset if ether_type == b"\x08\x00" else None


def packets(raw: bytes):
    endian, timestamp_scale, link_type = pcap_layout(raw)
    pos = 24
    while pos + 16 <= len(raw):
        timestamp_seconds, timestamp_fraction, incl, _ = struct.unpack_from(
            f"{endian}IIII", raw, pos
        )
        pos += 16
        frame = raw[pos:pos + incl]
        pos += incl
        ip = ipv4_offset(frame, link_type)
        if ip is None or len(frame) < ip + 20 or frame[ip] >> 4 != 4:
            continue
        ihl = (frame[ip] & 0x0F) * 4
        if ihl < 20 or len(frame) < ip + ihl + 20 or frame[ip + 9] != 6:
            continue
        tcp = ip + ihl
        src = ".".join(map(str, frame[ip + 12:ip + 16]))
        dst = ".".join(map(str, frame[ip + 16:ip + 20]))
        sport, dport, seq = struct.unpack_from("!HHI", frame, tcp)
        tcp_len = ((frame[tcp + 12] >> 4) & 0x0F) * 4
        if tcp_len < 20 or len(frame) < tcp + tcp_len:
            continue
        payload = frame[tcp + tcp_len:]
        if payload and (sport == 5188 or dport == 5188):
            timestamp = timestamp_seconds + timestamp_fraction / timestamp_scale
            yield (src, sport, dst, dport, seq, payload, timestamp)


def extract(path: Path, out_dir: Path, limit: int, direction: str, full_frame: bool) -> None:
    flows = defaultdict(dict)
    for src, sport, dst, dport, seq, payload, timestamp in packets(pcap_bytes(path)):
        is_server = sport == 5188
        if direction == "server" and not is_server:
            continue
        if direction == "client" and (is_server or dport != 5188):
            continue
        key = (src, sport, dst, dport)
        segment_key = (seq, payload)
        previous = flows[key].get(segment_key)
        if previous is None or timestamp < previous[1]:
            flows[key][segment_key] = (payload, timestamp)
    out_dir.mkdir(parents=True, exist_ok=True)
    manifest = []
    for flow, segments in sorted(flows.items()):
        stream_parts = []
        stream_segments = []
        expected = None
        stream_len = 0
        for (seq, _), (payload, timestamp) in sorted(segments.items()):
            if expected is None:
                expected = seq
            if seq > expected:
                break
            overlap = max(0, expected - seq)
            if overlap < len(payload):
                chunk = payload[overlap:]
                stream_parts.append(chunk)
                stream_segments.append((stream_len, stream_len + len(chunk), timestamp))
                stream_len += len(chunk)
                expected = seq + len(payload)
        stream = b"".join(stream_parts)
        pos = 0
        count = 0
        while pos + 8 <= len(stream) and count < limit:
            wire_kind = stream[pos:pos + 2].hex()
            payload_len = int.from_bytes(stream[pos + 2:pos + 4], "little")
            if pos + 16 <= len(stream):
                _, compressed_len = struct.unpack_from("<II", stream, pos + 8)
                extended_len = 8 + compressed_len
                zlib_marker = stream[pos + 16:pos + 17]
                if (
                    extended_len > payload_len
                    and extended_len <= 1_048_576
                    and extended_len % 65_536 == payload_len
                    and pos + 8 + extended_len <= len(stream)
                    and zlib_marker == b"\x78"
                ):
                    payload_len = extended_len
            end = pos + 8 + payload_len
            if payload_len > 1_048_576 or end > len(stream):
                break
            frame = stream[pos:end]
            completed_at = max(
                timestamp
                for start, stop, timestamp in stream_segments
                if start < end and stop > pos
            )
            output = frame if full_frame else frame[8:]
            target = out_dir / f"{flow[0]}_{flow[1]}_{flow[2]}_{flow[3]}_{wire_kind}_{count:04d}.bin"
            target.write_bytes(output)
            manifest.append({
                "direction": direction,
                "flow": f"{flow[0]}:{flow[1]}->{flow[2]}:{flow[3]}",
                "wire_kind": wire_kind,
                "metadata_hex": frame[4:8].hex(),
                "payload_len": payload_len,
                "completed_at_unix": completed_at,
                "output_kind": "full-frame" if full_frame else "payload",
                "sha256": hashlib.sha256(output).hexdigest(),
                "path": str(target),
            })
            pos = end
            count += 1
    (out_dir / "manifest.json").write_text(
        __import__("json").dumps(manifest, ensure_ascii=True, indent=2) + "\n",
        encoding="utf-8",
    )
    summary = {
        "schema": "quoteNetzipRs.official_5188_extract.v1",
        "capture_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "direction": direction,
        "output_kind": "full-frame" if full_frame else "payload",
        "record_count": len(manifest),
        "records": manifest,
    }
    (out_dir / "extract-summary.json").write_text(
        __import__("json").dumps(summary, ensure_ascii=True, indent=2) + "\n",
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("capture", type=Path)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--limit", type=int, default=32)
    parser.add_argument("--direction", choices=("client", "server"), default="server")
    parser.add_argument("--full-frame", action="store_true")
    args = parser.parse_args()
    extract(args.capture, args.out_dir, args.limit, args.direction, args.full_frame)


if __name__ == "__main__":
    main()
