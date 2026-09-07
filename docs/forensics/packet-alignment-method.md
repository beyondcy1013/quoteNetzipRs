# Wine/Rust Packet Alignment Method

This document defines the evidence format for comparing quoteNetzipWine with
quoteNetzipRs. It is an exploration-stage procedure; it does not promote a
port or decoder without payload and callback evidence.

## Required Capture

Start packet capture before Wine login and stop it after the callback window.
Retain the pcap/pcapng, Wine status snapshot, callback JSONL, Rust logs, and
SHA-256 values in one dated directory. Use Unix milliseconds or microseconds
for every event.

## Stream Identity

Partition traffic by protocol five-tuple (`src`, `src_port`, `dst`,
`dst_port`, `protocol`) and assign a lifecycle-scoped `stream_id`. Keep 6100,
7100, 5188, 16801, and 7709 in separate partitions.

## TCP and Application Alignment

For each stream, sort by TCP sequence, remove retransmitted segments, record
gaps and truncation, and emit only complete application packets. Normalize away
link/IP/TCP headers and pktmon metadata, but retain network-packet fields,
direction, wire kind, metadata, payload length, compression hint, and payload
offset. Never compare raw pcap bytes directly.

Each normalized frame should expose only non-sensitive evidence:

```text
stream_id, direction, wire_kind, payload_len, metadata_len,
payload_hint, compression_offset, normalized_payload_sha256
```

## Callback Correlation

Associate a callback with a frame only when the timestamps overlap, the login
lifecycle is the same, the source stream is the same, and symbol/batch and
field values agree. A length or arrival-order match alone is insufficient.

## Disposition

Every result must be one of `confirmed`, `rejected`, or `needs-verification`.
The current known shape is that 5188 samples contain 3901 control frames,
6100 may contain authentication/download ZSTD traffic, 16801 carries local
callback traffic, and 7709 is supplement-only. These labels remain subject to
new capture evidence.

