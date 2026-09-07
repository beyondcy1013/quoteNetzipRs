# Official Full-Push Replication Authority

Status: **exploration / evidence-building** (updated 2026-09-02)

## 2026-09-02 initial-login correction

- `confirmed` from the formal `vendor_pm` flow: the full-push route request is
  for `系统\\大智慧服务器L1.ini` (267B client request), whose 1880B response
  contains the active 5188 server rows. The existing `系统\\通达信股票服务器.ini`
  request is the separate 7709/supplement configuration and must not be used
  to select the official full-push endpoint. Rust now exposes a parameterized
  download-file builder and uses the L1 filename in the authenticated flow;
  the legacy filename remains only as an explicit compatibility default.

- `confirmed` by the contributed online protocol and driver acceptance: the
  vendor initial account login is the 19-field `认证/请求登录` client manifest,
  encoded with ordinary level-3 ZSTD. Its 854-byte decoded object and
  462-byte ZSTD frame were rebuilt byte-for-byte against the vendor capture;
  the live server returned a 436-byte, 15-field `认证` response containing
  `提示信息=登录成功` and the expected account/permission metadata.
- `rejected`: using the 12-field dictionary `加密包` as the initial account
  login. That shape remains valid for each post-authentication 5188 connection
  (`大智慧C_登录包`) but is a different protocol stage. The earlier
  2026-09-01 sections below are retained as experiment history and are
  superseded on this point.
- `confirmed`: initial login compression is ordinary ZSTD. A dictionary decoder
  may appear to succeed on the frame while producing polluted output, so the
  decoder must validate the expected root before accepting either path.
- `needs-verification in this repository`: the vendor repeats the manifest
  structure as an approximately 100-second control heartbeat. Its dynamic
  fields, response role and relationship to hidden 5188 endpoint distribution
  must be imported as a sanitized fixture and then verified on the retained
  authenticated socket before runtime scheduling is enabled.
- `open`: successful initial authentication still returns only 7709 routes in
  the currently observed download table while Wine holds ten 5188 sessions.
  Endpoint distribution therefore remains a separate missing control-channel
  stage; authentication success alone does not establish official full-push.

## 2026-09-01 late-session corrections

- Superseded by the 2026-09-02 correction above: production authentication no longer sends the ordinary-ZSTD
  `认证|登录` object. `connect_auth_sequence` now sends the credentials-bearing
  12-field dictionary `加密包` (`请求=大智慧C_登录包`) and accepts success only
  after the exact four-field response, echoed request number and embedded
  current-session `3610` are validated. The final `Tdx_Encrypt` stage requires
  the `加密解密` root. Dictionary decode is accepted only when its root matches;
  otherwise plain ZSTD is attempted. Focused regression request
  `233202-18d1330bb455c4e9` passed 22 tests and deployment
  `233258-18d1330bb455c4ea` succeeded.
- A synchronized formal-account cold start under
  `diagnostics/20260901-live-pair/cold-sync-2311/` captured 34 active 5188
  directions, client `3610/2d10/2a10`, server `3110/3210/3e04/1504`, and 22
  realtime Wine callback batches in one lifecycle. The post-restart status
  reported `login_primary=false`, so callback provenance remains
  `needs-verification`; the bytes are nevertheless valid transport-boundary
  evidence.
- That capture proved a previously missing frame invariant. Large zlib object
  frames (`1504` and `0104`) store only `(8 + compressed_len) mod 65536` in the
  outer 16-bit length, while payload words 0/1 are uncompressed/compressed
  lengths and zlib begins at payload offset 8. The reassembler now promotes
  the extended length only when the zlib marker and modulo invariant both
  agree. This removed the first class of random pseudo-kinds created at the
  64-KiB boundary; further object kinds and callback field parity remain open.
- Runtime replay after the fix changed the historical `vendor_pm` 3104 flow
  from 5,195/4,922 total/delta frames to 7,099/6,766, with zero trailing bytes.
  This proves the old counts were artifacts of u16 truncation. Bounded object
  decoding now closes declared compressed and uncompressed lengths for kinds
  `0130/0104/1504/1b04/2804/3f04`. Sample heads identify `1504` as transferred
  configuration/files (`split.pwr`, `pmd.txt`, `net.xml`, market ini), while
  `0104/1b04/2804` carry market/code-table-shaped binary objects. These are
  startup objects, not yet realtime `OEM_REPORT` field mappings.
- The corrected `2704` samples retain a six-byte `(record_count, opaque_word,
  zero)` prefix followed by a dense `0x81`-tagged variable body. Across samples
  the first word ranges with apparent record count, but the second word does
  not close either payload or body length. No field decoder is promoted until
  the custom body codec is matched against a same-window Wine quote record.

## Account and experiment policy

Use the formal account by default, especially for cross-source comparisons,
data-value parity, coverage, latency and replacement acceptance. The test
account `168` is reserved for repeated, short-interval structural diagnostics
of one narrowly defined issue where minimizing account churn is useful. It is
not an equivalent data source: test-168 has been observed on 7709-only routing,
and its subscriptions, market values, coverage, payload volume or callback data
may be absent, partial, stale, or inconsistent with formal-account entitlements.
Account type must be recorded for every capture or runtime probe without storing
credentials. Do not compare connection counts, selected servers, symbol coverage
or payload volume across account types as if they were protocol constants. Do
not use test-168 values as Wine parity, production coverage, decoder truth, or
acceptance evidence. Any surprising or account-sensitive result must be rerun
with the formal account before becoming a shared implementation rule.

### Context-compaction (`/compact`) risk

`/compact` is lossy context management, not evidence persistence. It can drop
exact fixture paths, hashes, byte offsets, command order, account/window
context, failed hypotheses and unresolved caveats. Before invoking it, persist
observations, evidence paths, commands, conclusions and the next discriminating
test in this ledger, the owning forensic document, or the project skill. After
compaction, re-read those authoritative sources and raw fixtures before
continuing. Never cite a compacted summary as protocol evidence or use it to
promote a decoder rule across the acceptance gate.

This is the single canonical ledger for the Rust reproduction of
`quoteNetzipWine`'s official full-push market path. It records current technical
conclusions, evidence links, unresolved questions and acceptance gates. It does
not replace raw captures or immutable forensic notes.

## Authority order

1. Raw fixtures and capture indexes under `docs/forensics/`, `diagnostics/` and
   `windows_debug/` (facts that can be replayed).
2. This ledger (current conclusions and implementation state).
3. `docs/capability-boundaries.md` (product semantics and ownership boundary).
4. Focused forensic reports and audit history, including
   `docs/gpt5.6_fullpull_supplement_boundary_audit.md`.
5. README, scripts, service names and UI labels (informational only).

When sources disagree, keep both claims in the contradiction ledger and run a
discriminating fixture/test before changing shared behavior.

## Canonical boundary

Official full-push is: formal account authentication, vendor-selected
non-7709 data connections (currently observed on 5188), Wine-equivalent
initialization and decoding, and continuous server-driven callbacks.
7709/0547 queries, renewals, worklists and unsolicited deliveries are
supplementation, even when they look push-like. Authentication success alone is
not data-session success.

## Confirmed successful analysis

- Runtime gate check after release request `222139-18d1330bb455c4c4` returned
  `/api/fullpull/official-5188/status` with `authenticated=false`, no selected
  endpoint, `lane=pending-production-wiring`, and
  `business_decoder=opaque-evidence-only`. This is the authoritative current
  deployment state: transport scaffolding is deployed, but the Rust process
  is not yet an authenticated production full-push consumer.

- A fresh post-restart, start-of-capture alignment of
  `diagnostics/20260901-live-pair/align-boot-2210/boot.pcap` was rerun with
  `scripts/align-pcap-packets.sh` on 2026-09-01. The normalized evidence shows
  two unique 6100 application packets in each direction (582B client request,
  326B server response; both `field44=12`, ZSTD hint), ten 5188 server-only
  streams containing only `0x0139`/wire `39 01` control frames after
  retransmission removal, and local 16801 callback traffic. This confirms the
  alignment tooling and transport observations; it does **not** promote 6100
  to an行情 decoder or 5188 control frames to business data. The 6100 role is
  still `needs-verification` (authentication/download versus market payload).

- The deployed port-parameterized matrix was then run against the same fixture
  with `service_port=6100`. It classified both packets as `auth_login`
  dictionary envelopes (`field44=12`) with an inflated `认证` root. This direct
  evidence rules out treating that observed 6100 pair as a market quote frame;
  post-login business-stream discovery remains open.
- Workspace regression after adding the optional service-port matrix parameter
  passed in webClx request `223816-18d1330bb455c4d0` (`2109_build.log`): all
  reported test groups were green (85+5+1+64+4+6+17 tests, with only the
  existing ignored fixtures). This verifies compatibility of the new 6100
  analysis path without changing production lane semantics.
- Post-deployment replay of `vendor_pm.pcapng` through
  `/api/debug/pcap-5188-summary` identified 34 flows with application frames
  and approximately 30,019 reconstructed frames. Core server kinds were
  `0x0427` (1,409), `0x0421` (104), `0x0454` (95), `0x040d` (94), and `0x043e`
  (1). This is fresh transport/reassembly evidence for the official 5188
  sample; payload business fields remain intentionally undecoded.
- A frame-sample inspection of the same long-lived 5188 flow provides a
  concrete inner-codec lead: a `3e04` payload is 5,132B with a zlib magic
  (`78 9c`) at payload offset 24, while a `1504` payload is 29,653B and starts
  with zlib magic. These are bounded zlib candidates, not decoded quote
  records; decompression output and Wine field parity remain unverified.
- The bounded embedded-zlib scan for the `3104` flow reports 16 candidate
  offsets with decoded lengths such as 74, 187, 312, 1,248, 2,154, 2,712,
  6,108, 6,147, 6,118, 5,944, 7,388 and 50,554 bytes, while whole-payload
  zlib decoding remains zero-success. This supports an inner-stream model and
  gives concrete boundaries for future object reconstruction; it is not yet a
  field decoder.

- Deployment `224543-18d1330bb455c4d6` was smoke-tested at runtime: the
  generic auth-flow route returns HTTP 400 for `service_port=7709` while
  `/health` remains OK. This confirms the supplement-only port cannot be
  accidentally routed through official authentication analysis.

- Static call-shape evidence confirms Wine's ACK path is bounded text/object
  formatting (`0x100742f0` -> `0x100075f0`, conversion bound `0x3a8`). Capture
  and `0104` correlation further confirm each six-byte `2a10` entry as
  `market[2] + symbol_index(u32 LE)`. ACK variant generation and runtime
  subscription set/slot selection remain `needs-verification`.

- Wine ABI and callback contract are documented and encoded in `tuwenca-codec`.
- 6100/7100 formal login response roles and endpoint extraction are validated;
  credentials remain runtime-only.
- 5188 frame boundary is implemented in
  `/home/codes/stock/crates/netzip-fullpull/src/official_5188.rs`: 8-byte header,
  metadata preservation, TCP reassembly, direction classification, handshake
  shape checks and payload bounds.
- Wine 5188 client sequence is reproducibly `3 x 0x3610`, `3 x 0x2d10`, optional
  `0x0710`, then `0x2a10`; server kinds include `0x2704`, `0x0d04`, `0x5404`,
  `0x2104` and `0x3e04`.
- The 2026-09-01 boundary note confirms that some captured sockets have only
  server-to-client packets because capture began after initialization or the
  socket had another session role; this is not evidence that Rust may skip
  authentication or the client initialization sequence.
- The metadata-preservation regression and release build passed through
  webClx request `120759-18d115af25cb03a5` (log:
  `/home/bin/webclx/compile/runs/20260901T120804-2928724-32395/build-1.log`).
- The follow-up release verification for the source audit and transport test
  passed through webClx request `121914-18d115af25cb03ae` (log:
  `/home/bin/webclx/compile/runs/20260901T121919-2991465-17244/build-1.log`).
- Wine initialization outer objects, realtime.dat layout, OEM report mapping,
  code-table, split, finance and file encoders have focused validation notes.
- Existing Rust 7709/0547, K-line, F10 and FIN parsers are useful supplement
  components, but are not full-push evidence.
- A source audit of `../quoteNetzipWine/src` found no 5188 business-payload
  decoder; Wine exposes the DLL control/callback surface while the packet
  semantics remain in the vendor component. The Rust `quote_frame_scan` helper
  targets legacy 7709/7719 layouts and is not a valid 5188 decoder.
- Binary symbol audit of the vendor `Stock.dll`/`Stock64.dll` confirms only
  `Start`/`Ask`/`Stop` are exported; visible `ZSTD` strings do not establish a
  5188 codec. See `docs/forensics/wine-binary-symbol-audit-20260901.md`.
- The pcap summary path now normalizes pcapng through the shared capture
  converter before classic-pcap parsing, removing a tooling gap that blocked
  Rust-side analysis of the real 5188 captures.
- Warning cleanup for that path was queued as webClx request
  `122620-18d115af25cb03b0`; the worker log is under
  `/home/bin/webclx/compile/runs/20260901T122625-*/build-1.log`.
- Added an evidence-only `payload_hint` classifier in
  `netzip-fullpull::official_5188`. It recognizes zstd/zlib magic or leaves the
  payload opaque; it performs no decompression and makes no quote-field claim.
- Added `POST /api/debug/pcap-5188-summary`, which exposes the Rust-side
  pcap/pcapng 5188 flow reassembly statistics for reproducible evidence work.
- The 5188 pcap scanner now deduplicates packet identities (sequence, ACK,
  flags and captured payload) before stream reassembly, matching pktmon capture
  handling and preventing retransmission copies from inflating frame counts.
- The packet parser accepts both Ethernet+IPv4 and pktmon captures carrying a
  bare IPv4 frame despite an Ethernet link-type declaration; this prevents
  silent loss of real 5188 packets during pcapng conversion.
- Payload bounds now use the detected link offset as well, preventing a bare
  IPv4 packet from reading 14 bytes beyond its declared IP length.
- Malformed TCP records with a data offset below 20 bytes are now discarded
  before port/payload extraction.
- Malformed-TCP filtering passed release verification in webClx request
  `124519-18d115af25cb03bd` (log:
  `/home/bin/webclx/compile/runs/20260901T124525-3088679-26135/build-1.log`).
- Bare-IPv4 payload-boundary fix and fixture passed release verification in
  webClx request `124256-18d115af25cb03bc` (log:
  `/home/bin/webclx/compile/runs/20260901T124302-3081599-14534/build-1.log`).
- Workspace test run `124654-18d115af25cb03be` completed successfully; the
  resulting log is `/home/bin/webclx/compile/runs/20260901T124659-3093496-19521/build-1.log`.
- Malformed TCP header filtering workspace regression `125016-18d115af25cb03c1`
  completed successfully (log:
  `/home/bin/webclx/compile/runs/20260901T125021-3111320-7259/build-1.log`).
- Runtime probe at `127.0.0.1:16893` still serves the prior deployment;
  `POST /api/debug/pcap-5188-summary` returned HTTP 404. The new diagnostic
  route is intentionally not deployed while the official full-push decoder is
  incomplete.
- A focused regression now constructs two identical 5188 TCP segments and
  asserts one unique segment remains before frame reassembly.
- The scanner now stops a direction at the first TCP sequence gap and reports
  `reassembly_error`, preventing concatenation across missing bytes from
  creating fabricated application frames.
- 5188 flow results now also count evidence-only payload magic hints
  (`zstd-magic`, `zlib-magic`, `opaque`) per direction; these counters do not
  imply successful decompression or field decoding.
- Each flow now retains up to 16 bounded frame samples (kind, metadata,
  payload length and first 32 bytes) for reproducible inner-object analysis;
  full payloads remain unexposed by this summary endpoint.
- The scanner now attempts standard zstd/zlib decompression only for payloads
  carrying the corresponding magic and reports successful decoded lengths;
  failures and opaque payloads remain unchanged. This is candidate analysis,
  not a protocol decoder.
- Decompression candidate output is bounded: total successes are counted, while
  only the first 16 decoded lengths are retained per codec.
- Bounded zstd/zlib candidate statistics passed release verification in webClx
  request `125524-18d115af25cb03c4` (log:
  `/home/bin/webclx/compile/runs/20260901T125529-3130494-18882/build-1.log`).
- Candidate decompression is capped at 8 MiB per payload; oversized expansion is
  recorded as a failed hint rather than materialized without bound.
- `Official5188Session::send_wine_initialization` now gates outbound fixture
  frames through the observed Wine client-shape check before writing them. It
  also requires each of the three `2d10` frames to decode as four raw u32 words
  plus a sixteen-byte zero tail; validation completes before the first network
  write. Payload values remain caller-supplied and authenticated.
- Initialization-send API release verification passed in webClx request
  `130106-18d115af25cb03c8` (log:
  `/home/bin/webclx/compile/runs/20260901T130111-3153150-24907/build-1.log`).
- `confirmed`: the `2d10` write-boundary regression and all 68 shared-crate
  tests passed in webClx request `165430-18d115af25cb03fc` (log:
  `/home/bin/webclx/logs/quoteNetzipRs/220_build.log`). A malformed reserved
  tail is rejected before the fixture peer receives any bytes.
- Authenticated-connect gating and 7709 rejection passed release verification
  in webClx request `130330-18d115af25cb03c9` (log:
  `/home/bin/webclx/compile/runs/20260901T130335-3161101-23556/build-1.log`).
- The 8 MiB expansion-bound regression passed workspace tests in webClx request
  `125936-18d115af25cb03c7` (log:
  `/home/bin/webclx/compile/runs/20260901T125941-3144105-12971/build-1.log`).
- The 5188 replay endpoint is now listed in both Linux/native and research
  capability views, keeping discovery metadata consistent.
- Flow summaries now expose `trailing_bytes` for an incomplete final frame, so
  truncated captures cannot be mistaken for complete application streams.
- Bounded frame-sample extension passed release verification in webClx request
  `124845-18d115af25cb03c0` (log:
  `/home/bin/webclx/compile/runs/20260901T124850-3106530-11445/build-1.log`).
- The new route is listed in the Linux/native capability inventory so tooling
  can discover it without treating it as a public market-data endpoint.
- Capability-list registration passed release verification in webClx request
  `123932-18d115af25cb03ba` (log:
  `/home/bin/webclx/compile/runs/20260901T123937-3070688-15511/build-1.log`).
- Research capability registration for the 5188 replay endpoint passed release
  verification in webClx request `125757-18d115af25cb03c6` (log:
  `/home/bin/webclx/compile/runs/20260901T125802-3139492-8830/build-1.log`).
- Capability-list regression assertions passed in webClx request
  `130520-18d115af25cb03ca` (log:
  `/home/bin/webclx/compile/runs/20260901T130525-3167683-27815/build-1.log`).
- An explicitly ignored real-fixture test now exercises
  `vendor_pm.pcapng`; queued as request `130612-18d115af25cb03cb` for
  on-demand evidence capture. It is not part of ordinary workspace tests.
- Real-fixture replay (`130612-18d115af25cb03cb`) successfully reassembled
  vendor 5188 server streams, including recurring 5132-byte payloads and
  short status payloads. The inner payloads remain opaque; this confirms the
  transport boundary only, not quote decoding.
- Kind display now exposes both the numeric little-endian value (`kind`, e.g.
  `0x040d`) and capture wire order (`wire_kind`, `0d04`). This resolves a
  representation contradiction found when comparing Rust JSON with the raw
  `vendor_pm_5188_frames.txt` prefixes. Protocol constants were not changed.
- Real replay then exposed that the old constants themselves had been written
  in wire spelling, which made direction classification return `Unknown`.
  Constants are now corrected to decoded numeric values (`0x0427`, `0x040d`,
  `0x0454`, `0x0421`, `0x043e`; client `0x1036`/`0x102d`/`0x102a`/`0x1007`).
  Regression tests decode raw `36 10` and `3e 04` headers and assert client/server
  direction. This is a protocol-boundary correction, not inner quote decoding.
- The first real replay also included zero-payload SYN/ACK records in the TCP
  sequence walk, creating a false one-byte initial gap and zero decoded frames.
  The scanner now excludes empty TCP payloads from application reassembly while
  retaining sequence-gap reporting. Replaying `vendor_pm.pcapng` now yields
  long-lived server flows with hundreds to thousands of classified frames and
  nonzero `server_frames`; inner payload decoding remains `needs-verification`.
- Byte-level analysis of the extracted `3e04` samples found a stable 5132-byte
  payload: three little-endian header words followed by a 5120-byte body. The
  Rust `Official5188BulkEnvelope` parser and pcap summary now retain those raw
  words without assigning business meaning. Replaying the long-lived flows
  reports 73 such envelopes on representative connections; this is an inner
  boundary observation, not quote-field decoding.
- Across ten representative server flows, the bounded local extraction found
  five distinct `3e04` payload hashes (each repeated across connections), and
  two distinct hashes each for the 18-byte `0d04` and 20-byte `5404` payloads.
  This cross-connection repetition supports a broadcast/shared-data role, but
  does not identify the contained security or quote fields.
- A byte-search against the available Wine `OEM_REPORT` fixture (for example
  `SH000001` close/open/volume float encodings) found no direct matches inside
  the sampled `3e04` bodies. This is negative evidence against treating the
  body as a plain OEM record array; encryption, compression or another packing
  layer remains `needs-verification`.
- Wine `2a10` subscription payloads have a stable `10 + 6*N` shape. Each entry
  is `market[2] + symbol_index(u32 LE)` in the same index space as the `0104`
  code table. The Rust parser and pcap summary expose counts and consecutive
  ranges without publishing raw entries.
- Two independent authenticated lifecycles (`formal-primary-0006` and
  `cold-sync-2311`) each contain seven `2a10` frames: six declare 1024 entries
  and one declares 58, for 6202 entries total. Their normalized range shapes
  match exactly after ignoring endpoints, local ports and timestamps.
- Five 1024-entry frames tile contiguous SH/SZ symbol-index space; the sixth
  1024-entry frame and the 58-entry frame are discrete mixed sets. Runtime set
  selection, user-list semantics and connection slot assignment remain
  `needs-verification`; no captured list is a runtime template.
- The older `vendor_pm` replay reported a reduced 122-entry socket and
  different starts. Retain that as session-specific historical evidence, not
  as the current universal subscription count or partition rule.
- The explicit ignored replay `debug_pcap::tests::scans_vendor_pm_5188_capture`
  was rerun locally after the consecutive-range change. It passed and emitted
  `subscription_frame_count=1`, `subscription_entry_count=1024`, and a single
  SH range on the representative long-lived flow. Other capture flows still
  contain unknown kinds and incomplete trailing bytes; those remain preserved
  as transport evidence and are not promoted to quote records.
- Release verification for the consecutive subscription-range extraction
  completed in webClx request `134136-18d115af25cb03d4` (log:
  `/home/bin/webclx/compile/runs/20260901T134141-3266197-13817/build-1.log`).
- An evidence-only embedded-zlib scan was added after `78 9c` markers were
  observed inside some `3e04` bodies. It requires a complete bounded zlib
  stream and retains only offset/decoded length. The real fixture replay found
  no promoted zlib decode for the recurring 5188 stream; truncated or invalid
  candidates were rejected. This is negative codec evidence, not proof that
  the inner object is uncompressed.
- `Official5188BulkStream`/`assemble_bulk_envelopes()` now reassemble recurring
  `3e04` bodies by their observed 5120-byte `block_offset` sequence, rejecting
  gaps, overlaps and metadata changes. This captures a confirmed transport
  invariant while leaving the resulting stream opaque for codec analysis.
- A bounded sample review of `0x2704` frames found a recurring six-byte prefix
  (`u16` values such as count/length candidates followed by a zero word) and
  variable bodies with frequent `0x81` markers. The apparent length field does
  not map consistently to total payload length across frames, so no delta
  record layout has been promoted. The next falsifying test is same-symbol,
  same-timestamp matching against a Wine `OEM_REPORT`; until then these bytes
  remain raw evidence.
- The vendor SDK source embedded in `quoteNetzipWine/StockAPI.full.rar`
  (`StockC#/OemStock.cs`) documents the outer `OEM_DATA_HEAD` and fixed 500-byte
  `OEM_REPORT` callback layout, but contains no 5188/`2704` wire decoder or
  inner-object mapping. This confirms that the callback schema cannot be used
  as a network payload schema; the missing comparison requires synchronized
  Wine capture and callback samples.
- A byte-level scan across 141 extracted `2704` samples found no stable
  `SH`/`SZ` marker or six-digit public security-code text (apart from an
  isolated incidental `SH`). Printable-byte ratios were low and variable.
  This is negative evidence against a plain text symbol list, not proof of a
  particular compression or encryption algorithm.
- 5188 pcap flow summaries now retain `first_payload_at_micros` and
  `last_payload_at_micros` as integer capture timestamps. This enables future
  callback-window matching without treating packet time as a quote timestamp;
  the fields are indexing evidence only.
- `Official5188DeltaEnvelope` and pcap fields `delta_envelope_count` /
  `delta_prefixes` now preserve the observed `2704` six-byte prefix without
  naming its words. The focused Rust/workspace regression passed; this is an
  evidence surface for future synchronized callback matching, not a quote
  decoder.
- Release verification for the raw `2704` envelope and callback-window index
  integration completed in webClx request `140726-18d115af25cb03db` (log:
  `/home/bin/webclx/compile/runs/20260901T140731-3352006-27356/build-1.log`).
- The ignored `vendor_pm.pcapng` replay now asserts capture-time bounds and
  searches all flows for a `2704` delta stream instead of assuming the longest
  server flow contains that kind. The corrected replay passed; representative
  flows expose `delta_envelope_count` values of `7732` and `1408`.
- Release verification for the corrected replay assertions and `2704` summary
  integration completed in webClx request `141016-18d115af25cb03dc` (log:
  `/home/bin/webclx/compile/runs/20260901T141022-3362492-5081/build-1.log`).
- Gap handling and overlap correction passed release verification in webClx
  request `123647-18d115af25cb03b8` (log:
  `/home/bin/webclx/compile/runs/20260901T123652-3060750-24817/build-1.log`).
- First compile exposed a thread-boundary error (`Box<dyn Error>` was not
  `Send`) in this debug handler; the handler now converts scanner errors to
  `String` inside `spawn_blocking` before returning them as API errors.
- The classifier and unknown-payload regression passed in webClx request
  `122742-18d115af25cb03b1` (log:
  `/home/bin/webclx/compile/runs/20260901T122747-3020597-30251/build-1.log`).

## Current implementation reality

### 2026-09-04 Wine fixed ten-slot topology

- The formal-account cold-start capture
  `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/wine-primary-sync-20260904T011938.pcap`
  (SHA-256 `f70e2e4111b3b9dd8df7d041b1f3d17b6b2e217f7c767484a862a11f1b06eee5`)
  proves ten long-lived connections to `58.16.134.228:5188` outside market
  hours. Every connection sends `3610 x3` and `2d10 x3` and receives the same
  initialization object families plus periodic `3901` server heartbeats.
- Exactly seven connections additionally send one `2a10`: six contain 1024
  entries and one contains 59. The remaining three send no `2a10`; they are
  initialized, receive code tables/control objects, and stay connected. They
  are not failed or spare TCP handshakes and must not be omitted merely because
  the subscription plan has seven non-empty partitions.
- The first subscribed slot also sends one `0710` with the same 12-byte payload
  in six independently retained flow summaries. It belongs between `2d10 x3`
  and that slot's `2a10`; the other nine slots do not send it.
- Runtime invariant: open ten fixed slots with control numbers login `2..11`,
  ABK `12..21`, ACK `22..31`. Apply subscription partitions only to their
  matching slots; slots without a partition send the validated `2d10 x3` tail
  without inventing an empty or synthetic `2a10`. A failed slot fails the
  complete ten-slot initialization rather than silently degrading to seven.

### 2026-09-01 authenticated control supplies dynamic client initialization

- **Confirmed:** in the formal-account `vendor_pm.pcapng`, all ten dynamic
  95-byte-payload `3610` variants have their changing 64-byte region in a
  preceding server-to-client 7100 payload. Representative 7100 payloads embed
  the complete 103-byte frame, which Wine sends to 5188 about 100 microseconds
  later.
- **Rejected:** generating the dynamic region from a guessed cipher key or
  replaying one captured initialization sequence. The bytes are current-session
  control output and differ by 5188 connection.
- **Needs-verification:** connection-role assignment, runtime `2d10` seed/group
  selection, runtime `2a10` set selection, and `0710`/`2e10`. The `2a10` wire
  encoding and two-session seven-way partition shape are confirmed, but this is
  not a lifecycle binding or ordered initialization rule.
- Evidence and reproducible method:
  `docs/forensics/official-5188-control-frame-correlation-20260901.md` and
  `diagnostics/20260831-netzip-windows-vs-rust/analysis/correlate_5188_control_frames.py`.
- **Corrected boundary:** the 2026-08-06 controlled run observed 7100 probe plus
  6100 login, but other captures contain full 7100 login/control sessions.
  Therefore neither port has one immutable global role. Rust must choose by
  downloaded configuration and verified response sequence, and must retain the
  selected control socket through 5188 initialization.

### 2026-09-01 late-session capture boundary

- **Confirmed:** the later formal-account Wine collection ran with primary
  login disabled and backup login enabled. Its timeline has 7100 plus many 7709
  sockets and no 5188. It proves the switch/protocol boundary and supplies
  supplementation evidence; it does not weaken the earlier primary-enabled
  5188 capture.
- **Needs-verification:** the companion 1 GiB Windows all-port ETL must be
  converted with pktmon-aware tooling before its endpoint coverage is known.
- Evidence: `docs/forensics/wine-full-start-formal-20260901.md`.

`official_5188` currently stops at transport/framing. Inner 5188 object
decoding, callback parity, authenticated session binding and production
quoteGateway publication are **not complete**. The current `NativeSession` and
the service route named full-push still execute 7709 worklist/query logic; treat
that path as legacy supplementation/transition until renamed and isolated.

Wine's `Start(callback)` and `Ask(PCWSTR, ...)` initialization commands are a
separate local DLL control plane. The observed command order is authentication,
backup-login, then initialization; these UTF-16 query strings are not the raw
5188 frame payload and must not be copied into the network transport as if they
were wire bytes. The 5188 client frame sequence is established independently
from packet captures.

## Evidence index

- 5188 frame and cadence analysis:
  `diagnostics/20260831-netzip-windows-vs-rust/analysis/vendor_pm_5188_detail.txt`
  and `vendor_pm_5188_frames.txt`.
- Boundary-focused interpretation of the same capture (including server-only
  socket caveat and the next discriminating experiment):
  `docs/forensics/official-5188-frame-boundary-20260901.md`.
- Cross-source latency/behavior report:
  `docs/glm5.3_netzip_rs_vs_wine_latency_forensics.md`.
- Authentication fixtures and frame indexes:
  `docs/forensics/7100-auth-20260806/`.
- Wine initialization and OEM validation:
  `docs/forensics/wine-initialization-outer-objects-rust-validation-20260901.txt`,
  `wine-realtime-dat-rust-validation-20260901.txt` and `oem-*-rust-validation-20260901.txt`.
- Historical protocol notes and task decisions:
  `docs/netzip-wine-replication-audit.md`, `docs/EXPERIENCE.md` and
  `docs/codex/tasks/netzip-rs-client-progress.md`.

For each new experiment, record at minimum: date/time window, account class
(`formal` or `test-168`), endpoint/port class, connection count, frame/object
summary, fixture hash, expected hypothesis, observed result and confidence.

## Live-market capture inventory

See `docs/forensics/live-market-capture-inventory.md`. Captures are evidence
only; endpoints, tokens and payloads must not become runtime defaults.

## Open questions (`needs-verification`)

- The deployed `embedded_zlib_by_kind` replay summary was rerun against
  `diagnostics/20260831-netzip-windows-vs-rust/captures/vendor_pm.pcapng`
  after request `225941-18d1330bb455c4dd`. Candidate counts were spread across
  dozens of opaque/non-core kind values (including values not present in the
  confirmed `0x0427/0x040d/0x0421/0x0454/0x043e` set), rather than clustering on
  the observed `3e04` or `1504` frames. This is a scanner false-positive/inner
  boundary warning, not evidence of additional protocol kinds. Keep the
  candidates as length/offset evidence only and require a complete object
  boundary plus same-symbol Wine callback match before promoting any mapping.
  Disposition: `needs-verification`; next test: restrict candidate promotion to
  contiguous frame bodies and correlate decompressed hashes with a synchronized
  formal-account Wine callback window.

- The callback-window indexer initially assumed newline-delimited JSON and
  failed on the formal-account capture
  `diagnostics/20260901-live-pair/primary-confirmed-2000/callbacks/` because it
  contains concatenated pretty-printed objects. The parser now consumes
  successive JSON values with `JSONDecoder.raw_decode`. Re-running against the
  deployed 5188 summary produced `candidate_count=0`; the pcap and callback
  timestamps are from different capture windows, so this is a valid negative
  correlation result, not parity evidence. Disposition remains
  `needs-verification`; next test is a synchronized cold-start Wine/Rust window.

- A second check used the paired `primary-confirmed-2000` artifacts with a
  60-second window. The callback events span Unix milliseconds
  `1788263633839..1788263659605`, while the 5188 pcap summary starts around
  `1788264098788986` microseconds (about 439 seconds later), yielding zero
  candidates for a concrete timing reason. The files are therefore not a
  synchronized pair despite their directory label; no callback parity claim
  is promoted.

- Map 5188 inner compressed/object payloads to the same-symbol Wine callback
  fields, including delta ordering and reconnect behavior.
- Prove which post-login endpoint list is dynamic and how session credentials
  bind to each 5188 connection.
- Verify BJ/market-2 coverage in the official chain and in 7709 code-table sync.
- Determine whether online minute/tick delivery exists beyond local
  `实时.dat` parsing; current online executors do not prove it.
- Define callback-to-quoteGateway ownership, cancellation and recovery metrics.

## 2026-09-01 - Current Wine provenance gate (rechecked)

The live Wine host currently reports `gateway_forwarder.received_quotes=18314`
and successful gateway posts, but its effective vendor configuration is
`login_primary=false`, `login_backup=true`, with both quote endpoints set to
`智能选择`. This is useful live callback evidence, but it does **not** prove
that the callbacks came from the official non-7709 full-push chain. The
provenance disposition is therefore `needs-verification` until a cold start
reports `effective.login_primary=true` and the same capture window contains
non-7709 5188 server frames. Do not use these callbacks for field mapping or
claim Rust parity. The status snapshot was obtained from the local Wine host
API on 2026-09-01 and should be rechecked immediately before the next capture.

An exact cold-start experiment stopped the Wine supervisor, terminated only
the quoteNetzipWine/vendor processes, stopped the matching Wine server, and
invoked `--prepare-vendor-config` directly as the Wine user with no matching
process left. The same `Sharing violation (os error 32)` was reproduced, so
the immediate cause is not merely an adopted stale host PID. The supervisor
was restored in rollback mode and `/health` returned `ok`. Configuration
preparation remains an external Wine baseline issue; Rust full-push claims
remain independent of this failed provenance experiment.

The first post-fix primary capture (`diagnostics/20260901-live-pair/primary-confirmed-1955/`)
does confirm `effective.login_primary=true` and ten non-7709 5188 TCP flows.
However, the Rust pcap summary found only `3901` two-byte control frames in
those flows and no reassembled `2704`, `3e04`, or other server business frames.
The synchronized Wine event file contains 17 large `股票数据` batches, but
without network business frames in the same window their callback provenance
is not established. This is an explicit contradiction to resolve, not
field-mapping evidence: determine whether the callbacks were buffered before
capture, delivered through another endpoint, or require a longer post-login
window before promoting any decoder result.

A second 120-second primary window (`primary-confirmed-2000`) reproduced the
same contradiction: ten 5188 flows and 11 total `3901` frames, but no
reassembled business frame, alongside 17 `股票数据` callbacks (26,590,658 raw
bytes). Repetition strengthens the observation while still leaving network
provenance unresolved; these callback bytes remain excluded from decoder
promotion.

## 2026-09-01 - Production wiring audit

The running Rust service and its publish worker still use `Tdx7709Session`
through `NETZIP_TRANSITION_7709_ENDPOINTS`; the capability response describes
the public contract as the legacy 7709 transition publisher and marks official
full-push as pending. The `Official5188Session` and initialization builders
are currently transport/research foundations, not the production publisher.
This distinction is now an explicit acceptance gap: do not claim the Rust
service has replaced Wine's official full-push chain until a production worker
owns authenticated 7100 state, opens non-7709 5188 sessions, and publishes
decoded full-push records independently of the 7709 worker.

The expanded routing capture (`primary-routing-2010`) included ports 6100,
7100, 5188 and local vendor proxies 16801-16805. It observed 100 inbound
5188 packets, all 10-byte `3901` keepalives/control replies, plus only one
local 16801 request/response pair. No 5188 business payload was present in
the window. This narrows the callback discrepancy: the visible OEM batches
are not being carried as ordinary 5188 payloads during the sampled interval;
the 16801 exchange and any buffered/local-file path require separate decoding.

The captured 16801 response is a structured local-proxy object beginning with
the same `penc` marker used by the Wine-side answer buffer. Its outer lengths
are 240 bytes on the response and 142 bytes on the request, with nested
6-byte entry records visible in the response. This is a stronger candidate
for the callback transport boundary than the sampled 5188 keepalives, but no
quote-field mapping is promoted yet; the next implementation task is a
bounded 16801 object extractor that can be correlated to callback sequence
and packet length without retaining sensitive values.

The response payload also starts with the established local-2000 header
`51 7f dc 7e 05 53 00 00`; its header fields declare a 240-byte object and
contain `penc` at the expected bounded-object region. This ties the 16801
proxy exchange to the existing `local_2000` structural analyzer and provides
a concrete implementation seam: reuse its length/header validation on
reassembled local-proxy TCP payloads before attempting any OEM interpretation.

## 2026-09-01 - Reject 0547 endpoints at the official 5188 boundary

`Official5188Session::connect_authenticated` now rejects both port `7709` and
port `547` (`0547` in project terminology). Both are supplementation paths;
accepting either at the official full-push constructor could silently route a
purported authenticated session onto the wrong protocol. The focused regression
covers unauthenticated, 7709 and 547 endpoints. This confirms endpoint-boundary
behavior only; it does not establish a live authenticated 5188 session.

The same change was followed by a fresh `vendor_pm.pcapng` ignored replay and
workspace test run. The replay still reconstructs the captured 5188 flows and
passes all assertions, while retaining unknown kinds and trailing bytes. This
reconfirms transport/reassembly stability only; no new inner-object or quote
field semantics were promoted.

`Official5188Session::receive_until_disconnect` now drives a bounded 5188
application-frame callback loop. A local TCP fixture verifies that split reads
produce complete frames in order and that peer close is surfaced as an error,
leaving reconnect policy to the authenticated integration. This is session
transport infrastructure only; it is not proof of authenticated live delivery.

`Official5188Session::reconnect_authenticated` now replaces the TCP stream and
clears the old reassembly buffer before a caller re-sends Wine initialization.
A two-connection local fixture verifies that a stale half-frame cannot bleed
into the new session. The reconnect policy is still not wired to the service's
formal login lifecycle.

Fresh client-direction extraction disproved the assumption that one captured
initialization sequence can be replayed after reconnect. Across ten complete
flows, the 95-byte `3610` frame changed on every connection, `2d10` formed two
distinct three-frame sets, and every `2a10` subscription differed within that
single capture. In two newer independent lifecycles, corresponding seven-way
`2a10` partitions are instead identical after ignoring connection identity. The temporary
combined reconnect-and-replay API was removed. Captured bytes remain shape
fixtures only; runtime initialization must be generated from the current formal
authentication session and connection assignment. Evidence:
`docs/forensics/official-5188-client-init-variance-20260901.md`.

README 7709 examples now use `<supplement-host>` exclusively. A repository test
rejects the historical `<authenticated-fullpull-host>` label and requires every
explicit port-7709 example line to carry supplement terminology. This prevents
operational documentation from silently undoing the protocol boundary.

## 2026-09-01 - Persistent authenticated control-session foundation

- `confirmed`: the formal `vendor_pm` 7100 flow contains ten Wine login-packet
  requests that decode to the same 448-byte, 12-field shape. Among fields safe
  to fingerprint, `请求`, `账号权限`, `券商`, `加密版本`, and `接口版本` are stable;
  only `编号` has ten variants across ten requests. Identity-field equality is
  deliberately not fingerprinted and remains `needs-verification`.
- `confirmed`: all ten dynamic 95-byte-payload `3610` frames now correlate to
  those requests. Nine responses contain the complete frame; one contains the
  exact evidence-backed 64-byte variable region. Control-response-to-5188-send
  delay is 74–111 microseconds in this capture.
- `rejected`: returning only `Auth7100LoginResult` is sufficient for official
  initialization. That path drops the authenticated TCP socket before the Wine
  initialization burst can be requested.
- Rust now has `Auth7100ControlSession`, which owns the socket that performed
  verified login and can exchange validated complete `网络包` objects on that
  same connection. Existing summary APIs remain compatible and intentionally
  close their socket.
- `needs-verification`: construction of the ten login-packet requests from
  current-session values and extraction/assignment of their `3610` responses.
  The persistent session API is lifecycle infrastructure, not proof of live
  initialization, decoded quotes, or callback parity.

Evidence: `docs/forensics/official-5188-control-frame-correlation-20260901.md`,
`diagnostics/20260831-netzip-windows-vs-rust/analysis/official-5188-control-correlation.json`,
and `diagnostics/20260831-netzip-windows-vs-rust/analysis/vendor_pm_7100_flow_matrix.json`.

The decoded per-connection request root is now structurally confirmed as
`加密包` with header words `[12, 0, 0, 448, 448]`; its twelve field spans close
exactly at byte 448. `build_candidate_official_5188_login_control_packet()`
encodes that shape through the verified raw-content dictionary and stable outer
envelope. It accepts all current-session values from the caller, has no
captured identity defaults, and is intentionally not wired into the resident
official chain. Cross-session field sourcing and byte parity remain required.

The request-number correlation is stronger but still session-scoped: login
packets use `2..11`, and each value is echoed as a server `应答编号`. ABK/ACK
then advance through `12..31` in two-number groups. Application-level TCP
reassembly still finds only nine distinct control objects containing the stable
67-byte-payload `3610` even though Wine sends it ten times. The missing tenth
source remains an explicit contradiction; no request is fabricated to close it.

ACK request shape is now represented by
`build_candidate_official_5188_ack_control_packet()`. Its eight fields and root
lengths 1685/1693 are evidence-backed, while the 1379/1387-byte opaque `数据`
must be supplied by the current lifecycle. The two data variants map perfectly
to the two observed `2d10 x3` sets (six versus four connections), but causality
is still `needs-verification`; the Rust runtime does not select captured `2d10`
bytes from that mapping.

`confirmed`: a second cold-start control session in `vendor_lunch.pcapng`
contains ten login, ten ABK and ten ACK objects. Its two ACK `数据` CRC32 values
match the same-length `vendor_pm` values exactly, while the occurrence counts
reverse from lunch `1379 x4 / 1387 x6` to afternoon `1379 x6 / 1387 x4`.
Therefore a fixed six/four distribution rule is `rejected`; two stable
role/partition templates is a stronger hypothesis. Because lunch captured no
client 5188 initialization frames, template-to-`2d10` causality and the live
selection rule remain `needs-verification`.

`rejected`: neither cold-start capture returns a complete 1379/1387-byte ACK
`数据` value as one decoded server field; the observed server `数据` fields are
75, 102 or 103 bytes. Direct whole-field copying is not the source. Embedded
or transformed use of those shorter values remains `needs-verification`.

Static runtime inspection assigns ownership to the official Stock DLL family:
both staged `Stock.dll` and `Stock.dat` export `Start/Ask/Stop`, and
`Stock.dat` contains `SFLogInPack=ABK`, `SSLogInRePack`, `&ack=`, `penc`, and
`hypenc`. The 101-byte `大智慧/C8_Login_1036.dat` is a complete historical
`3610` frame with a 93-byte payload and metadata `[0, 0, 3, 0]`; current
per-connection captures use 95-byte payloads. It is confirmed format evidence
but rejected as a current-session default.

Static and packet evidence now also locate the ACK-data lifecycle boundary.
`Stock.dat` function `0x10038120` handles numeric `0x1031` (wire `3110`); its
ACK branch copies caller-supplied bytes into session buffer `+0x6c261c`.
The only direct call to `0x10074200` later reads that same buffer and formats
the `&ack=` request. Every complete `vendor_pm` 5188 flow has two server
`3110` frames and one `3210` frame. Rust therefore classifies numeric
`0x1031/0x1032` as server initialization control, but leaves both payloads
opaque. Direct derivation from the 75-byte `3610` response is rejected; the
transform from server initialization payloads to the 1379/1387-byte ACK data
remains `needs-verification`.

The caller/accessor boundary is now narrower: `0x1000e0b0` computes the full
application-frame length as `8 + declared payload length`, and the ACK branch
copies exactly that many bytes from the reassembled frame object. The stored
inputs are therefore complete second-`3110` frames of 639 or 644 bytes. This
promotes the source-frame identity to `confirmed`; the expansion/encoding from
those frames to 1379/1387-byte ACK control values remains `needs-verification`.

The next helper copies the declared `3110` payload to a zeroed local buffer but
uses a C-string `MultiByteToWideChar` wrapper. Bounded inspection of all ten
`vendor_pm` flows found the first NUL at offset 64 for every 631-byte payload
and offset 111 for every 636-byte payload. Thus the effective `&ack=` source is
the 64/111-byte prefix, not the complete payload. Full-payload text conversion
is `rejected`; code-page conversion and generic control packing remain
`needs-verification`.

The ACK `数据` values themselves contain no NUL, `penc`, `hypenc`, or ZSTD
marker. Their visible-byte counts are 1293/1379 and 1301/1387, leaving an
identical 86 non-visible bytes in both variants; the eight-byte difference is
entirely visible. Follow-up request `182450-18d115af25cb0407` found exactly 43
visible runs separated by 43 CRLF pairs. Request `182905-18d115af25cb0409`
confirms zero non-ASCII bytes, zero other controls, 44 lines and a trailing
empty line. Both the fixed binary skeleton and mixed-GBK hypotheses are
therefore `rejected`; pure CRLF-delimited ASCII is confirmed. The exact line
grammar remains under structural analysis. All requests record field-local
analysis without raw values.

WebClx request `183433-18d115af25cb040b` passed seven focused tests for the
field-local line-shape and schema-label analyzer. It emits only length/class
shapes (`L`/`D` runs and punctuation) plus non-sensitive key/tag/path labels;
it does not retain alphanumeric values. This confirms the evidence-preserving
observation mechanism, not the ACK construction algorithm. The schema labels
and line-shape output remain `needs-verification` until correlated with the
Wine packer at `0x10074200` and its callers.

WebClx request `183606-18d115af25cb040c` passed the complete workspace test
suite, and release request `183642-18d115af25cb040d` produced the optimized
workspace binaries (`status=0`). These are build/regression evidence only;
the public service contract still does not claim live official 5188 parity.
WebClx request `184033-18d115af25cb040e` also passed the focused analyzer
suite after redacting ASCII strings associated with sensitive labels, closing
the alternate analysis-output leakage path.
Request `185449-18d115af25cb0413` passed all seven 7100 flow-matrix tests
after adopting the same bounded pktmon-prefix IPv4/TCP detection used by the
5188 scanner. Both offline research paths now agree on the packet-shape
boundary; this is transport evidence only and does not promote inner quote
decoding.
Request `185651-18d115af25cb0415` passed the complete workspace regression
(`status=0`; all crate tests and doctests), confirming the shared pktmon offset
logic does not regress the full-pull or supplement crates.

The formal Wine capture inventory was rechecked: its retained callback JSONL
contains 80 status/control rows with zero raw payload bytes, and its connection
timeline has no 5188 connection because primary login was disabled. It is
therefore useful for startup/supplement timing only, not for 2704-to-callback
field mapping. The large all-port ETL remains `needs-verification` until a
correct conversion yields a primary-enabled 5188 session.

`confirmed`: `Auth7100ControlSession::exchange_official_5188_init_triplet()`
now decodes current request and response objects instead of selecting a length
match from arbitrary response bytes. It requires the exact stage name, the
four response fields `请求/来源/应答编号/数据`, source `认证服务器`, matching
request/answer numbers, and one complete `3610` data field with payload lengths
95/94/67. WebClx request `171957-18d115af25cb03fd` passed 20 focused auth tests,
68 shared full-pull tests and the main binary check (log:
`/home/bin/webclx/logs/quoteNetzipRs/221_build.log`).

The triplet helper is not the live Wine state machine. Timestamped `vendor_pm`
frames confirm the interleaving
`3610(95) -> 3110(48) -> 3610(94) -> 3110(631|636) -> 3610(67) ->
3210(466|467) -> 2d10 x3`. All four 631-byte second responses precede one
`2d10` set and all six 636-byte responses precede the other; the `3210` length
does not cleanly partition the sets. Runtime code must therefore call the
strict login/ABK/ACK control methods separately and use
`Official5188Session::exchange_initialization_stage()` between them. Batching
the three control requests ahead of 5188 is rejected. The 631/636 correlation
is not a derivation rule for ACK data or `2d10`. In the newer
`formal-primary-0006` and `cold-sync-2311` lifecycles, the second `3110` is 632B
while both `2d10` groups still occur and a given subscription partition changes
group. Length-based and subscription-based assignment are therefore rejected.

`confirmed`: Rust now exposes a cross-socket
`Auth7100ControlSession::initialize_official_5188_interleaved()` orchestration
boundary. It performs login control -> `3610(95)`/`3110(48)`, then ABK control
-> `3610(94)`/`3110(631|636)`, invokes the caller's current-session ACK builder
only after those two server responses, then exchanges an ACK-length
`3610`/`3210(466|467|468)`
and invokes the current-session post-frame builder. The 5188 transport accepts
the tail only as validated `2d10 x3` followed by optional `0710/2a10`. A dual
socket fixture passed in webClx request `175958-18d115af25cb0403` (log
`/home/bin/webclx/logs/quoteNetzipRs/227_build.log`). The builders remain
caller-supplied because ACK and `2d10` derivation are still
`needs-verification`; this is an ordering/state boundary, not live completion.

`confirmed`: historical requests `141417`, `141929`, `142307`, `142435` and
`142757` each ran only `cargo build --release`; their notes are not direct test
evidence. WebClx request `180303-18d115af25cb0404` freshly ran and passed the
endpoint, continuous-receive, reconnect-buffer, reconnect-ordered-init and
README 7709-boundary tests (log
`/home/bin/webclx/logs/quoteNetzipRs/228_build.log`).

The separate `captures/2026-09-01_full-session/official_full_session.pcapng`
does not currently provide an independent ACK/`2d10` session. Its generated
7100 matrix contains zero client/server packets and zero TCP payload bytes, and
the companion connection timeline contains only localhost control sockets.
Treating it as second-session derivation evidence is `rejected`. The 1 GiB
`official_all_ports_followup.etl` remains `needs-verification` until it is
converted and inventoried with bounded tooling.

`190652-18d115af25cb0417` completed the release regression after the latest
static-evidence and skill updates. No new 2704/3e04 business decoder was
promoted: current samples still show only bounded structural prefixes and no
stable public-symbol marker. The next accepted experiment remains a
same-time-window Wine `OEM_REPORT` callback paired with one long-lived 5188
flow; until that fixture exists, inner-field claims stay
`needs-verification`.

`190933-18d115af25cb0418` completed the follow-up release baseline after
rechecking `2a10` consecutive-range extraction and the static-evidence ledger
(log: `/home/bin/webclx/logs/quoteNetzipRs/2048_build.log`). This confirms the
current structural parser and documentation remain buildable; it does not
close the inner-field or callback-parity acceptance gates.

## 2026-09-02 - Formal-account synchronized cold-start fixture

Authoritative paired fixture:
`diagnostics/20260902-live-pair/formal-primary-0006/`.

- `confirmed`: capture and callback collection overlap in wall-clock time. The
  5188 summary contains 38 directional flows, 680 complete frames, 230
  length-closed zlib objects and 15 server `2704` delta envelopes. The Wine
  JSONL contains 19 events, including realtime callback batches during the
  same application-data interval.
- `confirmed`: the six server flows carrying `2704` use local ports 48516,
  48528, 48532, 48546, 48550 and 48554. Early callback batches occur at
  1788278839673, 1788278839729, 1788278839762 and 1788278840684 ms, with
  `packet.count` values 294, 1059, 436 and 445 respectively.
- `needs-verification`: sums of `2704.prefix_word0` per flow (497, 377, 281,
  460, 430 and 57) do not directly equal callback batch counts. A `0x81` byte
  occurs near the apparent count but is not a stable exact delimiter. Neither
  observation is a record decoder.
- `contradiction`: the restart requested formal primary login and the same
  fixture directly records official 5188 initialization, business frames and
  callbacks, while `wine-status-post.json` reports effective
  `login_primary=false`/backup. The effective flag may be rewritten after
  startup or have different semantics. Preserve both facts; do not use this
  flag alone to accept or reject network provenance.
- `next discriminating test`: retain frame-completion timestamps in the pcap
  summary, retain Wine `packet.count`, packet payload length and raw length in
  the callback index, then correlate each `2704` against callbacks by local
  port and millisecond distance. Market/symbol identity is still required
  before promoting any payload field.
- `confirmed` (post-replay): frame-completion timestamps reduced the delta-only
  250 ms candidate set from 268 flow windows to 37 frame/callback pairs. The
  closest pair is 4.096 ms apart, but several ports contribute frames around a
  single callback, so nearest-time alone is not a one-to-one mapping.
- `confirmed`: realtime Wine packets obey the callback ABI exactly:
  `packet.payload_len = packet.count * 500` and
  `raw_len = 200 + packet.payload_len`. The first four overlapping batches
  contain 294, 1059, 436 and 445 `OEM_REPORT` records. Their code ranges and
  market mixtures are now retained in the source JSONL, but no 5188 payload
  field has yet been promoted from this callback-level invariant.
- `confirmed`: the former `2704.prefix_word0` is the per-frame internal record
  count read by Wine `0x44aa30`; it is not required to equal a Wine callback
  batch count because callback publication may aggregate across frames/ports.
  Fresh
  corrected samples around the first callbacks include values 57, 122,
  140-141, 149, 155 and 166-167, while callback counts are 294, 1059, 436 and
  445. The earlier direct-equality hypothesis remains rejected, while the
  field itself is now named `record_count`.
- `confirmed`: decoded `0104` market objects contain a 98-byte header followed
  by fixed 68-byte records. Each record starts with a `u16 symbol_index`, so
  six-digit code offsets are 100, 168, 236, ... in
  both SH and SZ objects, and each record carries a GBK security name (for
  example SH `000001` is `上证指数`). This classifies `0104` as market/code
  metadata rather than a direct 500-byte `OEM_REPORT` batch. Its fields may
  still provide the symbol dictionary needed to decode `2704` identifiers.

## Next experiment template

For each protocol probe, create a dated note under `docs/forensics/` containing:

```text
account_class: formal | test-168
time_window: ISO-8601 start/end
hypothesis: one sentence
inputs: fixture paths and SHA-256, selected endpoint class
observed: frame kinds/counts, connection lifetime, decoded fields
comparison: same-symbol Wine callback or explicit “not comparable”
disposition: confirmed | rejected | needs-verification
follow_up: smallest discriminating test
```

## Contradiction ledger

| Claim | Current disposition | Discriminating test |
|---|---|---|
| “full-push” service is official full-push | Rejected by code: it builds `Tdx7709Session` | runtime socket/port trace after service rename |
| 7100 success implies a usable data session | Not established | same-account login-to-5188 trace with bound session id |
| 7709 unsolicited delivery equals official push | Rejected by boundary semantics | source identity and protocol contract test |
| All supplement modes are online | Rejected/partial | per-mode executable integration tests |
| Wine official business stream is 5188 | Needs-verification; continuation-2147 observed a complete 6100 `压缩|ZSTD` packet while 5188 carried only 3901 control frames, but existing 6100 evidence documents login/dictionary/download traffic on that port | Classify the bounded 6100 packet as auth/download versus market data before correlating callbacks; preserve 5188 as a separate control/data hypothesis until a business frame is proven |

## Replacement acceptance gates

Do not claim replacement until all are green: authenticated non-7709
long-lived sessions; Wine-shaped initialization; decoded 5188 fields with
same-symbol callback parity; representative full-market coverage and freshness;
reconnect/recovery; independent `netzip-fullpull` source identity; and proof
that 7709 failure does not alter the full-push chain.

The updated callback-window helper was syntax-checked against the formal
resilient collector: all 80 JSONL events expose `timestamp_ms` and are now
eligible for bounded time-window evaluation. The available 5188 summary still
lacks an overlapping session, so this improves readiness without creating a
false match.

The callback-window analysis helper now accepts both ISO `received_at` events
and the `timestamp_ms` field used by the formal Wine JSONL collector. This
removes a silent indexing gap; generated matches remain evidence-only
`candidate-window-only` records until payload and symbol parity are verified.

The synchronized collector now emits compact one-object-per-line JSONL after
sequence deduplication (`scripts/capture-primary-callback-pair.sh`). Release
verification `230445-18d1330bb455c4df` passed; this prevents formatting from
masking timestamp overlap in future Wine/Rust paired captures.

## 2026-09-01 - Live login dialog validated on test-168 (contribution from netzip_win)

A live round trip from the netzip_win port (`crates/netzip-fullpull/src/auth_7100.rs`)
confirmed the current initial login and the death of the legacy one. Full
evidence: `docs/forensics/live-login-dialog-20260901.md`.

- `confirmed`: the current initial login is the 12-field `加密包` shape
  (request name `大智慧C_登录包`, credentials filled, dictionary-compressed).
  Live test-168 exchange: 363B request -> 398B four-field response with the
  request/source/number echoes and a current-session `3610` (81B payload) in
  `数据`. Follow-ups are the byte-identical 271B download and 333B
  `Tdx_Encrypt` templates; the `Tdx_Encrypt` response root is `加密解密`.
- `confirmed`: the 2026-08-06-era `认证|登录` object is rejected today —
  ordinary-ZSTD variants are silently dropped (socket held ~30 s, then closed
  with zero bytes), dictionary-wrapped variants get `认证服务器不支持该请求`.
  `auth_7100_client.rs` still defaults to the legacy path and needs this port
  before its next live login attempt.
- `confirmed`: the `align-boot-2210` 6100 pair (582B client / 326B server,
  field44=12) is this login dialog, not market data; the "inflated root named
  认证" classifier note was an outer-envelope artifact.
- `confirmed`: decoding a plain-ZSTD control frame with a dictionary-primed
  decoder silently corrupts output; accept the dictionary decode only when the
  root is `加密包` and otherwise fall back to plain ZSTD.
- `confirmed`: test-168 download route set is 5 entries, all `7709/7709` —
  supplement-only provisioning; 5188 initialization work requires the formal
  account.
- Current-session `3610` payload length is value-dependent: 81B (initial
  login), 94B (zero-field per-connection exchange), 95B (formal `vendor_pm`),
  93B (historical fixture). Strict 95/94/67 stage gates match the formal
  chain; length derivation stays `needs-verification`.

## 2026-09-01 (~23:35 +08:00) - Formal-account live login via the new dialog (netzip_win)

User-authorized formal-account login executed through the netzip_win
port of the new dialog (`NETZIP_LIVE_ALLOW_FORMAL=1` gate; credentials were
process-memory only and never printed):

- `confirmed`: account 1522 completes the full dialog live: probe 430B->342B,
  login 365B (decodes 468B, 12 fields) -> 402B four-field echo, download
  1540B, `Tdx_Encrypt` 404B. Response sizes differ from the test-168 exchange
  (363/398/460/275B), confirming a genuinely distinct account over the same
  protocol shape.
- `confirmed` (new, time-window fact): at ~23:35 the download route set for
  the formal account is also 5 entries, all `7709/7709` — the payload has 23
  UTF-16 lines with no 5188 row anywhere, so this is server-side routing
  behavior, not a parser truncation. Combined with `restart-2155`/
  `align-boot-2210` (21:55/22:10, ten 5188 flows after restart), the fresh
  login route list withdraws the 5188 rows sometime between ~22:10 and
  ~23:30 after close — with the open alternative that Wine reconnects to
  locally cached endpoints, so the trading-hours rerun stays the
  discriminating test. The earlier "test-168 is supplement-only" disposition
  must be narrowed: provisioning vs time-of-day was not separable until this
  formal run.
- Zero-field per-connection exchange returns `3610` 94B on the formal account
  too; the 95B formal baseline therefore comes from real field values, not
  account class.
- NEW (23:45 +08:00, read-only netstat on the local Windows install): the
  ORIGINAL vendor client currently holds **ten ESTABLISHED `58.16.134.228:5188`
  connections** plus one `121.41.70.217:6100` control connection and a local
  `127.0.0.1:5188` OEM listener. That endpoint IP appears in NO file under the
  installation directory (full ASCII + UTF-16LE scan), so 5188 endpoints are
  delivered by the service through a control channel that is not yet
  replicated — prime candidate: the ~100-second stable request/variable
  response exchange on the retained 6100 socket recorded in the control
  correlation, or a second `下载文件` request with a different file name.
  This supersedes the pure time-window reading: at 23:45 the original client
  still holds 5188 while a fresh login route list has none. The replication
  path is: extract the stable request bytes/ cadence from `vendor_pm`
  offline, replay it on an authenticated control socket, and observe whether
  the response carries a 5188 endpoint object.
- Next discriminating run (trading hours only): rerun the same acceptance
  between 09:30-15:00, expect 5188 rows in the download response, then run
  Phase D (authenticated 5188 connect) and Phase X (per-connection
  `3610`->`3110(48)`) with real field values for the first time.

## 2026-09-02 (~01:30 +08:00) - Full authenticated chain replicated live (netzip_win)

- `confirmed`: the 5188 endpoint channel is a second `下载文件` request for
  `系统\大智慧服务器L1.ini` after the real `请求登录`. The response (1880B,
  uncompressed UTF-16 config, BOM at offset 422) is broker/permission-filtered:
  the formal account (华创/点播版) receives 4 active 5188 rows
  (222.85.139.177 / 58.16.134.228 / 103.141.11.1 / 1.202.82.150); other
  brokers' rows are `//`-commented. Row shape is 7 columns
  `券商,权限,名称,IP,主端口,接口版本,软件版本` with 接口版本=858.
- `confirmed`: the real login request is now replicated byte-for-byte:
  19-field `认证/请求登录` manifest (854B inflated, plain level-3 ZSTD, no
  dictionary — the compressed frame equals the captured 462B frame). The
  server answers 436B / 15 fields with `提示信息=登录成功` and
  `账号到期`. Field values were extracted from `vendor_pm` (client build
  tokens: Stock.exe 版本 362, date token b3f90300, file CRC32 tokens,
  通道=0). The August legacy template fails because its manifest tokens are
  stale, and dictionary-compressed variants are rejected outright.
- `confirmed`: the 3610 payload length is stage+value dependent: 95B
  (login stage, real values: 本地IP, 账号权限=点播版, 券商=华创,
  接口版本=858), 94B (same stage, zero values), 94B (ABK stage, real
  values), 81B (initial-login data frame). The ABK 3110 response was 632B
  live — a third length variant (631/636 captured previously); first NUL at
  offset 35 for this variant, so the `&ack=` source prefix is
  value-dependent too.
- `confirmed`: the ACK `数据` (1387B) is a file-manifest text: 44 CRLF lines
  whose rows carry size/CRC32/date tokens (e.g. `2990932234`, `20260831`) —
  same family as the login/heartbeat manifest. Generating the runtime's own
  manifest text from its installation inventory is the remaining step before
  the ACK stage and `2d10` work can resume.
- Working implementation: `Z:\stock\crates\netzip-fullpull\src\auth_7100.rs`
  (project moved/renamed from netzip_refactory; driver-level live acceptance
  passes: login → L1 list → 5188 connect, holding pre-init honestly).

## 2026-09-02 (~02:30 +08:00) - ACK stage reaches the data connection (netzip_win)

- `confirmed`: the ACK `数据` grammar is fully decoded — `SFLogInPack=ACK` +
  `<MarketInfo>` (9 market-version rows), `<FileInfo>` (16 file|CRC32 rows),
  `<SDidsCrc>` (8 did|Crc rows), CRLF-delimited, trailing empty line. The
  netzip_win port generates its own manifest (`build_ack_manifest_text`) from
  its installation's real file CRC32s with fresh-install market state.
- `confirmed`: the auth-node control side ACCEPTS a generated manifest and
  answers with a current-session ACK-stage `3610` (44B for a 592B manifest;
  67B for the captured 1387B one — length scales with manifest size).
- `needs-verification`: forwarding the generated-manifest stage to the 5188
  data connection gets the connection closed by the server, while the
  captured 1387B manifest would presumably be accepted — the remaining gap is
  manifest VALUE semantics (MarketInfo server-state rows, complete file set)
  and/or trading-hours state. The `3110` ABK response also showed a third
  length variant (632B, first NUL at 35) under reconstructed values.
**UPDATE (~03:30): STAGE 3 PASSED.** Adding the 9 structural market rows
(market ids 18515/23123/9282/18003/19272/22350/17999/20307/19010 with their
attrs, state fields zeroed) to the generated manifest made the control side
accept a 1103B manifest and answer 3610(63B); forwarded on the 5188 data
connection, the server replied **3210(466B)** — the captured ACK-stage
response shape. The empty-MarketInfo close was the earlier blocker. The
remaining unknown is runtime `2d10 x3` seed/group selection and subscription
slot assignment. Later full-payload searches found neither stable `2d10` group
inside `3210(466|467|468)`, any `3110`, nor decoded 6100/7100 objects. `3210`
therefore confirms ACK-stage acceptance but is excluded as a direct `2d10`
source. The two adjacent sessions share concrete group values, while an earlier
date/batch shows value evolution; captured words remain fixtures, not defaults.

## 2026-09-02 (~20:08 +08:00) - Wine 主站掉线根因确认并修复（决定性）

- root cause（confirmed）：quoteNetzipWine `DllRuntime::initialize()` 默认发送的
  旧式同步 `Ask(请求=登录&模块=认证&…&编号=0)` 与网际风自身配置自动登录冲突/超时返回 0，
  vendor 立即断开股票主站并切备用，并把 `用户/配置文件.ini` 写回 `登录股票主站=0`。
  这是此前白天/收盘后多次 Wine「主站几秒~几十秒后掉线、只剩备用 7709」的真正原因。
  脱敏 Ask 标记证据：ask-diagnose-1931 run，Ask returned 0 [模块=认证 编号=0] 后 24ms 断开。
  冷启动主站常 ≤60s 掉线被旧抓包窗口掩盖；formal-primary-0006（00:06）~65s 本地 FIN+RST 证实。
- fix（confirmed）：`initialize()` 默认 `QUOTENETZIPWINE_INIT_LOGIN_MODULES=none`，
  跳过两条旧式登录 Ask，只发只读 `请求=初始化`；主站/备用登录交给网际风按配置文件自动登录
  （配置 `登录股票主站=1` 由 restart 前显式 set-vendor-login-primary true 确保）。
  `run-host-wine.sh` 白名单透传该 env；Ask 诊断保留脱敏命令标记。
- 验收（confirmed）：fixed-accept-2000 run，20:00:45 主站登录成功，10 条 58.16.134.228:5188
  保持 ESTAB ≥480s（至 20:08:47），无 Ask returned 0/无断开，login_primary=true 且 login_backup=false，
  认证 100s 心跳正常、gateway 持续成功 post。
- 运维注意：重启/进程管理须按 /proc cwd+comm 精确匹配，禁止 `pkill -f '…exe…'`（会杀自身脚本树）；
  实验 env 须 `systemctl set-environment` 或写入 /etc/quoteNetzipWine-wine-supervisor.env，
  命令行 env 经 systemd supervisor 不保留；避免重启期间轮询导致误判服务掉线。
- 经验文档：docs/forensics/wine-primary-disconnect-root-cause-ask-auth-20260902.md、
  docs/forensics/wine-primary-hold-1954-20260902.md。

## 2026-09-02 (~20:24) - 收盘后主站保活机制确认（0x0139 心跳）

- closing-parity-2020：180s 稳定主站抓包，S2C 仅 290 帧全部 kind 0x0139，
  payload 恒 2 字节 00 00，~0.83s/条 —— 纯 keep-alive，无业务。
- Wine quote_batch 6187 只×周期是本地 OEM 缓存重放，非网络增量（勿做 parity 真值）。
- Rust：Official5188Kind::SERVER_HEARTBEAT(0x0139) 常量+方向+测试加入；上层应忽略该帧。

## 2026-09-02 (~20:40) - 5 主分区 100% 验证 + 动态 6/7 精确定位（离线）

样本：fixed-accept-2000（稳定主站 58.16.134.228，10 条连接，20:00 冷启动全捕获）
方法：每条连接用自己收到的 SH/SZ 0104 code-table 构造 Rust 模型
  (SH 600-699 + SZ 000/001/002/003/300/301, 0x10000|ordinal, 顺序保留) 的 5×1024，
  与该连接发送的 2a10 逐字节比对。
- confirmed：P1..P5 五条连接（60844/60860/60872/60882/60890）各发一个 1024 分区，
  与模型 partition 1..5 完全逐字节一致（SH/SZ 表 26485/4600 行, eligible 5214>=5120）。
  Rust `build_official_5188_primary_subscription_partitions` 建模正确。
- confirmed（新）：第 6 条连接(60894)发 1024 分区（SH578+SZ446，930/1024 非 primary，
  含 SH 000xxx 指数/其他 + SZ 302 等）→ 动态/非主推分区 6，模型未覆盖。
- confirmed（新）：第 7 条连接(60904)发 59 SZ 分区（159xxx 场内基金/ETF 连续段，
  ordinal 2374.. 附近）→ 分区 7，模型未覆盖。
- 结论：Wine 一条连接一个 2a10 分区；5 主推分区已完全复刻，P6/P7 是需要下一步
  重构的动态补充订阅（来源：0104 全表除去主推后的剩余 eligible？或基金/指数独立规则）。
- 待验证：P6 是否 = 剩余 non-primary 全市场某 1024 切片、P7 是否 = SZ 场内基金全集/切片；
  需要另一 lifecycle 对照（同账号同会话多连接 + 跨会话稳定性）。

## 2026-09-02 (~21:00) - 接收清单 = 全部 7 分区订阅统一来源（决定性）

- 证据：`用户/只接收股票代码表.csv`（UTF-16LE TAB，启用标记=1，6955 个 SH/SZ
  代码）与 fixed-accept-2000 解码的全部 7 条客户端 `2a10`（6203 条）逐条比对：
  **6203/6203 命中**（P1–P7 各分区 100%）。
- 细粒度：P6 SH 51x/56x ETF 段 408 条 0 条不在清单；清单内该分类约 38 条未进
  P6（1024/分区上限内 SH 578 配额截断）。跨交易日（09-01 vs 09-02）P6/P7 随
  0104 表变化平移，证明是基于当前 0104 表 + 接收清单的确定性派生。
- 结论（confirmed）：Wine 的 2a10 构造 = 接收清单 ∩ 当前 0104 表，按内部分类
  排序后切 1024/分区；P1–P5 的 eligible 前缀结构是清单覆盖主推证券时的自然
  结果，P6/P7 是清单中 non-primary 证券的分类补充切片。
- Rust 实现方向：以当前 0104 表把接收清单映射为 (market, code, ordinal)，再按
  分类段序切分区。边界：清单文件行号 ≠ wire ordinal；分类段枚举序（26 段）仍
  需从多生命周期反推；换 broker/权限后清单是否随之变化未验。
- 证据笔记：docs/forensics/cross-login-type-subscription-identity-20260902.md。

- 强化（21:04）：formal-primary-121528（12:15，独立登录）用其自有 0104 表解码
  的 6203 订阅与同一 12:16 版只接收清单比对同样 **6203/6203**。同清单贯穿
  121528 与 fixed-accept-2000 两个独立生命周期 → 清单为订阅统一来源获双窗口
  证实（含 59 条 P7）。清单行号 ≠ wire ordinal 边界不变。

- 集合论刻画（21:10）：只接收清单 6955 中，清单∩当前 0104 = 6203 = 全部订阅
  集合（752 清单码不在 0104 表，0 条在 0104 却未订阅）。P1–P5(5120) = 交集
  eligible 前 5120（5214 eligible 截 94 尾）；P6+P7(1083) = 交集剩余。
  P6 起始 SZ301 而非清单文件序(SH 前) → 段级顺序为 vendor 26 段分类枚举，
  顺序规则推导与「顺序是否影响 2704 收流」仍开放待在线验证。

- P6 段序边界（21:18 补充）：0104 记录含 GBK name，可佐证证券类型（如
  510010=180治理ETF、110075=南航转债、900901=云赛B股、588000=科创50ETF、
  000001=上证指数），vendor 的 26 段顺序疑似按「指数/ETF/转债/B 股/基金等
  类型 + 市场」的枚举序，但**分类规则本身未证实**（依名称前缀猜测属
  needs-verification，不作为实现依据）。实现用 0104 SH-then-SZ 降级序，
  待在线 2704 覆盖实验确认顺序敏感度。

## 2026-09-02 (~21:48-21:51) - Parity A 桶修复与真实失配率确认（决定性）

- A 桶修复（webClx `214535-18d1330bb455c5d9` / 3177）：parity 对比器接入
  zero-amount delta-skip 规则（`internal_amount==0` → `0x44aa30` 保留旧
  public-state，不视为 mismatch）。**字段命中数大幅提升**：amount 83→1013、
  price 355→1281、volume 377→1307、timestamp 974→1353（全部 12 字段 +900 以上）。
  A 桶 10 条全部消除。
- D 桶收窄：A 桶修复后剩余 20 条 mismatch_samples **全部来自 seq=34 单一批次**
  （15:00:00 收盘竞价窗口）+ 6030xx 连续代码段。D4+E11+C4+B1。这不是散布性
  解码 bug，而是一次性收盘批量更新差异（Wine 收到了完整不同版本增量）。
- **真实失配率 = 20/1792 = 1.1%**（远低于旧口径 310/2102 = 14.7%）。
  残留集中在收盘竞价单一批次，需交易时段新窗口验证是否为收盘特有。
- 证据：`/tmp/parity-abucket-fix2.json`、
  `docs/forensics/cross-login-type-subscription-identity-20260902.md`。

## 2026-09-03 开盘验收（09:15-09:45 +08:00）- Wine 端到端供数确认

样本：diagnostics/20260903-live-pair/open-capture-0937/（6min 窗口 09:37:48-09:43:22）
主站：222.85.139.177:5188 x10 ESTAB，凌晨 02:18 起持续 ≥7.5h，login_primary=true / login_backup=false
- 09:25:08 集合竞价定盘：浦发 9.24(昨9.28)、茅台1297.5、宁德349.99、平安银行/中国平安等全市场
  5841 只非 0 价 → Wine quote_batch 给出 09:25:00 竞价快照。
- extract：45249 帧/6min，含 2704 x37655 + 0421 x5702 + 3e04/bulk x1892 + 心跳；2704 值解码
  需 Wine core baseline（delta-indexes 已解，values 因缺基线 not decoded）。
- 网关 09:15-09:45：received_batches 29081 / received_quotes 1842461 / successful_posts 4955 /
  dropped 0 / retried 0 / last_error null → 洪峰无丢弃。
- stockScreener 端到端落盘：data/realtime/preopen_quote_dump/2026-09-03.jsonl（09:14-09:30，
  2692 个 update 事件）feed_source 统计 quoteNetzipWine x9910 最大单源 + netzipRust7709 x7492
  + rusthq 多组 → Wine 是选股真实行情源之一且被打标。
- 结论：quoteNetzipWine 今晨已在真实开盘向选股提供 09:25 竞价+09:30 连续行情，链路 0 丢弃，
  选股文件持续更新（realtime.jsonl 09:45 仍在写）。

## 2026-09-03 (~10:00) - Rust 2704 值解码负面结论：缺正确 baseline 会产生伪正确错误值

样本：open-capture-0937（09:37:48-09:43:22, 222.85.139.177:5188）
- 帧量：2704 x37655 + 0421 x5702 + bulk x1892；delta-indexes 全解出（37655 帧），
  symbol→code 用 09-02 fixed-accept 0104 表映射成功（SH/SZ 索引跨日稳定）。
- extract 跨帧 baseline resolver 让 347 帧"成功"产出 decoded_values（1271 条可映射记录）。
- **这些 decoded 值经 scale 换算后与网关 quoteNetzipWine 权威值不符**：
  600259 中稀有色 解出 0.06（网关 ~19元）、600765 中航重机 0.0（~19元）、
  688693 锴威特 -1542（荒谬）、688365 光云 226 vs 网关 ~17元。
- 根因：uses_baseline=true 帧是相对"某开盘基线快照"的 delta，不是相对前一帧；
  用跨帧累积的伪 baseline 解 delta 会静默产出错误 record。
- 结论：Rust decoded values 在缺真正开盘基线前**不可信**；保持
  business_decoder=opaque-evidence-only。绝不能把此类"成功解码"当 parity 通过。
- 下一步真正基线来源候选：开盘冷启动瞬间的 0104+首推全量（非 delta）；或
  Wine 内存/实时.dat 的 311B 内部记录快照；或 3e04/0104 bulk 对象。

## 2026-09-03 (~10:06) - netzip_win 并行会话真实打通认证→5188 全链路（重要协作更新）

- 并行会话（netzip_win, webclx s5228-5298）08:12 linux-login pcap 证明 Rust 复刻驱动
  已在真实环境完成：6100/7100 认证（探测 428→342、登录 623/267/378/363/628 →
  436/1880/385/393/359，含 1880B L1 下载）→ 222.85.139.177:5188 冷启动连接 →
  完整初始化（3610×3→3110×2→3210→0415 文件→0104×4→2d10×3→2a10×5→0427 业务帧流）。
- 资产：/home/codes/stock/netzip_win/diagnostics/20260903-linux-login/extract-0812/
  含今日最新 SH26485/SZ4600/B$790/SF742 全量表 + 完整生命周期帧。
- 并行会话昨晚（09-02 21:00-21:51）已把 2704 parity 推进到：amount 83→1013、
  price 355→1281、volume 377→1307 匹配；剩 D 桶 20 条（全来自 15:00 收盘竞价批次，
  真实失配率 1.1%）；B 桶负 amount 已修。P1-P5 逐条复刻 + P6/P7 集合证实(6203/6203)。
- 结论修正：Rust 复刻的协议/解码层实际接近完成（netzip_win + fullpull crate），
  quoteNetzipRs 服务端(16893) 显示 authenticated=false 仅说明该服务进程未接入这套
  已验证驱动，不代表复刻未完成。下一步关键是服务端生产接线决策。
- 2704 绝对基线疑问澄清：冷启动首帧 0427 也是 uses_baseline=true（服务器侧持久基线），
  绝对基线非单帧可得；netzip_win 昨晚 parity 用 15:00 收盘 + Wine core 基线完成，
  说明基线来源 = Wine 内存/实时.dat 311B 记录快照（load_core_baselines 路径），
  非 09:25 定盘帧。明日脚本仍可用于独立复核。

## 2026-09-03 (~10:15) - netzip_win 正式账号完整打通官方 5188 初始化（决定性里程碑）

- 正式账号 1522 真实运行（凭据仅进程内存，未打印）：
  探测→登录成功(4服务器)→选 222.85.139.177:5188→交错初始化完成
  →0104 x4→post-init 8帧(2d10 x3 + 2a10 x5)→收到服务端帧→DriverStatus::Connected。
- 修复的协议变体：ABK 阶段 3110 首 NUL=532（631 长度），历史样本只有 64。
  official_5188_ack_source_prefix 631=>&[64,532]；新增 ACK-DIAG 取证打印。
- 代码路径全部真实执行：login/ABK/ACK 三阶段交换、ACK manifest 构造、
  0104 收集、2d10 triplet、5主分区 2a10、post-init 发送。
- 驱动事件含 "官方 5188 初始化完成；0104 4 张，post-init 8 帧" 与
  "官方 5188 收到 2 个服务端帧" → Connected。
- 意义：Rust 复刻（netzip_win+fullpull）协议/初始化层已真实闭环，
  从"只能 shadow"推进到"真实初始化+收到服务端数据"。剩余：接收循环持续化、
  2704 业务解码接线、动态 2a10 6/7 集合来源、服务端生产 lane。

## 2026-09-03 (~10:30) - netzip_win 正式账号持续接收业务帧（决定性里程碑2）

- 正式账号 1522 live：初始化完成(0104x4+post-init 8帧) → Connected →
  51s 内持续接收：服务端帧 349 / 业务帧 344（~6.8 帧/s，2704 delta 等持续流入）。
- 修复的协议认知：ABK 3110(628-636) 与 ACK 3210(464-468) payload 长度逐连接漂移，
  非固定档位。official_5188_ack_source_prefix 改为"任意首个 NUL 即边界"（非长度表）；
  ABK 接受 600..=680、ACK 接受 450..=480。驱动接收循环加 10s 周期存活/帧数 emit。
- fullpull 测试 76 项全绿；netzip_win release 经 webClx 4030-4032。
- 意义：Rust 复刻（netzip_win+fullpull）已真实保持官方 5188 会话并持续收业务帧，
  协议/初始化/长连三层全部实跑通过。剩余：2704 值解码接线(用持续帧+Wine基线)、
  动态 2a10 6/7、生产服务 lane、业务字段 parity 上线判定。

## 2026-09-03 (~10:30) - Live 2704 帧协议解析验证通过（里程碑3）

- netzip_win 驱动加 NETZIP_NATIVE_DUMP_DIR，正式账号 30s live 导出 199 条 2704 payload。
- 用 fullpull decode_official_5188_delta_indexes 逐帧解析：199/199 全部成功，
  records 153-173/帧，symbol_index 连续段、uses_baseline=true、实时 timestamp。
  (examples/live2704_probe.rs, webClx 4004-4006)
- 结论：live 会话 2704 与离线样本结构一致，Rust 解码器可直接消费驱动收到的业务帧；
  接 Wine core 基线即可解出完整行情值。2704 live 解码最后一块拼图就位。

## 2026-09-03 (~10:33) - Live 数据真实性端到端确认（里程碑4）

- Rust live dump 的 2704 delta 帧 symbol 映射（用 08:12 今日 0104 表）：
  SH23677->600022、SH23830->600228、SH24701->603077，均为真实活跃 A 股。
- 网关 quoteNetzipWine 源同三标的有 10:33:0x 实时价（1.34/10.5/2.31），
  证明 Rust 会话收到的 delta 与 Wine 同源同标的，非噪声/伪造。
- 完整证据链闭合：正式账号登录->L1->5188 连接->真实初始化->持续收 2704
  delta(199帧/38.5s 可解析)->symbol 映射真实活跃股->网关同源确认。
- Rust 复刻的"真实接收官方行情推送"已全部实跑验证；剩余仅为
  ①今日 Wine 311B 基线获取(live delta 解绝对值) ②生产服务 lane 接线 ③P6/P7 来源。

## 2026-09-03 (~10:45) - Live 2704 基线语义确证（关键协议认知）

- 尝试三来源基线解 live 2704 delta，结果均不能得到网关一致绝对值：
  ① 9/2 core（隔夜）：44/199 帧可解但值系统性漂移（不同 symbol 不同 scale 偏差）
  ② 今日 Wine 内存 dump（/proc 读 6161 条 311B 记录，timestamp 今日实时）：50/199 帧，
    但 last_int 与网关价仍不成比例（301220 解 4342 vs 网关 26.9）
  ③ 空基线：全失败（amount 负回绕）
- 结论：2704 delta 是相对**服务器端参考帧**（连接/开盘时某全量快照）的增量，
  不是相对 Wine 内存当前值。Wine 内存已随 delta 累积到最新，与服务器基线不同步。
  9/2 core 恰好≈当日服务器基线所以那晚 parity 正确；跨日则失效。
- 真正解法：必须在**新数据连接建立瞬间**捕获服务器首推的全量参考帧（非 delta 的
  2704/3e04/全量对象），Rust 从该点开始累积 delta。这就是明日 09:25 冷启动抓包
  (open-auction-baseline-capture.sh) 要抓的目标，且需 Rust 会话从连接即持续 dump。
- Wine 内存 /proc dump 方法有效（能读 6161 条今日 311B 记录），用于 Wine 自身值
  校验可用，但不是 Rust delta 的正确基线源。

## 2026-09-03 - 2704 baseline contradiction reopened

- `confirmed`: cold-start and mid-session captures observed 2704 records with
  `uses_baseline=true`; cross-session Wine current-state records produce drift.
- `needs-verification`: those observations do not prove that the missing state
  is a server full-reference object. Earlier evidence also attributed working
  parity to Wine core/`实时.dat` records, so baseline ownership remains disputed.
- Discriminating evidence: rebuild and hash the replay tools, capture from the
  beginning of one connection, preserve all initialization/bulk objects, and
  compare decoded fields against same-session, same-symbol Wine callbacks.
- Until that comparison passes, do not describe either server reference frames
  or Wine current-state records as the authoritative decoder baseline.

## 2026-09-03 - Byte44 replay corrected by webClx 4067

- `confirmed`: request `170424-18d1a65e45a96d23` explicitly rebuilt the three
  replay examples after passing 172 fullpull tests with 1 ignored and Clippy.
- `confirmed`: the 08:12 cold 0104 tables contain eleven byte44 values, not all
  zero. Modes 1 and 3 dominate; SH `603059` has mode 1.
- `confirmed`: the rebuilt byte44-aware extractor decodes 0/11 cold frames;
  failures expose negative amount accumulators in nonzero modes. The earlier
  11/11 and all-mode-zero statements came from a stale example binary.
- `needs-verification`: byte44's complete Wine mode semantics and tagged amount
  projection. Do not relax the guard or publish projected amounts without a
  same-symbol Wine comparison for representative nonzero modes.

## 2026-09-04 01:20 - Cold-start 2704 uses a fresh slot, not the 0104 seed

- `rejected` (H4): inserting `from_code_table_metadata` into
  `Official5188MapBaselineResolver` as the `mask&1` baseline. 0104 tails still
  supply 昨收/涨跌停 metadata (`0x120..0x137`); they are not the previous
  311-byte business record.
- `confirmed` on a same-session night cold start (Wine supervisor restart
  2026-09-04 01:19, capture + memory dump under
  `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/`,
  gitignored): `seed_code_tables` + `decode_official_5188_values_with_fresh_fallback`
  reproduces Wine's in-memory OHLC/volume/amount/昨收/bid1/ask1 for the initial
  dump (38 frames, 5171/5171 records, 0 missing slots). Mode that treats 0104
  as baseline does not.
- `confirmed`: `实时.dat` last write remains 2026-09-01; Wine did not restore
  disk state. The connect-time server dump is sufficient.
- `needs-verification`: trading-hours `OEM_REPORT` same-symbol/same-timestamp
  match (09:25 window). Night memory identity is decoder evidence, not
  production publish evidence.
- Product: shadow reader uses the fresh-fallback decoder; public projection
  stays `opaque-evidence-only`. Mid-stream captures still use the strict
  decoder (`requires missing baseline`).

## 2026-09-04 12:22 - Layer split: memory identity vs callback parity

- `confirmed`: auction callback statistics in
  `docs/codex/tasks/official-5188-open-gaps-20260904.md` mix three layers.
  Full write-up:
  `docs/forensics/official-5188-layer-split-20260904.md`.
- `confirmed`: 6,832 commit-v5 matches are successful layer-A records;
  bid/ask array hits of 35–41 therefore cannot steer `ladder_volumes` token
  work. The compare fills OEM slots 5–9 with zero and requires 10-float
  bit-equality.
- `confirmed`: 7,466 unmatched is first a ±250ms join / coverage question.
- `confirmed`: ACK `3610` length 62 vs 67 scales with client manifest size
  (1387B→67, default rows→62). Do not hard-code 67-byte captures.
- `needs-verification`: OEM ten-level expansion from the five-level 311-byte
  ladder; amount float units vs internal integers; P6/P7 vendor 26-segment
  order vs SH-then-SZ.

## 2026-09-04 - open-0921 receive-only window inventory

- `diagnostics/20260902-live-pair/open-0921-20260903/full-5188.pcap` is a
  post-initialization server-to-client capture: 9 observed 5188 flows, 2,167
  `2704` business frames and 18 `3901` heartbeat frames. No client control
  frames are present because capture began after initialization writes.
- The paired Wine callback log contains 6,000 events and 101,516 quotes from
  `09:22:05` through `09:23:18`, covering 4,201 symbols. It is suitable for
  same-window timing/coverage checks, but cannot reconstruct `2a10` assignment
  or an absolute `2704` baseline by itself.
- Per-flow delta-index parsing remains structural evidence until a fresh-slot
  baseline is captured at connection start. The 2026-09-04 cold start validates
  fresh-fallback at 5,171/5,171 records; trading-hours callback parity remains
  the publication gate.

## Account-class and route interpretation

Account and route observations must be interpreted together with the login window:

| Account/session | Observed route | Meaning |
|---|---|---|
| Test-168, 2026-09-01 | 5 entries, all `7709/7709` | Not a production 5188 baseline |
| Formal `1522`, active window (~21:55--22:10) | 10 `5188` flows observed | Formal account can receive official full-push |
| Formal `1522`, fresh login ~23:35 after close | 5 entries, all `7709/7709` | Route also depends on time/server/cache state |

Operational rule: use a formal account during an active trading window for 5188 acceptance. Use
`168` only for short, repeatable structural diagnostics. Record account class, login outcome, selected
endpoints, connection count, and time window independently. Do not infer a Rust decoder defect or
production coverage from a `168` `7709`-only session, and do not assume a formal account guarantees
5188 outside the active window. Avoid concurrent or repeated logins on the same account during a
capture because they can disturb the existing Wine session or server-side allocation.
