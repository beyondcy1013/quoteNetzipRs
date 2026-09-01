# 5188 Frame Boundary and Session Shape

Status: **exploration / evidence-building**  
Date: 2026-09-01 (Asia/Shanghai)

## Source fixtures

- `diagnostics/20260831-netzip-windows-vs-rust/analysis/vendor_pm_5188_detail.txt`
- `diagnostics/20260831-netzip-windows-vs-rust/analysis/vendor_pm_5188_frames.txt`
- Capture inventory and hashes: `docs/forensics/live-market-capture-inventory.md`

The notes below are observations from the captured Wine process. They are not
runtime defaults and do not identify a business field without a same-symbol
callback comparison.

## Observed client shape

The long-lived client connections sent the following application-frame kinds:

```text
0x3610 x3
0x2d10 x3
0x0710 optional
0x2a10 optional
```

The 0x0710 and 0x2a10 frames are not present on every connection. The client
payload sizes in the representative sessions were 40, 103, 102, 75 and either
20 or 750/6162 bytes. The 8-byte header was preserved in all analysis; bytes
4..7 are metadata and are not assumed to be zero.

## Observed server shape

Representative long-lived sessions lasted about 3,355 seconds. The server
sent thousands of frames while the client sent only the small initialization
sequence. The most frequent server kind was `0x2704` (22,268 frames in the
aggregate summary); other recurring kinds were:

```text
0x0d04, 0x5404, 0x2104, 0x3e04, 0x3901
```

Payload lengths varied from small status frames to multi-kilobyte blocks (the
summary includes 5,140, 5,552 and 5,840-byte application payloads). Several
connections also contained additional kinds whose first two bytes do not match
the recurring classes. Those bytes remain opaque until the inner codec is
identified.

## Session-shape caveat

The capture contains connections with no client-to-server packets but with
server-to-client frames (for example local ports 10404 through 10420). This is
an observation about the capture topology only. It may represent an inherited,
already-initialized socket, capture start after the client handshake, or another
process/session role. It must not be used to infer that a Rust connection can
skip authentication or initialization.

## Current Rust boundary

`crates/netzip-fullpull/src/official_5188.rs` correctly handles:

- 8-byte frame headers and little-endian payload length;
- TCP segmentation and multiple frames per read;
- metadata preservation and bounded payloads;
- known-direction classification and Wine-shaped client-sequence checks.

It intentionally retains inner payload bytes. No `0x2704` payload is currently
treated as a 7709 record, compressed object, or quote update. The implementation
therefore remains transport-complete but business-decoder incomplete.

## Discriminating next experiment

The reproducible Rust-side entry point for this experiment is:

```http
POST /api/debug/pcap-5188-summary
Content-Type: application/json

{"path":"diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng"}
```

The response is intentionally bounded to per-flow counts and at most sixteen
frame samples. Save the JSON response beside the dated forensic note and record
its SHA-256; do not promote sample bytes to runtime defaults.

The scanner deduplicates pktmon packet identities before reassembly, stops at a
TCP sequence gap, reports `trailing_bytes` for an incomplete final frame, and
accepts both Ethernet+IPv4 and bare IPv4 packet bodies. These properties are
transport evidence only; they do not establish the inner 5188 codec.

WebClx request `185048-18d115af25cb0410` passed the bounded IPv4/TCP offset
regression, and request `185129-18d115af25cb0411` passed the ignored real
`vendor_pm` replay (`status=0`, 5.57s). The replay confirms that pktmon-style
records can be parsed without treating the Ethernet label as authoritative;
flows with no complete application frame remain explicitly represented with
trailing-byte/reassembly evidence rather than being promoted to quote data.
Request `185331-18d115af25cb0412` adds a fixture with a 24-byte pktmon metadata
prefix before bare IPv4/TCP and passes all seven `debug_pcap` tests. This
closes the regression boundary for the observed all-port capture shape.

## Representation note

The 5188 kind field is little-endian on the wire. Therefore raw bytes `0d04`
decode to numeric value `0x040d`. Rust summaries now report both fields:
`kind` is the numeric value and `wire_kind` is the byte-order-preserving hex
string. Existing protocol constants and matching logic continue to use the
numeric value; only presentation was made explicit after the real-fixture
replay exposed the ambiguity.

The replay also showed that earlier constants had accidentally used wire
spelling as numeric values, causing server frames to be classified as
unknown. Constants and direction tests now use decoded numeric values while
`wire_kind` retains capture spelling.

The real replay also contained zero-payload SYN/ACK records. Including them in
the application sequence walk produced a false initial one-byte gap and no
frames. The scanner now filters empty TCP payloads before reassembly; the same
fixture produces classified server frames, including wire kinds `3e04`,
`0d04`, and `5404`, while preserving genuine gap diagnostics.

For `3e04`, extracted samples are consistently 5132 bytes: a 12-byte raw
header (three little-endian words) plus a 5120-byte body. The Rust
`Official5188BulkEnvelope` parser validates only this shape and preserves all
bytes. Representative long-lived flows contain 73 complete envelopes; the
three words remain `needs-verification` for business semantics.

The same bounded extraction observed five distinct `3e04` payload hashes and
two variants each of the 18-byte `0d04` and 20-byte `5404` payloads, repeated
across ten connections. Treat this as evidence of shared/broadcast content,
not as proof of a particular security-code or quote-field layout.

Searching sampled `3e04` bodies for direct little-endian float encodings from
the available Wine `OEM_REPORT` fixture (including `SH000001` price and volume)
found no matches. The body therefore must not be treated as a plain OEM array;
compression, encryption or another packing layer is still `needs-verification`.

The replayed Wine sessions contain one `2a10` subscription frame per socket.
The observed entry counts are `1024` on the primary long-lived sockets and
`122` on a reduced socket, confirming that subscription size is session-specific.

The post-range-change replay passed the ignored Rust fixture test and exposed
the expected consecutive-range record for the 1024-entry SH subscription.
The same replay includes unrelated/unknown wire kinds and truncated tails on
some flows, so the range result is only a subscription-shape observation; it
does not establish a public symbol mapping or complete market coverage.

An embedded-zlib candidate scan was also run against the reassembled payloads.
The formal fixture shows that block offset zero begins with a 16-byte embedded
`1504` object header; offset 16 is `78 9c`. Feeding that point and the next
block at offset 5120 to a streaming zlib decoder yields 15,764 bytes before the
captured prefix ends. The decompressed bytes begin with
`.//update//stkinfo6.fin`.

The recurring `3e04` blocks are therefore a contiguous file-distribution
stream: `block_offset` increments by the fixed 5120-byte body length while the
other two header words remain stable in the representative flow. Rust exposes
this as `assemble_bulk_envelopes()`, which rejects gaps and metadata changes.
The capture does not contain the full compressed object, so the complete file
and all envelope-word meanings remain open. This stream is not a source of
311-byte `2704` baseline records.

The dominant `2704` frames were subsequently tied to Wine's decoder at
`0x44aa30`. The six-byte prefix is `u16 record_count + u32 value_end_offset`,
not three independent `u16` words. The offset is payload-relative: the value
bitstream is `payload[6..value_end_offset]` and the index bitstream begins at
`payload[value_end_offset]`. The old candidate-length check failed because it
compared a payload-relative end offset with the post-prefix body length.

A scan of 141 extracted `2704` bodies found no stable market marker or public
six-digit code text. The result rules out only the simplest plain-text layout;
compression, delta coding and encryption remain separate hypotheses requiring
same-symbol callback evidence.

The vendor SDK source archive was checked as an additional source. Its C#
sample defines `OEM_DATA_HEAD` and the 500-byte `OEM_REPORT` callback object,
but has no 5188 or `2704` network decoder. Therefore the callback layout is a
target contract only, not evidence for the wire layout.

Flow summaries now expose integer capture-time bounds (`first_payload_at_micros`
and `last_payload_at_micros`). These are intended to select candidate frames
for a synchronized Wine callback window and are not substituted for the
`OEM_REPORT` business time field.

The pcap summary and complete-frame exporter now report the named `2704`
header and restore the first-pass `market/symbol_index/timestamp/uses_baseline`
sequence with Wine's MSB-first token tables. This does not yet decode the value
bitstream or map the internal symbol index to a public code.

1. Select one long-lived connection with a nearby Wine `OEM_REPORT` callback.
2. Reassemble complete 5188 frames, then test each recurring kind for an
   evidence-backed compression/object envelope (without guessing a codec from
   magic bytes alone).
3. Match one symbol and timestamp across the raw inner object and Wine callback.
4. Record the account class (`formal` or `test-168`), endpoint class, connection
   count, fixture hash and match result in the authority ledger.

Disposition: **confirmed for frame boundaries and cadence; needs-verification
for inner object structure, field mapping and callback ownership**.
