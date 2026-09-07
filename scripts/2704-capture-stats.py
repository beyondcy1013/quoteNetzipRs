#!/usr/bin/env python3
"""Frame-level 2704 statistics for one 5188 capture + its extract directory.

Usage:
  2704-capture-stats.py CAPTURE.ip.pcap EXTRACT_DIR OUTPUT_JSON [STEADY_LOG]

Combines three views:
1. sent-frame source of truth: the driver's own event counter from the
   steady-state acceptance log (STEADY_LOG, optional) — the live socket sees
   every frame, pcap capture holes do not; the "存活 Ns：服务端帧 X，业务帧 Y"
   line's final Y is the authoritative business-frame count;
2. pcap lower bound: a resynchronizing chain-walk of the reassembled
   server->client streams counting 2704 headers on the aligned path (capture
   holes make this a lower bound, never a failure measure);
3. exported/decoded: manifest.json + decoded-values files (strict policy
   exports complete frames only), with record mask classes (mask >> 3), the
   bucket axis used by ladder_volumes triage.

failure_upper_bound = sent_business_frames - exported_complete.
No payload content is emitted.
"""

from __future__ import annotations

import json
import struct
import sys
from collections import Counter, defaultdict
from pathlib import Path

# Protocol labels are hex digit pairs transmitted big-endian on the wire
# (label 3110 -> bytes 31 10 -> little-endian u16 0x1031). Keys here are the
# little-endian u16 values produced by struct.unpack('<H', ...).
KNOWN_KINDS = {
    0x0401,  # 0104 code table
    0x040D,  # 0d04
    0x0415,  # 1504 config file
    0x041B,  # 1b04 pinyin
    0x0421,  # 2104
    0x0427,  # 2704 realtime
    0x0428,  # 2804 block
    0x042E,  # 2e04
    0x0130,  # 3001 formula
    0x043C,  # 3c04
    0x043E,  # 3e04 finance
    0x043F,  # 3f04 news
    0x0441,  # 4104
    0x0448,  # 4804
    0x0451,  # 5104
    0x0454,  # 5404
    0x1003,  # 0310
    0x102A,  # 2a10 subscription
    0x102D,  # 2d10 handshake
    0x1031,  # 3110 control
    0x1032,  # 3210 control
    0x1036,  # 3610 ack
    0x0139,  # 3901 heartbeat
    0x013A,  # 3a01
    0x3836,  # 3638 news variant
}
KIND_2704 = 0x0427
MAX_PAYLOAD = 300_000


def assemble_downlink(path: Path) -> list[bytes]:
    data = path.read_bytes()
    flows: dict[tuple, dict[int, int]] = defaultdict(dict)
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
        src = bytes(packet[12:16])
        sport, dport = struct.unpack_from(">HH", packet, ihl)
        seq = struct.unpack_from(">I", packet, ihl + 4)[0]
        data_offset = (packet[ihl + 12] >> 4) * 4
        payload = packet[ihl + data_offset :]
        dst = bytes(packet[16:20])
        if payload and sport == 5188:
            # Keep each TCP sequence space independent. Multiple 5188 sockets
            # share the same remote endpoint, so (sport, src) can merge streams.
            flows[(src, dst, sport, dport)].setdefault(seq, {}).update(enumerate(payload))
    streams = []
    for key, segments in flows.items():
        base = min(segments)
        buffer: dict[int, int] = {}
        for seq, byte_map in segments.items():
            for rel, byte in byte_map.items():
                buffer[seq - base + rel] = byte
        streams.append(bytes(buffer[k] for k in sorted(buffer)))
    return streams


def scan_2704_headers(streams: list[bytes]) -> dict:
    """Greedy non-overlapping scan: count plausible 2704 headers whose spans do
    not collide with an already-claimed header (payload bytes can imitate
    headers, so overlap rejection keeps the count conservative). Capture holes
    make this a lower bound on sent frames, never a failure measure."""
    total = 0
    per_stream = []
    for stream in streams:
        claimed_to = -1
        count = 0
        for offset in range(max(claimed_to + 1, 0), max(len(stream) - 8, 0)):
            if offset < claimed_to:
                continue
            if stream[offset] != 0x27 or stream[offset + 1] != 0x04:
                continue
            payload_len = struct.unpack_from("<H", stream, offset + 2)[0]
            end = offset + 8 + payload_len
            if not (16 <= payload_len <= MAX_PAYLOAD and end <= len(stream)):
                continue
            count += 1
            claimed_to = end
        per_stream.append(count)
        total += count
    return {"total_headers": total, "per_stream": per_stream, "flows": len(streams)}


def sent_business_frames_from_log(path: Path) -> int | None:
    """Final 业务帧 count from a steady-state acceptance log."""
    import re

    pattern = re.compile(r"存活 \d+s：服务端帧 \d+，业务帧 (\d+)")
    count = None
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = pattern.search(line)
        if match:
            count = int(match.group(1))
    return count


def main() -> None:
    if len(sys.argv) not in (4, 5):
        raise SystemExit(__doc__)
    capture = Path(sys.argv[1])
    extract_dir = Path(sys.argv[2])
    output = Path(sys.argv[3])
    steady_log = Path(sys.argv[4]) if len(sys.argv) == 5 else None

    scan = scan_2704_headers(assemble_downlink(capture))

    manifest = json.loads((extract_dir / "manifest.json").read_text(encoding="utf-8"))
    frames_2704 = [f for f in manifest if f.get("wire_kind") == "2704"]
    decoded = [f for f in frames_2704 if f.get("decoded_values_file")]
    records = 0
    mask_classes: Counter[int] = Counter()
    raw_masks: Counter[int] = Counter()
    for frame in decoded:
        for record in json.loads(
            (extract_dir / frame["decoded_values_file"]).read_text(encoding="utf-8")
        ):
            records += 1
            mask = record.get("mask", 0)
            raw_masks[mask] += 1
            mask_classes[mask >> 3] += 1

    sent = sent_business_frames_from_log(steady_log) if steady_log else None
    exported = len(frames_2704)
    failure_ub = max(0, sent - exported) if sent is not None else None

    result = {
        "schema": "netzip.2704-capture-stats.v1",
        "capture": str(capture),
        "extract_dir": str(extract_dir),
        "steady_log": str(steady_log) if steady_log else None,
        "sent_business_frames": sent,
        "pcap_chainwalk_2704_lower_bound": scan["total_headers"],
        "pcap_downlink_flows": scan["flows"],
        "frames_exported_complete": exported,
        "frames_with_decoded_values": len(decoded),
        "records_decoded": records,
        "frame_loss_upper_bound": failure_ub,
        "mask_class_distribution": {
            str(k): v for k, v in sorted(mask_classes.items())
        },
        "raw_mask_distribution": {str(k): v for k, v in sorted(raw_masks.items())},
        "note": (
            "sent comes from the driver's live-socket counter (authoritative); "
            "pcap chain-walk is a lower bound (capture holes); "
            "frame_loss_upper_bound = sent - exported_complete conflates capture "
            "loss with decode-relevant incompleteness. mask_class = mask >> 3."
        ),
    }
    output.write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(
        f"sent={sent} pcap_lb={scan['total_headers']} exported={exported} "
        f"records={records} loss_ub={failure_ub}"
    )
    print("mask_class:", dict(sorted(mask_classes.items())))
    print(f"written: {output}")


if __name__ == "__main__":
    main()
