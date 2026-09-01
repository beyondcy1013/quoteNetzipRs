# Official 5188 production wiring

Status: exploration; not complete.

## Crate consolidation checkpoint (2026-09-02)

- `/home/codes/stock/crates/netzip-fullpull` is the single authoritative
  protocol crate for the Rust service.
- The duplicate reference copy formerly under this repository was archived at
  `/home/codes/stock/archive/netzip-fullpull-20260902/netzip-fullpull` and is
  excluded from builds by its absence from the workspace.
- ACK protocol constants now live in `netzip_fullpull::auth_manifest`, and the
  ACK client length gate accepts the observed variable range 40..=80 bytes.
- Deployment request `021454-18d1330bb455c524` installed the resulting release
  at `/home/bin/netzip/quoteNetzipRs`; audit status was successful with one
  modified binary and no missing paths.

## Current boundary

The production publish worker in `src/bin/netzip_service.rs` owns
`Tdx7709Session` instances configured by `NETZIP_TRANSITION_7709_ENDPOINTS`.
That worker is the 7709 supplement/transition path. The 5188 types in
`crates/netzip-fullpull/src/official_5188.rs` and the 7100 lifecycle in
`src/auth_7100_client.rs` are currently research and transport foundations.

## Required implementation slices

1. Add an authenticated session owner that retains the 7100 control socket and
   creates one current-session `Official5188Session` per discovered endpoint.
2. Generate each connection's 3610/3110/3210/2d10 initialization from the
   current session. Captured frames remain shape fixtures only.
3. Reassemble server frames with gap/duplicate/length checks and expose raw
   frame evidence without treating 2704/3e04 bytes as decoded quotes.
4. Add a verified business decoder only after a same-symbol, same-timestamp
   match against a Wine `wjf.oem_report.v5` callback. Until then, publish no
   guessed `CompactQuote` values.
5. Introduce a separate production lane and metrics for official 5188; keep
   7709/0547 exclusively in the supplement lane. A 7709 failure must not stop
   the 5188 lane.
6. Update capabilities and deployment units only after authenticated,
   non-7709 live delivery, reconnect, representative coverage, and callback
   parity tests pass.

## Evidence gate

The current primary Wine captures prove `login_primary=true` but contain only
`3901` 5188 control/keepalive frames. They do not yet prove business-frame
delivery or field mapping. This is a required next experiment, not a reason to
promote the current 7709 worker under a new name.

The full workspace test baseline passed through webClx request
`201639-18d13133fc66c8b7` (`2053_build.log`, status 0). This validates the
transport and structural test suite only; it does not clear the production
5188 wiring or business-decoder gates above.

The supplement endpoint-boundary fix was deployed through webClx request
`201952-18d13133fc66c8b9`; the install report (`2055_install-report.json`)
records a successful release install with both Rust services active.

The post-deployment 30-second capture (`post-deploy-2020`) again found ten
5188 flows, three `3901` frames, and zero server business frames. This rules
out the latest Rust deployment as the cause of the missing business payload;
the Wine-side callback path remains separate from the observed 5188 traffic.

Post-deployment health check confirms the Rust service and both systemd lanes
are active. The Wine host also currently reports `login_primary=true`. These
are runtime availability checks only; the production 5188 business-frame and
decoder gates remain open.

## Latest runtime checkpoint (2026-09-01 20:20 Asia/Shanghai)

The 2055 deployment was rechecked after installation:

- `quote-netzip-rs-full-push.service`: active
- `quote-netzip-rs-supplement.service`: active
- `GET /health`: `{"ok":true,"service":"quoteNetzipRs","version":"0.1.0"}`
- `GET /api/capabilities`: still reports the public contract as the legacy
  7709 transition publisher and official full-push as pending
- webClx build log: `/home/bin/webclx/logs/quoteNetzipRs/2055_build.log`
- install audit: `/home/bin/webclx/logs/quoteNetzipRs/2055_install-report.json`

This confirms service availability and the supplement endpoint-boundary fix; it
does not satisfy the official 5188 production gate. The next implementation
checkpoint remains an authenticated 7100 session owner feeding a separate
`Official5188Session` lane, with no 7709 fallback and no guessed quote decoder.

## Runtime status endpoint deployment (2026-09-01)

The release containing `Official5188ConnectionPlan` and the read-only status
route was deployed through webClx request `203125-18d13133fc66c8c0` (release
and install status 0; audit `2060_install-report.json`). Post-deploy checks
confirm both Rust services are active and the new route is present in
`/api/capabilities`. Before login it reports `authenticated=false`, no selected
endpoint, `lane=pending-production-wiring`, and
`business_decoder=opaque-evidence-only`, which is the expected honest baseline.

The authenticated control session now exposes
`connect_selected_official_5188(timeout)`. It requires
`login_success_confirmed`, takes only `selected_quote_endpoint` from the same
7100 login result, and delegates to the 5188 transport's authenticated endpoint
and supplement-port rejection checks. Compile verification is queued as
webClx request `203314-18d13133fc66c8c1`.

The explicit service route `POST /api/fullpull/official-5188/connect` is now
deployed with request `203842-18d13133fc66c8c6` (audit
`2065_install-report.json`). A post-deploy call without a retained 7100
session returns `7100 control session is not retained`, confirming the route
does not create unauthenticated or 7709-backed connections.

## Latest deployment checkpoint (2026-09-01 21:07 Asia/Shanghai)

The unified 5188 receive-loop API deployment was retried after the Cargo
package-cache lock cleared and completed through webClx request
`210255-18d1330bb455c4a6` (`2072_build.log`). Both Rust services remain active
and `/health` reports `quoteNetzipRs` version `0.1.0`. The installed binary at
`/home/bin/netzip/quoteNetzipRs` matches `target/release/quoteNetzipRs` by SHA-256
(`cfc41489c895d51bdc8219734ca8b3f0680e765bfe8684e6cbcf9777c4c80860`).

The install audit was submitted with the obsolete path `/home/bin/quoteNetzipRs`
and therefore reports `missing=1`; the deployment script's authoritative
install path is `/home/bin/netzip/quoteNetzipRs`. This is an audit-path defect,
not evidence of a failed install, and must be corrected on the next deployment.
The runtime status still reports `lane=pending-production-wiring` and
`business_decoder=opaque-evidence-only`; no official 5188 business delivery
has been promoted.

The latest available Wine primary capture (`diagnostics/20260901-live-pair/
primary-confirmed-2000/primary-5188-summary.json`) contains only server
`3901` control/keepalive frames on its 5188 flows. It reports zero `2704`,
`3e04`, delta, bulk, and subscription frames. This is current negative
evidence for business-payload availability, not a Rust parser failure; the
production decoder gate remains closed until a capture contains actual
business frames.

The live Wine status endpoint was rechecked after the Rust release baseline:
`login_primary=true`, effective primary routing remains `智能选择`, and the
gateway forwarder reports `received_batches=12` and `received_quotes=18564`
with five successful posts. This confirms that the Wine callback/forwarder
side is producing data, but it is not evidence that Rust has received the same
symbols over an authenticated 5188 business stream. The values therefore stay
as provenance evidence only; callback parity remains open until a paired
5188 capture overlaps the callback window.

A continuation capture ran for 20 seconds at `2026-09-01T21:46:14+08:00`
under `diagnostics/20260901-live-pair/continuation-2145/`. The Wine status
snapshot retained `login_primary=true` and the same forwarder counters, but
the event endpoint timed out on every poll and the resulting JSONL is empty.
This is negative/insufficient evidence only: it adds no business-frame sample
and does not advance decoder or callback-parity acceptance.

After restarting `quoteNetzipWine-wine-supervisor`, `quote-netzip-rs-supplement`,
and `quote-netzip-rs-full-push`, all three services returned `active`. The
20-second paired capture under `diagnostics/20260901-live-pair/restart-2155/`
again showed the same shape: one 6100 response with 326 wire payload bytes,
multiple 5188 flows totaling only 90 server payload bytes of 3901 control
frames, and local 16801 callback traffic. This restart check confirms both
applications reconnect, but packet contents are not equivalent and no
business-frame parity is established.

The follow-up capture at `continuation-2147/` did collect 17 `股票数据`
callback events while the Wine status remained primary-authenticated. Rust's
`POST /api/debug/pcap-5188-summary` on the paired pcap found only `3901`
control frames (`payload_len=2`, opaque), with zero `2704`, `3e04`, delta,
bulk, or subscription envelopes. This separates callback production from
the still-missing observable 5188 business stream; the decoder gate remains
closed and the capture is retained as a negative correlation fixture.

The all-port summary of that same pcap adds a material contradiction to the
current 5188-only hypothesis: `121.41.70.217:6100` sent a 326-byte complete
`网络包` whose parsed tail is `压缩|ZSTD`, while the observed 5188 flows still
contain only `3901` control frames. Local `127.0.0.1:16801` then carries the
callback-shaped packets. This is not yet proof that 6100 is the official
full-push business protocol, but it is stronger business-path evidence than
the 5188 sample and must be analyzed before further 5188 decoder assumptions
are promoted. Disposition: `needs-verification`; follow-up: bounded decode of
the 6100 ZSTD packet and same-window callback correlation, with 7709 excluded.

After the next cold restart, the packet-by-packet alignment capture
`diagnostics/20260901-live-pair/align-2200/` found ten 5188 server streams,
each with two unique `3901` frames (20 bytes total per stream) and two
retransmitted copies. No 6100 payload appeared in this 20-second window;
16801 carried only local system callback packets. Wine status still reported
`login_primary=true`, but its forwarder counters were unchanged. This proves
the restart and stream partitioning work, while also showing that service
health and callback counters do not imply a matching 5188 business stream.

The boot-aligned capture `diagnostics/20260901-live-pair/align-boot-2210/`
started tcpdump before restarting all three services. Its first complete
external response was the same 6100 `field44=12`, `压缩|ZSTD` 326-byte packet;
the client sent a matching 582-byte request. The ten 5188 flows then produced
only 20-30 bytes each of repeated `3901` frames. Local 16801 exchanged the
callback-shaped packets, and 2,542 event records were collected. This is the
strongest packet-by-packet restart evidence so far, but it still does not
prove that the 6100 packet is market data rather than authenticated download
traffic.

The subsequent full workspace regression completed successfully through webClx
request `211134-18d1330bb455c4a9` (`2075_build.log`, status 0). This covers the
existing 5188 transport/reassembly tests, 7100 authentication/session tests,
pcap parsing, and 7709 supplement boundary tests. It is a structural baseline
only and does not clear the live 5188 business-frame, reconnect, coverage, or
Wine callback-parity gates above.

## Second formal fixture replay (2026-09-02)

The independent fixture `diagnostics/20260902-live-pair/formal-primary-0002/`
was replayed with `official_5188_extract` and `/tmp/wjf-frida-20260902.core`
through webClx request `062320-18d1330bb455c572`. The run completed with
`status=0` and exported 160 complete frames, but every frame was `0x0139`
control traffic: zero `0104` code tables, zero `2704` value frames, zero
decoded records, and zero loaded core baselines. Its callback JSONL is empty.
This is retained as formal negative evidence and does not count as a second
business or callback-parity fixture.

The same capture was re-read after adding Linux cooked v2 (DLT 276) support to
`pcap_summary` through webClx request `062719-18d1330bb455c574`. The parser now
sees 703 pcap packets and 338 unique packets. The ten 5188 server flows still
contain only 10-byte `3901` control frames; the only compressed business-shaped
traffic is two complete ZSTD packets on `121.41.70.217:6100` (328 bytes each).
This confirms the earlier zero-packet result was a link-layer parser gap and
keeps the 6100 path as a separate `needs-verification` lead rather than
promoting it to the 5188 decoder.

## Historical Wine evidence update (2026-09-02)

The paired capture
`diagnostics/20260902-live-pair/formal-primary-0006/` supersedes the earlier
negative 20-second captures for business-frame availability. Its generated
`primary-5188-summary.json` contains 38 flows and 680 reconstructed frames:

- 68 client initialization frames (`3610`, `2d10`, `2a10`, `0710`);
- 581 server frames, including `3e04`, `1504`, `0104`, `0d04`, `1b04`,
  `2104`, `2804`, `2e04`, `3f04`, `4104`, `5104`, `5404`, and `2704`;
- 19 `3e04` bulk envelopes and 15 `2704` delta envelopes;
- repeated zlib object families (`0104`, `1504`, `1b04`, `2804`, `3f04`,
  `4104`) with declared lengths and decoded object samples.

The paired Wine callback log contains 17 `股票数据` events, 11 non-empty
batches, and 14,610 quote records. `callback-window-index.json` records 102
same-window candidates, including `2704` and `3e04` frames that occur within
milliseconds of a 294-record callback batch. These are now valid decoder
fixtures and prove that the 5188 business stream is observable in a historical
Wine session.

The evidence still does **not** prove field parity: candidates remain
`candidate-frame-window-only`, and no network record has yet been matched to a
specific `(market, code, timestamp)` and all quote fields from
`wjf.oem_report.v5`. The implementation must therefore keep the business
decoder gate closed while using this capture to reconstruct the `2704` delta,
`3e04`/`1504` bulk objects, and their relation to callbacks. No new Windows
capture is required for this offline phase; a second independent paired Wine
session remains the parity acceptance test.
