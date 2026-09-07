---
name: quote-netzip-rs-fullpull-replication
description: Reproduce quoteNetzipWine's authenticated official full-push market chain in Rust, keeping it separate from 7709 supplementation.
---

# quoteNetzipRs Official Full-Push Replication

## 1. Identity and boundary

- Repository: `/home/codes/stock/quoteNetzipRs`.
- Product target: reproduce the real `quoteNetzipWine` official full-push path in Rust.
- Authoritative crate: `/home/codes/stock/crates/netzip-fullpull`.
- Product integration: `quoteNetzipRs` service and quoteGateway adapter.
- Official full-push means: formal account authentication, vendor-selected non-7709 data
  connections, server-driven continuous delivery, Wine-equivalent decoding and callbacks.
- Current observed data endpoint: `5188`; authentication/control path includes `6100` and `7100`.
- `7709`, `0547`, K-line, F10, finance, code-table queries and worklist scanning are
  `netzip-supplement` or transition compatibility, never official full-push evidence.
- Read first: `docs/fullpull-replication-authority.md`, then
  `docs/capability-boundaries.md`, `docs/netzip-wine-replication-audit.md`,
  and `docs/gpt5.6_fullpull_supplement_boundary_audit.md`.
- Preserve unrelated dirty work. Compile/deploy/restart through the webClx queue.
- Credentials stay in protected runtime configuration; never print, commit, serialize or log them.
- Account policy: prefer the formal account for ordinary replication, live comparison and final
  acceptance. The test account identified as `168` is allowed only for repeated short-interval
  structural diagnostics of the same hypothesis (for example frame shape, control sequence, or
  bounded reconnect behavior). Test-168 is not an equivalent data source: it has been observed on
  7709-only routing, and its market values, coverage, subscriptions, or callback payloads may be
  missing, partial, stale, or wrong relative to formal-account entitlements. Record the account
  class (formal/test-168), time window and purpose in the evidence ledger, never the secret.
  Connection count, selected servers, market coverage and payloads may differ by account; any
  surprising or account-dependent result, and every data-value/coverage/latency conclusion, must be
  repeated with the formal account before changing shared behavior or claiming acceptance.
- `/compact` risk policy: context compaction is lossy and may discard exact fixture paths, hashes,
  byte offsets, command order, account/window context, failed hypotheses, and unresolved caveats.
  It is not evidence persistence. Before `/compact`, persist observations and next discriminating
  tests in the canonical ledger, owning forensic document, or this skill. After compaction, re-read
  those authoritative sources and raw fixtures before continuing. A compacted summary must never be
  cited as evidence, used to fill a decoder rule, or promoted across the acceptance gate.
- Project phase: **exploration and evidence-building**. The Rust implementation is not yet a proven
  replacement for Wine official full-push. Every new rule must carry confidence and evidence.
- Experience is part of the deliverable: after each meaningful probe, comparison or fix, persist the
  result in the owning `docs/forensics/`, `docs/EXPERIENCE.md`, audit document or this skill.
- Uncertainty is explicit. Use `needs-verification` for unknowns; do not convert a hypothesis into a
  default endpoint, decoder rule, fallback or public capability.
- Contradictions stop propagation. When code, docs, captures or runtime metrics disagree, register the
  conflict, identify the smallest discriminating test, run it before changing shared behavior, and
  record both the result and the rejected hypothesis.
- A successful webClx callback proves only the command recorded in its build
  log. A note naming a regression while the command is only `cargo build
  --release` is build evidence, not behavioral test evidence. Cite individual
  test results before promoting a protocol invariant.

## 2. Fast routing

| User wording | Owner | First inspection | Acceptance |
|---|---|---|---|
| 官方全推 / 真全推 / Wine 全推 | `netzip-fullpull` + service integration | `src/auth_7100_client.rs`, `official_5188.rs`, Wine `src/dll.rs` | authenticated 5188 session and callback parity |
| 5188 长连接 / 增量帧 | `netzip-fullpull::official_5188` | `Official5188Session`, `Official5188Reassembler` | frame boundary, reconnect and fixture tests |
| 5188 字段解码 / 2704 | `netzip-fullpull` | Wine captures and callback fixtures | same-symbol field-level comparison |
| 5188 抓包重放统计 | `src/debug_pcap.rs` | `POST /api/debug/pcap-5188-summary` | deduplicated stream/frame counts; payload remains opaque; reports numeric `kind` plus capture-order `wire_kind` |
| 认证后服务器选择 | `src/auth_7100_client.rs` and `src/auth_download.rs` | login result and `selected_quote_endpoint` | formal login plus selected non-7709 endpoint |
| 7709 / 0547 / 补数 | `netzip-supplement` and compatibility modules | `src/tdx7709.rs`, supplement routes | supplement tests; never mark full-push |
| 部署全推服务 | project deploy scripts + webClx | `deploy/*.service`, `scripts/install-service.sh` | service health and source metrics |

## 3. Feature map

Capability: authenticated control chain
User terms: login, 6100, 7100, account authentication
Authoritative owner: `src/auth_7100_client.rs`, `src/auth_download.rs`
Entry points: `login_auth_6100_with_verified_dictionary`, auth HTTP status/login routes
Flow: probe 6100/7100 -> formal 6100 login -> dictionary/download response -> parse 5188 list
Non-owners: 7709 token/bootstrap code
Focused verification: response roles `zstd_dictionary -> download_file -> zstd_dictionary`, login success, endpoint selection
Confidence: confirmed

Capability: official 5188 transport
User terms: 5188 session, official stream, server push
Authoritative owner: `/home/codes/stock/crates/netzip-fullpull/src/official_5188.rs`
Entry points: `Official5188Session`, `Official5188Reassembler`, `Official5188Handshake`
Flow: authenticated endpoint -> TCP stream -> 8-byte application frame -> direction classification -> raw payload
Non-owners: `NativeSession`, `tdx7709`, `0547` scheduler
Focused verification: split-TCP tests, metadata preservation, Wine frame-shape tests, reconnect behavior
Confidence: confirmed for framing; needs-verification for business decoding

Capability: Wine-equivalent realtime callback
User terms: OEM_REPORT, quote callback, full market realtime
Authoritative owner: target `netzip-fullpull` decoder plus quoteGateway integration
Flow: 5188 inner object -> decoded quote -> Wine-compatible canonical batch -> gateway
Non-owners: local `realtime.dat` OEM generator and 7709 polling publisher
Focused verification: same-session symbol/value/time comparison, coverage, freshness, reconnect and zero ingest failures
Confidence: needs-verification

## 4. Decision tree

1. Does the change start from a formal-account login and end in server-driven non-7709 delivery? Use this skill.
2. Does it actively query a symbol, range or 7709 endpoint? Route to supplementation.
3. Is the input a Wine callback/object fixture? Reconstruct the callback contract first; do not infer network fields from OEM layout alone.
4. Is a frame kind or payload not decoded? Preserve bytes and mark `needs-verification`; do not map it to 7709.
5. Is the service currently connected only to `:7709`? It is not official full-push, regardless of service name.

## 5. Change procedure

1. Reproduce with the smallest Wine fixture or sanitized capture.
2. Confirm the owner and boundary using `rg -n '5188|7100|callback|2704|official_5188' src docs ../quoteNetzipWine`.
3. Implement in `netzip-fullpull`; keep product orchestration in `src/bin/netzip_service.rs`.
4. Add a focused regression test before changing fallback or publication behavior.
5. Compare both directions: protocol/frame result and user-visible quoteGateway result.
6. Inspect `git diff --check`, preserve unrelated changes, then queue release verification through webClx.

### Exploration and contradiction protocol

1. Write the observation with source path, timestamp, endpoint/port, direction and fixture hash.
2. Separate facts, interpretations and open questions in the evidence document.
3. For every interpretation, name at least one falsifying observation or comparison.
4. If two sources conflict, do not average them or choose the newer-looking value. Reproduce both,
   isolate account/session/process/market/date differences, and add a focused regression fixture.
5. Promote a rule only after independent evidence agrees (for example, network frame plus Wine
   callback value plus Rust test). Otherwise retain raw bytes and `needs-verification`.
6. End each session by persisting what changed, what remains unknown, and the next discriminating test.

Minimal path:

```bash
rg -n '5188|7100|callback|2704|official_5188' src docs ../quoteNetzipWine
cargo +nightly test --manifest-path ../crates/netzip-fullpull/Cargo.toml
cargo +nightly metadata --no-deps
bash /home/root/.codex/skills/webclx-compile-and-deploy/scripts/request-webclx-compile-api.sh --note '验证官方全推复刻'
```

## 6. Verification matrix

- Authentication: formal login success, verified dictionary response roles, selected 5188 endpoint.
- Transport: metadata-preserving frame encode/decode, TCP segmentation, bounded payloads, reconnect.
- Initialization: Wine client frame shape and required post-login control sequence.
- Decoder: `0x2704` and bulk frame payloads compared to the same-symbol Wine callback values.
- Runtime: long-lived non-7709 sockets, continuous server delivery, quoteGateway freshness/coverage,
  `ingest_failures=0`, no silent 7709 fallback.
- Completion gate: do not claim official full-push replacement until authenticated 5188 initialization,
  decoder/callback parity, representative open-market coverage and recovery all pass.

## 7. Build and deploy

- Pure compile/check: use `webclx-compile-and-deploy` Mode 1.
- Deployment/restart: use the project deployment script through webClx Mode 2/3.
- Do not deploy the legacy 7709 `quote-netzip-rs-full-push.service` as proof of official full-push.
- Keep Wine online as comparison source until the replacement evidence gate passes.
- Verify `/health`, `/api/capabilities`, auth status, selected endpoint, source identity and gateway metrics after runtime changes.

## 8. Known traps

- A 7709 unsolicited frame is still supplementation, even when it looks push-like.
- `full-push`, `native push` and `push-worklist` are historical names, not protocol evidence.
- A connected 5188 socket without decoded server delivery is not full-push completion.
- 5188 kind uses little-endian wire order: raw bytes `0d04` decode to numeric
  `0x040d`. Keep both `wire_kind` (capture order) and `kind` (numeric value)
  in evidence so display formatting is not mistaken for a protocol constant.
- Protocol constants must use decoded numeric values (`0x0427`, `0x040d`,
  `0x0454`, `0x0421`, `0x043e`; client `0x1036`/`0x102d`/`0x102a`/`0x1007`).
- `tuwenca-codec` OEM encoders model DLL callback objects; they do not decode 5188 inner payloads.
- Historical capture IPs and old server lists are evidence only, never runtime defaults.
- Do not expose account, password, tokens, raw credential-bearing packets or unredacted login fixtures.

## 9. Maintenance

Refresh this skill when authentication sequence, 5188 frame kinds, decoder ownership, service routes or
acceptance metrics change. Every new frame kind must name its evidence fixture and focused test. Keep
unknown fields raw until independently matched to Wine callback output.

## 10. Stored replication experience

### Wine ABI and callback contract

- Vendor exports are `Start(StockAnswer)`, `Ask(PCWSTR, PVOID, int)` and `Stop()` with
  Windows `extern "system"` calling convention.
- The callback has exactly two arguments: `callback(PCWSTR form, PVOID answer)`.
- `askId` is inside the 200-byte `OEM_DATA_HEAD`; it is not a callback argument.
- `Ask` return value is success/failure, not response length. Read the bounded OEM header `len`.
- A synchronous `Ask` may return NUL-terminated UTF-16 JSON such as initialization status. Parse
  valid JSON before attempting OEM binary parsing.
- Callback memory is transient. Copy the complete packet before asynchronous dispatch.
- Production Wine PE32 requires the isolated Wine 11.13 `win32` prefix and vendor prelaunch;
  `Start(callback)` can return zero when the vendor process is not ready.

Evidence: `../quoteNetzipWine/README.md`, `../quoteNetzipWine/src/dll.rs`,
`docs/EXPERIENCE.md` ABI and callback sections.

### 5188 inner-object exploration (2026-09-01)

- Wine's formal-account 5188 topology is ten fixed initialized sockets, not
  "one socket per non-empty subscription partition". The 2026-09-04 cold-start
  capture shows `3610 x3 + 2d10 x3` on all ten sockets. Seven sockets add one
  `2a10` (six 1024-entry partitions and one 59-entry partition); the other
  three add no subscription but receive initialization objects and server
  heartbeats. The first subscribed slot alone adds the stable 12-byte `0710`
  between `2d10 x3` and `2a10`. Rust must retain all ten before market open.
  Never fill the last three with guessed or empty `2a10` frames.

- Initial account authentication uses the 19-field `认证/请求登录` client
  manifest with ordinary level-3 ZSTD. Require the 15-field `认证` response and
  `提示信息=登录成功`. The 12-field dictionary `加密包`
  (`大智慧C_登录包`) is a later, per-5188-connection initialization stage;
  never substitute it for initial login. The vendor also reuses the manifest
  shape for an approximately 100-second control heartbeat, but its dynamic
  fields and endpoint-distribution role remain `needs-verification` in this
  repository. A dictionary decode is usable only when the expected root
  matches; otherwise retry plain ZSTD to avoid silent dictionary pollution.
- Large zlib object kinds can exceed the outer u16 length. Observed `1504` and
  `0104` payloads carry `(uncompressed_len, compressed_len)` in their first
  eight bytes and zlib at offset 8; outer length equals
  `(8 + compressed_len) mod 65536`. Require both marker and modulo equality
  before using the extended length, or compressed tails become pseudo-frames.

- Recurring `3e04` payloads contain a fixed 12-byte raw header plus a 5120-byte
  body. Within a flow, the first header word (`block_offset`) advances by 5120
  while the other two words remain stable; `assemble_bulk_envelopes()` models
  this transport invariant and rejects gaps or metadata changes.
- Concatenating contiguous `3e04` bodies did not produce a complete standard
  zlib stream. Embedded `78 9c` markers are therefore only candidate bytes;
  `embedded_zlib_candidates()` reports a result only for a complete bounded
  stream and the real fixture produced no promoted candidate.
- `2704` samples commonly start with six bytes shaped like little-endian
  `(count, candidate_length, 0)` followed by variable bytes with frequent
  `0x81` markers. The candidate length is not consistent with total payload
  length, so no delta layout is accepted. Required next evidence is a
  same-symbol/same-timestamp comparison with Wine `OEM_REPORT`.

All three points are exploration evidence, not business decoding. Keep the
inner payload opaque until callback parity is demonstrated.

The vendor SDK archive `../quoteNetzipWine/StockAPI.full.rar` was also checked:
`StockC#/OemStock.cs` defines the outer `OEM_DATA_HEAD` and 500-byte
`OEM_REPORT` callback contract, but no 5188/`2704` wire decoder. Treat this as
confirmation of the target callback ABI only; it does not justify reusing the
OEM layout as a network parser.

For synchronized captures, use
`diagnostics/20260831-netzip-windows-vs-rust/analysis/build_callback_window_index.py`
to join pcap-summary flow time bounds with Wine callback `received_at` values.
Its output is a candidate-window index only; it must not be treated as
same-symbol parity until a symbol and business timestamp match is demonstrated.

### Provenance gate before promoting Wine callbacks

Before treating a Wine callback as official full-push evidence, query the
local host status and record `vendor_config.requested.login_primary`,
`vendor_config.effective.login_primary`, effective quote endpoints, and a
synchronized non-7709 capture. The `formal-primary-0006` cold-start fixture
requested primary login and contains authenticated 5188 initialization,
business frames, and overlapping callbacks, although the post-start effective
flag reads `login_primary=false`/backup. Therefore that flag is provenance
metadata, not a sole acceptance gate, until vendor rewrite semantics are
explained. Require direct non-7709 socket/frame evidence and retain this status
conflict as `needs-verification`; callback count or gateway success alone is
still insufficient.

The production integration boundary and acceptance gates are tracked in
`docs/codex/tasks/official-5188-production-wiring.md`; update that task before
changing the runtime lane or promoting a protocol hypothesis.

### Formal authentication and endpoint discovery

- Both 6100 and 7100 have evidence-backed formal-login roles. The 2026-08-06
  controlled run used 7100 for probe and 6100 for login, while historical and
  `vendor_pm` full-push sessions contain a long 7100 login/control flow. Treat
  endpoint role as session/config dependent; classify the response sequence
  instead of hard-coding “7100 probe only” or “6100 login only”.
- The verified login response roles are `zstd_dictionary -> download_file -> zstd_dictionary`.
- The downloaded server configuration selects non-7709 quote endpoints, currently 5188 in the
  real account capture. Do not require the historical 7709-only fixture to contain 5188.
- `Stock.字典` is a 1000-byte UTF-16LE raw-content ZSTD dictionary, not a standard binary zdict.
- Follow-up dictionary packets require the captured outer object template, raw-content dictionary,
  ZSTD level 3, dynamic request-number patching and updated lengths.
- Authentication success, endpoint discovery and data-session establishment are separate states;
  never report the first as proof of the latter.
- In `vendor_pm`, the selected long 7100 control connection performs a login
  request, downloads the server object, then emits a burst of per-connection
  requests whose responses embed complete `3610` frames. This control socket
  remains active with roughly 100-second request/response traffic. Do not close
  it after returning only an authentication summary.
- `Auth7100ControlSession` is the Rust owner for retaining that verified socket.
  The legacy `Auth7100LoginResult` path remains a summary-only compatibility API
  and closes its socket. Do not use the summary path to start official 5188.
- Ten Wine login-packet control requests share one 448-byte, 12-field shape.
  Among non-sensitive fingerprinted fields only `编号` varies per request (ten
  variants); the other comparable fields are stable. Sensitive identity fields
  have no fingerprint, so their equality remains unknown. Do not infer their
  values or copy them from a capture.
- Observed request numbering is login packets `2..11`, then two ABK numbers and
  two ACK numbers per pair of connections through ABK `28,29` and ACK `30,31`.
  Preserve this as evidence, not a constructor rule, until response-to-5188
  assignment is verified independently.
- The login-packet decoded root is `加密包` with header words
  `[12, 0, 0, 448, 448]` and twelve exact field spans.
  `build_candidate_official_5188_login_control_packet()` is a research-stage
  shape encoder only: callers supply current-session values, and the resident
  chain must not use it until field sourcing passes cross-session comparison.
- Only nine distinct control objects currently contain the stable
  67-byte-payload `3610`, although Wine sends that identical frame ten times.
  Keep caching/local construction/unidentified source as competing hypotheses;
  do not assign the tenth send to request 20 without new evidence.
- Triplet acceptance must use
  `Auth7100ControlSession::exchange_official_5188_init_triplet()`, which decodes
  exact control fields and binds `编号` to `应答编号`. Do not use generic
  embedded-frame scanning as runtime acceptance. Request
  `171957-18d115af25cb03fd` confirms stage/source/number/data validation and
  mismatched-number rejection.
- Correction: the triplet helper is offline compatibility, not the live
  orchestration API. Wine interleaves
  `3610(95)/3110(48)/3610(94)/3110(631|636)/3610(67)/3210(466|467)`.
  Use the separate login/ABK/ACK exchange methods and
  `Official5188Session::exchange_initialization_stage()` on the assigned data
  socket. Never collect all three `3610` frames before reading 5188.
- The live Rust ordering boundary is
  `Auth7100ControlSession::initialize_official_5188_interleaved()`. Its ACK
  builder runs only after both `3110` responses, and its post-frame builder
  runs only after `3210`. This proves ordering only: caller-supplied ACK and
  `2d10` derivations remain `needs-verification` and must not use captured
  runtime defaults.
- The second `3110` length partitions `vendor_pm` without exception:
  631 bytes maps to the four-connection `2d10` set and 636 bytes maps to the
  six-connection set. Keep this as correlation only until the 1379/1387 ACK
  data transformation or independent role rule is proven.
- ACK control `数据` has two observed variants: 1379 bytes on six connections
  and 1387 bytes on four. They map without exception to the two distinct
  `2d10 x3` sets. Treat this as a partition correlation, not a derivation rule;
  no `2d10` frame appears byte-for-byte in the control objects.
- Cross-session correction: `vendor_lunch` has the same CRC32 value for each
  same-length ACK `数据` template as `vendor_pm`, but its counts are reversed
  (`1379 x4 / 1387 x6` versus `1379 x6 / 1387 x4`). A fixed six/four rule is
  `rejected`; stable role/partition templates is `needs-verification`.
  `vendor_lunch` has no client 5188 initialization frames, so it cannot prove
  which `2d10` set follows either template.
- Server responses in both cold starts expose `数据` fields of 75, 102 and 103
  bytes, never a complete 1379/1387-byte ACK blob. Whole-field copying is
  `rejected`; embedding/transformation remains `needs-verification`.
- Every complete `vendor_pm` 5188 flow has two server wire-`3110` frames
  (numeric `0x1031`) and one wire-`3210` frame (numeric `0x1032`). Classify
  these as server initialization control, but keep payloads opaque.
- Staged `Stock.dat` `0x10038120` handles `0x1031`; its ACK branch copies the
  complete reassembled frame into session buffer `+0x6c261c`; the length
  accessor is `8 + declared payload length`, so the stored second-`3110`
  frames are 639/644 bytes. The only direct caller of
  `ack_string_pack_helper` later reads that same buffer. This rejects deriving
  ACK data inside the helper from the short `3610` response, but does not yet
  identify the transform producing 1379/1387 bytes.
- `ack_string_pack_helper` copies the stored payload but then uses C `strlen`
  before code-page conversion. The observed 631/636 variants have their first
  NUL at offsets 64/111 respectively, so only those prefixes feed `&ack=`.
  Converting the complete 631/636 payload is `rejected`.
- ACK `数据` has no NUL, `penc`, `hypenc`, or ZSTD marker. Both variants have
  exactly 86 CR/LF bytes; their eight-byte length difference is entirely
  visible ASCII. Both variants are confirmed pure ASCII with 44 lines, 43
  CRLF delimiters and a trailing empty line. Fixed binary and mixed-GBK
  interpretations are `rejected`; reconstruct the line grammar without
  exposing values.
- Staged official `Stock.dll`/`Stock.dat` own `Start/Ask/Stop`; `Stock.dat`
  carries `SFLogInPack=ABK`, `SSLogInRePack`, `&ack=`, `penc` and `hypenc`
  strings. Focus static analysis there, not in the Rust Wine adapter or
  `Stockdrv.dll`.
- Staged `大智慧/C8_Login_1036.dat` is a historical 101-byte `3610` frame
  (93-byte payload, metadata `[0,0,3,0]`). Current frames have 95-byte payloads;
  using the file as a live default is `rejected`.
- `2d10` payload structure is four raw little-endian u32 words plus sixteen
  zero bytes. Use `Official5188ClientSessionEnvelope`; do not assign business
  names to the words. The two sets differ in `word1` and the first frame's
  `word2`, and none of their complete word values appears raw in ACK data.
- `Official5188Session::send_wine_initialization()` must validate all three
  `2d10` envelopes before its first socket write. Keep the kind-only
  `Official5188Handshake` parser payload-agnostic; the transport boundary owns
  this stronger rule. WebClx request `165430-18d115af25cb03fc` confirms the
  malformed-tail zero-write regression and all shared-crate tests.
- The 14:56 fixture
  `captures/2026-09-01_full-session/official_full_session.pcapng` is not a
  second ACK/`2d10` session: its generated 7100 matrix has zero packets and
  zero payload bytes, while the companion timeline contains only localhost
  control sockets. That use is `rejected`. Keep the 1 GiB follow-up ETL as
  `needs-verification` until bounded conversion and endpoint inventory.

Evidence: `src/auth_7100_client.rs`, `src/auth_download.rs`,
`docs/forensics/7100-auth-20260806/`, `docs/codex/tasks/auth-7100-windows-forensics.md`.

### 5188 full-push model

- Wine opens multiple long-lived 5188 connections after login. Representative sessions last about
  3,355 seconds; the client sends only a small initialization sequence while the server sends
  thousands of frames.
- Confirmed client frame kinds include `0x3610`, `0x2d10`, `0x2a10` and optional `0x0710`.
- Confirmed server frame kinds include `0x2704`, `0x0d04`, `0x5404`, `0x2104` and `0x3e04`, with
  additional control/data kinds appearing on some sessions.
- Frame header metadata is meaningful. Preserve all 8 header bytes; do not replace bytes 4..7 with
  zeros. The little-endian payload length is at bytes 2..3 and must match the complete frame.
- `0x2704` deltas can arrive every few tens of milliseconds; bulk/state batches show longer gaps.
- Large payloads contain compression or a second object codec. TCP prefixes alone do not identify
  quote fields. Reassemble TCP first, then decode the inner object, then compare against the same
  symbol in Wine's public callback.
- `payload_hint` is an evidence-only prefilter for zstd/zlib magic. It does not prove the codec,
  does not decompress, and must never be used as a production quote decoder.
- `Official5188Session`, `Official5188Reassembler` and `Official5188Handshake` are the current
  Rust foundation. They are not completion evidence until the inner decoder and callback adapter pass.
- `Official5188Session::send_wine_initialization` is the only supported outbound fixture path;
  it validates the observed client-frame shape and requires caller-supplied current-session bytes.
- Never promote captured initialization bytes to runtime defaults. In ten complete formal-account
  flows, the 95-byte `3610` frame varied per connection, `2d10` formed two connection groups and
  every `2a10` subscription frame differed. Captures are shape fixtures only; live initialization
  must be derived from the current authentication result and connection assignment. The dynamic
  `3610` is now confirmed to arrive as a complete embedded frame in the preceding 7100 control
  response. Use `embedded_client_frames()` only to extract candidates; retain the control session,
  bind by current connection assignment and validate the ordered result with
  `Official5188Handshake`. Read `docs/forensics/official-5188-client-init-variance-20260901.md` and
  `docs/forensics/official-5188-control-frame-correlation-20260901.md` before changing outbound frames.

Evidence: `diagnostics/20260831-netzip-windows-vs-rust/analysis/vendor_pm_5188_detail.txt`,
`.../vendor_pm_5188_frames.txt`, `docs/glm5.3_netzip_rs_vs_wine_latency_forensics.md`,
`/home/codes/stock/crates/netzip-fullpull/src/official_5188.rs`.

### Wine initialization object sequence

- Dynamic Wine initialization emits callback objects for code tables, corporate actions, finance,
  file data, realtime data, empty realtime and control/status messages.
- Captured object catalog: `object_01/02` code tables, `object_03` split, `object_04` finance,
  `object_05` finance-v6 file, `object_09` realtime and `object_10` empty realtime.
- Objects `06..08` and `11..15` are control packets with repeated UTF-16 labels and dynamic fields;
  preserve their wrappers and do not invent missing dynamic values.
- Static outer-wrapper byte equality does not prove dynamic callback order or live-market behavior.

Evidence: `windows_debug/tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_init_full_2000_chunks/`,
`docs/forensics/wine-initialization-outer-objects-rust-validation-20260901.txt`.

### Realtime and OEM mapping lessons

- Wine `实时.dat` contains 6,211 occupied slots and a 5,206-byte internal slot layout; the file's
  first DWORD is a session token, not a fixed magic.
- The OEM realtime export contains 500-byte `OEM_REPORT` records. Empty/zero-time slots are handled
  differently by the raw disk view and public OEM view; do not reuse one filter for both.
- Price scale, compressed amount mode, tagged fixed-point values, `change`, `weiBi`, `liangBi`,
  `inVol`, classification and order-book direction are category-sensitive. Use the validated Wine
  getter rules and IEEE `f32` rounding, not generic decimal arithmetic.
- `tuwenca-codec` has validated encoders for Wine callback objects (`OEM_DATA_HEAD`, reports,
  market/stock info, split, finance and file). These encoders are not a 5188 network decoder.
- Normalize only proven uninitialized UTF-16 tails in fixture comparisons; never normalize business
  bytes or manufacture absent records.

Evidence: `docs/forensics/wine-realtime-dat-rust-validation-20260901.txt`,
`docs/forensics/oem-*-rust-validation-20260901.txt`, `crates/tuwenca-codec/tests/contract.rs`.

### Supplementation boundary and legacy traps

- 7709 `0x0547` supports real snapshot/unsolicited delivery, but it is still a separate query-
  supplementation protocol and is not equivalent to authenticated 5188 full-push.
- `NativeSession`, `netzipRust7709`, `push-worklist` and services containing `full-push` are legacy
  compatibility names. Their sockets, worker counts, renewal cadence and coverage cannot satisfy the
  official full-push gate.
- Never silently fall back from a failed 5188 session to 7709. Report the official chain as failed
  and supplementation as a separate capability.
- 7709 code tables, K-line, F10, finance, OEM mappings and query pagination belong to
  `netzip-supplement`, even while inherited source files remain physically in `netzip-fullpull`.

Evidence: `docs/capability-boundaries.md`, `docs/codex/tasks/netzip-rs-client-progress.md`,
`docs/glm5.3_netzip_rs_vs_wine_latency_forensics.md`.

### Performance and acceptance lessons

- Compare active symbols and identical event semantics. A stale last-trade timestamp on an inactive
  symbol is not transport latency.
- Wine's server-driven 5188 cadence must be compared with Rust's event arrival time, decode time,
  publish time, coverage and reconnect behavior. Do not attribute 20--45 second age values to parser
  cost without active-symbol evidence.
- Full replacement requires long-lived same-account behavior, authenticated non-7709 sockets,
  decoded callback parity, representative open-market coverage, competitive freshness, bounded file
  descriptors, zero gateway ingest failures and explicit recovery evidence.
- Keep Wine online as comparison source until every gate is green. A release build or established
  socket alone is not a replacement decision.

## 11. Evidence ledger

The canonical ledger is `docs/fullpull-replication-authority.md`; use
`docs/forensics/live-market-capture-inventory.md` to locate previously captured
live-market traffic and verify fixture hashes before replay.

When adding a protocol or decoder rule, record:

1. Source fixture path and SHA-256 (redacted captures where applicable).
2. Direction, endpoint port, frame/object kind and exact length fields.
3. Transformation or decode rule, including rounding and filtering behavior.
4. Same-symbol Wine callback comparison and mismatch count.
5. Focused test name and webClx compile request ID.
6. Runtime freshness, coverage, reconnect and gateway metrics when claiming live behavior.

Unmatched fields remain raw and are labeled `needs-verification`. Do not promote a hypothesis to a
default endpoint, fallback, public capability or deployment behavior.


### Wine primary disconnect root cause (2026-09-02)

- Wine 冷启动后「股票主站登录成功」约 3–70s 被断开并回落备用 7709 的真正根因是
  quoteNetzipWine `DllRuntime::initialize()` 的旧式同步
  `Ask(请求=登录&模块=认证&…&编号=0)` 与网际风配置自动登录冲突/超时返回 0；
  vendor 收到失败即断开股票主站并把 `用户/配置文件.ini` 写回 `登录股票主站=0`，
  之后不经 set-vendor-login-primary 直接 restart 只会连备用。
- 修复：`initialize()` 默认 `QUOTENETZIPWINE_INIT_LOGIN_MODULES=none`，跳过旧式登录 Ask，
  只发只读初始化 Ask；`run-host-wine.sh` 白名单透传；restart 前用
  `set-vendor-login-primary.py 配置文件.ini true` 显式保证主站登录。
- 判别证据：fixed-accept-2000（20:00:45 主站登录成功 → 10 条 58.16.134.228:5188 保持 ≥480s、
  login_primary=true/login_backup=false、无 Ask returned 0/无断开）；
  对照 ask-diagnose-1931（旧式 Ask 后 24ms 断开主站）。
- 进程管理禁止 `pkill -f '<exe串>'`（会误杀含该串的自身脚本树）；实验 env 需
  `systemctl set-environment` 或写入 /etc/quoteNetzipWine-wine-supervisor.env。


### Primary subscription partitions verified (2026-09-02)

- fixed-accept-2000 stable-primary session: five primary 1024-entry 2a10
  partitions are byte-for-byte equal to the Rust model built from each
  connection's own SH/SZ 0104 tables (SH 600-699 + SZ
  000/001/002/003/300/301, `0x10000 | ordinal`, table order, first 5120).
  `build_official_5188_primary_subscription_partitions` is confirmed correct.
- Two extra dynamic partitions exist beyond the model and are NOT derivable
  from the static 0104 tables alone:
  - P6: one connection sends 1024 mixed entries (SH578+SZ446) whose head
    starts at SH ordinal 0 (index codes like 000001) -- a non-primary set.
  - P7: one connection sends 59 SZ fund/ETF entries (159xxx band).
  They vary across lifecycles (58/59), so treat them as dynamic (likely
  account/config/self-select driven), not static table slices. Reconstruct
  their source before claiming full-market subscription coverage.
- Wine layout: ten primary connections are active; seven of them each send one
  2a10 partition (P1..P5 on five connections plus P6/P7 on two more). Each
  connection first receives its own SH/SZ/B$/SF 0104 code tables then sends
  its assigned 2a10; code-table JSON is emitted per connection by the extractor.



### 2026-09-03 open-market acceptance checklist (wake-up 09:15, focus 09:25 集合竞价 + 09:30)

Wine primary fix landed 2026-09-02 20:00 (fixed-accept-2000: ten
58.16.134.228:5188 ESTAB >= 480s, login_primary=true). Five primary 2a10
partitions are byte-for-byte verified; P6/P7 dynamic partitions and the
2704/3e04 business decoder still need open-market evidence. Wake 09:15 +08:00 (10 min before 09:25 集合竞价). Run in order; the
09:25 call-auction snapshot and 09:30 continuous session are the two data
moments the screener needs, so start the paired capture by ~09:20 at the
latest and keep it running through 09:35:

1. Health: `ss -nt 2>/dev/null | awk '$1=="ESTAB" && $5 ~ /:5188$/ && $5 !~ /127.0.0.1/ {c++} END{print c+0}'` expects ~10 ESTAB to the CURRENT L1 primary (observed 58.16.134.228 on 09-02, 222.85.139.177 on 09-03; endpoints rotate from the L1 list).
   `curl -fsS http://127.0.0.1:28787/api/v1/status` expects
   login_primary=true/login_backup=false. Config file
   `/home/codes/third_party/quoteNetzipWine/用户/配置文件.ini` must keep
   `登录股票主站 = 1`. Fix via
   `sudo -n python3 ../quoteNetzipWine/scripts/set-vendor-login-primary.py ... true`
   + `sudo -n systemctl restart quoteNetzipWine-wine-supervisor.service` before
   capture if needed (Wine login is config-driven now; never send the legacy
   `模块=认证` Ask -- initialize() defaults to `none`).
2. Paired capture for business parity: use
   `scripts/open-auction-baseline-capture.sh OUT_DIR 0924 0935` (waits until
   09:24, then tcpdumps 5188 and polls /api/v1/events with full JSON through
   09:35). This captures the first server objects and same-window callbacks
   needed to discriminate candidate baseline models. Current evidence does not
   prove whether the decoder baseline is a server reference object or a
   reconstructed client state. Do NOT rely on a mid-session capture by itself:
   it is all uses_baseline deltas and decodes wrong with a guessed baseline.
   Alternatively start tcpdump on tcp port 5188 AND poll
   `http://127.0.0.1:28787/api/v1/events` with full JSON (include
   packet/quote_batch, not just text) from ~09:20 through >= 09:35 so it covers the 09:25 call-auction
   snapshot AND the 09:30 continuous open (>= 300 s live).
   Name dir `diagnostics/20260902-live-pair/` style e.g.
   `diagnostics/20260903-live-pair/open-accept-<HHMM>`.
3. Extract: `target/release/examples/official_5188_extract pcap out/`; expect
   2704/3e04/bulk frames plus `0x0139` heartbeats interleaved. Wine quote
   batches at close are local OEM cache replays -- only use open-market
   same-window callbacks with same symbol+timestamp as decoder truth.
4. Callback parity gate (2026-09-03 lesson): a mid-session 2704 stream is
   ALL delta (uses_baseline=true) and CANNOT establish the baseline by itself;
   decoding deltas against a cross-frame pseudo-baseline yields silently WRONG
   values (600259->0.06 vs real 78.98). To validate: cold-start a FRESH 5188
   data connection at 09:25 so the capture includes each symbol's first
   first server objects plus the 0104 tables, keep >= 60-120s, then the
   mid-session deltas. Only then run
   `examples/official_5188_callback_parity.rs` and require same-symbol
   same-timestamp equality with the Wine `wjf.oem_report.v5` callback
   (price/volume/amount). Never publish the bogus records as parity.
5. P6/P7 experiment (dynamic partitions): while primary is live, record which
   local port sends which 2a10 partition and the 0104 tables on that port;
   compare two lifecycles. P6=1024 mixed SH578+SZ446 (930 non-primary head SH
   000xxx); P7=59 SZ 159xxx funds. Decide if they are fixed leftover slices or
   per-account dynamic sets.
6. End-to-end source check (Wine -> quoteGateway -> 选股):
   record the quoteScreener side during 09:20-09:45 (call-auction snapshot
   then continuous open) and confirm fresh symbols
   arrive with open-market datetimes. At minimum capture:
   - gateway source metrics for source quoteNetzipWine (16886 API or the
     gateway's source counters): received_batches/quotes growth, dropped,
     last_error;
   - stockScreener process (`/home/bin/stockScreener/stockScreener`) receiving
     updates for a few known active symbols (watch a liquid name) and its
     quote snapshot file / landing table growing;
   - Wine `/api/v1/status` gateway_forwarder attempted vs successful posts and
     any retried/dropped counters while open-market bursts are running.
   Acceptance: attempted==successful, dropped==0, no last_error growth, and
   selected symbols update at open cadence. This closes the "Wine can feed 选股"
   question, distinct from the Rust decoder parity gate in step 4.
7. Persist outcome to `docs/fullpull-replication-authority.md`,
   `docs/codex/tasks/official-5188-production-wiring.md`,
   `docs/forensics/`, and this skill before ending the run.



### netzip_win parallel-session status (2026-09-03)

- Parallel session (netzip_win, webclx s5228-s5298) already drives the real
  chain: formal account 6100/7100 auth -> L1 1880B -> 222.85.139.177:5188
  cold start -> full init (0104x4/2d10x3/2a10x5) -> 2704 frames
  (08:12 pcap under netzip_win/diagnostics/20260903-linux-login/).
- 2704 callback parity already at ~1.1% mismatch on 15:00 close data
  (amount 83->1013, price 355->1281, volume 377->1307 matches); remaining D
  bucket is the 15:00 auction batch. Baseline comes from Wine memory/
  实时.dat 311B records (load_core_baselines), NOT from a 09:25 absolute
  2704 frame (server keeps a persistent baseline; even cold-start first 0427
  is uses_baseline=true).
- quoteNetzipRs service (16893) authenticated=false means this process has not
  yet wired the verified netzip_win driver, not that replication is far off.
  Decide service-side production wiring next; avoid duplicating netzip_win work.


### netzip_win live initialization achieved (2026-09-03 ~10:15)

- Formal account 1522 live run now completes the whole official chain:
  auth probe -> login (4 servers) -> 222.85.139.177:5188 -> interleaved
  login/ABK/ACK init -> 0104 x4 -> post-init 8 frames (2d10 x3 + 2a10 x5)
  -> receives server frames -> DriverStatus::Connected.
- New protocol variant fixed: ABK 3110 first NUL = 532 at len 631 (historical
  samples only had 64). official_5188_ack_source_prefix: 631 => &[64, 532].
  ACK-DIAG eprintln in auth_7100.rs logs kind/len/first_nul on failure for
  future variants. 628 was a transient 631 misread, not a real length.
- Run recipe: NETZIP_RUNTIME_DIR=<wine runtime dir with 用户/>,
  NETZIP_TEST_ACCOUNT/PASSWORD from 用户/配置文件.ini (formal 1522),
  NETZIP_LIVE_ALLOW_FORMAL=1, NETZIP_NATIVE_5188_INIT=1,
  NETZIP_LOGIN_WAIT_CONNECTED_MS=8000; cargo build -p netzip-app then run
  target/release/netzip_win. WebClx compile ids 4021-4024.
- Next: keep the receive loop running (it stops after a few frames),
  wire 2704 value decode, resolve dynamic 2a10 sets 6/7, and only then wire a
  production service lane.


### netzip_win live receive loop sustained (2026-09-03 ~10:30)

- Formal 1522 live run keeps the initialized 5188 session and receives
  business frames continuously: 51s -> 349 server frames / 344 business
  frames (~6.8/s), no drop, then clean stop. Driver emits 10s liveness
  summaries (存活 Xs: 服务端帧 N 业务帧 M).
- Protocol fix: ABK 3110 payload length 628-636 and ACK 3210 464-468 drift
  per connection; they are NOT fixed tiers. official_5188_ack_source_prefix
  now treats the first NUL as the boundary (was a length-keyed offset table);
  ABK stage accepts 600..=680, ACK accepts 450..=480. 76 fullpull tests green.
- Driver now counts business frames (kind 0x0400..0x04ff/0x0130/0x013a/0x0139)
  and reports every 10s. Run recipe unchanged (see live initialization
  section). Next: decode 2704 values against the Wine core baseline using
  these sustained frames, then wire production lane.


### Live 2704 protocol parse verified (2026-09-03 ~10:30)

- Driver NETZIP_NATIVE_DUMP_DIR exports business-frame payloads; a 30s formal
  live run produced 199 x 2704 payloads. decode_official_5188_delta_indexes
  parses 199/199 (153-173 records/frame, realtime timestamps) via
  examples/live2704_probe.rs. Live 2704 = offline sample structure; wiring the
  Wine core baseline yields full quote values.


### Live 2704 baseline semantics disputed (2026-09-03 ~10:45)

- Cross-date Wine core baselines produce systematic drift, while cold-start
  captures observed only `uses_baseline=true` 2704 frames. These facts reject
  a cross-session current-state snapshot but do not identify the true baseline.
- Server reference object, initialization bulk object, and reconstructed
  same-session Wine state remain competing hypotheses. Resolve them with a
  from-connect capture plus same-time Wine callbacks; until then baseline
  ownership and initialization remain `needs-verification`.

## 12. Contradiction ledger

Maintain a contradiction entry whenever evidence disagrees:

```text
Conflict:
Sources:
Observed difference:
Possible causes:
Discriminating test:
Result:
Decision:
Persisted evidence:
```

Known examples that must remain visible until resolved:

- Historical 7709-only server-list fixtures versus fresh authenticated 5188 selection.
- 5188 frame prefixes that appear similar to 7709 framing but use a different session/protocol.
- Static Wine object byte equality versus unproven dynamic callback ordering.
- Connected socket/received frames versus actually decoded, same-symbol and gateway-published quotes.
- Raw `实时.dat` slot counts versus filtered public `OEM_REPORT` counts.

Never delete a contradiction after choosing a working interpretation. Close it with a dated test and
retain the rejected explanation so future agents do not recreate the same mistake.

Suggested invocation: `quote-netzip-rs-fullpull-replication 实现 5188 全推字段解码`

## 13. Latest static evidence boundary (2026-09-01)

### 13.1 Port-role evidence update

- A cold-start capture analyzed with the parameterized matrix confirms the
  observed 6100 pair is `auth_login` with `field44=12` dictionary envelopes
  and an inflated `认证` root. Do not treat this packet as market data.
- The full-session 7719 stream has stable legacy framing (`b1 cb 74 00`
  server 16-byte header; `0c 7a` client 10-byte header) and high byte volume,
  but its endpoint provenance and quote-field mapping remain
  `needs-verification`.
- Real 5188 replay currently proves complete frame transport and reassembly
  (including kinds `0x0427`, `0x0421`, `0x0454`, `0x040d`, `0x043e`), not
  business-field decoding. Keep runtime lane
  `pending-production-wiring / opaque-evidence-only` until callback parity is
  demonstrated.

- Wine `Stock.dat` ACK construction is a bounded text/object path: frame
  payload is copied using its declared length, formatted with the `&ack=`
  template, converted through a bounded `0x3a8` path, and dispatched via an
  instance method. This does not establish encryption, hashing, compression,
  or the 1379/1387 variant derivation.
- Wine subscription construction confirms `2a10` table length `10 + 6*N` and
  six-byte entries (two-byte prefix plus four opaque bytes). Keep the latter
  opaque until a same-session symbol mapping is proven.
- Persist these observations in
  `docs/forensics/wine-binary-symbol-audit-20260901.md` and summarize current
  status in `docs/fullpull-replication-authority.md`. New implementations must
  cite both locations and retain `needs-verification` labels for unresolved
  business semantics.
