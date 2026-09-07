# Packet Alignment Runs 2026-09-01

Exploration-stage evidence. These runs are separate lifecycle captures and
must not be merged into one protocol model.

## Restart capture: `align-boot-2210/boot.pcap`

Normalized with `scripts/align-pcap-packets.sh` after retransmission removal:

- 6100: client-to-server 582B and server-to-client 326B application packets;
  both expose `field44=12` and a ZSTD hint.
- 5188: ten server-to-client streams, each containing only wire `39 01`
  (`0x0139`) control frames; no business payload was identified.
- 16801: local callback traffic with structured application packets.

Disposition: 5188 control transport `confirmed`; 6100 role
`needs-verification`; 16801 local callback `confirmed`.

## Full-session fixture: `captures/2026-09-01_full-session/official_full_session.pcapng`

The same normalizer reports 6100 and 7100 ZSTD login/download-shaped flows,
7719/7712 traffic, and local proxy flows. No 5188 flow is present in this
fixture. This is a capture-scope difference, not evidence that 5188 is
optional or that 7719 is the official full-push replacement.

## Alignment rule

Only packets from the same cold-start lifecycle, account class, endpoint
selection, and callback window may be compared one-by-one. A port appearing in
one capture and absent in another remains `needs-verification` until a fresh
capture covers the complete initialization sequence.

## 7100 matrix cross-check

The deployed `/api/debug/auth-7100-flow-matrix` successfully parsed
`diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng`:
the session was classified as `auth_login`, with a `field44=12` dictionary
envelope and an inflated root object named `认证`. Sensitive field values were
represented only by structural metadata and fingerprints were omitted.

Running the same endpoint against `captures/2026-09-01_full-session/
official_full_session.pcapng` produced an empty/unknown 7100 session because
that fixture uses a different address-mapping/capture scope and lacks a
complete handshake. This is an insufficient fixture, not evidence that the
7100 protocol or dictionary path is absent.

After deployment `223046-18d1330bb455c4c9`, the live Rust service accepted
`service_port=6100` and reconstructed the vendor 6100 session
(`started_with_handshake=true`, 39B client payload and 1779B server payload).
The classifier returned `unknown` with no recognized 7100 envelopes, so this
stream remains transport evidence only and is not promoted to market data.

The same generic probe against `official_full_session.pcapng` with
`service_port=7719` found a long-lived stream (`78,823B` client payload,
`694,998B` server payload) but no recognized 6100/7100 envelope objects, so
the role was `unknown`. Its volume makes 7719 a high-priority candidate for
follow-up legacy framing analysis, while the current evidence is insufficient
to call it the official full-push market stream.

Raw inspection of the first 7719 records with `tcpdump` shows malformed or
offset link decoding (unknown EtherTypes and non-IPv4-looking headers). This
is consistent with pktmon metadata or a capture-link offset mismatch. The
Rust normalizer's pktmon/bare-IPv4 handling should be used for extraction;
traditional tcpdump output must not be treated as application framing.

The Rust `pcap-summary` endpoint still exposes bounded payload prefixes for
7719. Server-to-client begins with `b1 cb 74 00 ...`; client-to-server begins
with `0c 7a 0a 02/03 ...`. These stable prefixes are useful framing anchors,
but the existing legacy scanner requires a contiguous extracted stream before
it can validate lengths and compression. No decoder promotion is made from
prefixes alone.

Field-boundary inspection of those prefixes is consistent with the existing
legacy scanner's frame shapes: the first server payload is 37B with a 16-byte
header declaring a 21B body, while the first client payload is 26B with the
10-byte header declaring 16B at both length slots. This confirms a reusable
transport framing candidate for 7719, but does not establish endpoint
provenance or quote-field parity.
