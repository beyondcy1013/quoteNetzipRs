# NetzipRs Experience

## 2026-08-07 09:07 +08:00 - Gate resident authentication control and clear stale status

### Phenomenon And Root Cause

The new resident `POST /api/auth/login` route was reachable on the service's default LAN
`0.0.0.0` listener without an explicit enable switch. Any caller could therefore trigger another
real-account login and potentially disrupt an existing vendor session. A failed retry also left
the previous response roles, dictionary fingerprint, active-server count, and selected endpoints
visible in the status object.

### Change

- Require `NETZIP_AUTH_LOGIN_ENABLED=1` (also accepting explicit `true/yes/on`) at service startup;
  otherwise the endpoint returns `403` and performs no network operation.
- Clear all attempt-specific protocol and route fields when a new attempt enters `running`, while
  retaining only the current account/source metadata and timestamps.
- Expose `login_control_enabled` in the sanitized status so operators can distinguish disabled
  control from a missing password or a failed protocol attempt.
- Add unit tests for explicit opt-in, stale-state clearing, and password-free status serialization.

### Verification And Boundary

- The resident-auth contract tests passed 3/3, and the complete `netzip_service` binary test suite
  passed 45/45. No login endpoint was called and no credential was loaded for a network operation.
- A loopback HTTP smoke test was not run because the local execution policy rejected the command's
  mandatory background-process cleanup before startup; no service process was created.
- The control remains manual and opt-in. Startup login, automatic reconnect, and automatic 5188
  ingestion remain disabled pending dynamic follow-up and session-ownership evidence.

## 2026-08-07 08:37 +08:00 - Separate authentication success from 5188 and 7709 routing

### Phenomenon And Root Cause

The formal 6100 login path treated a missing 5188 endpoint as authentication failure even after
the response-role sequence and decoded `登录成功` marker had succeeded. This was too strong: the
fresh account-1522 response supplied 5188 routing, but the tracked historical download response
contains 10 active 7709 entries and no 5188. Authentication status and downstream server selection
are separate protocol facts.

The repository also contains 280-byte `plain_0118.bin` and `cipher_0118.bin` baseline files, but
they are not a trustworthy `Tdx_Encrypt` pair. Their capture scripts overwrite fixed filenames
without an event ID, the files were imported in the initial repository baseline, and neither file
matches the tracked 7709 TCP streams. Fixed-XOR and independent 8-byte-block checks also failed.

### Change

- Require a decoded success marker and at least one active downloaded server entry, not a fixed
  5188 port, for the narrow authenticated result.
- Preserve `selected_quote_endpoint` as the optional 5188 route and add
  `selected_7709_endpoint` as independent bootstrap state.
- Format IPv6 endpoint candidates with brackets while retaining existing IPv4 output.
- Add regression coverage for mixed 5188/7709 routing and the tracked 7709-only download sample.
- Do not implement or infer `Tdx_Encrypt` from the uncorrelated 0x118 files.

### Verification And Boundary

- `auth_7100_client::tests`: 8 passed; `auth_download::tests`: 5 passed.
- Full library binary: 97 passed and 1 failed. The only failure remains the Windows environment's
  missing `tcpdump` for pcapng conversion.
- No network login, hook, process injection, or production modification occurred.
- This saves the correct post-login route state but still does not generate the dynamic 7709
  bootstrap, prove a `Tdx_Encrypt` algorithm, or validate reconnect/failover behavior.

## 2026-08-07 06:35 +08:00 - Accept single-port 5188 entries after authenticated 6100 login

### Phenomenon And Root Cause

One authorized Rust account-1522 run completed the expected 6100 response-role sequence, decoded
the `Stock.字典` responses, and confirmed the decoded `登录成功` marker, but then returned
`decoded download response contained no 5188 quote endpoint`. The failure was generated upstream
in server-list extraction: `parse_server_entry` required exactly four comma-separated fields, and
`is_config_line` also required at least three commas. The installed vendor configuration contains
valid single-port rows in the form `name, host, port`, including a 5188 row, so the extractor
discarded the line before endpoint selection.

### Change

- Accept both `name, host, main_port, secondary_port` and `name, host, port` server rows.
- For a three-field row, use the single port for both `main_port` and `secondary_port`; preserve
  four-field behavior and reject all other field counts.
- Recognize two-comma rows as candidate configuration lines so the UTF-16 packet extractor retains
  them for the structured parser.
- Keep the authentication result limited to structured server entries; do not expose the decoded
  raw configuration, password, or opaque session values.

### Verification And Boundary

- `auth_download::tests`: 5 passed, including the installed three-field 5188 shape.
- `auth_7100_client::tests`: 6 passed, covering probe/login construction, response roles, and raw
  dictionary decoding.
- `cargo check --example auth_7100_login` and `cargo fmt --all -- --check` passed.
- Full library testing reached 95 passed and 1 failed; the only failure is the pre-existing
  `auth_7100_flow_matrix::tests::accepts_pcapng_capture_via_tcpdump_conversion` because this
  Windows host has no `tcpdump` executable.
- No second real login was performed. The parser fix is offline-verified but not live-retested.
- This proves the narrow 6100/7100 login and 5188 extraction path only. Dynamic follow-up fields,
  7709 `Tdx_Encrypt` bootstrap, reconnect/failover, and long-lived session equivalence remain open.

## 2026-08-06 - Bound gateway TCP client cache after CLOSE-WAIT exhaustion

### Phenomenon And Root Cause

The resident `netzip_service` reached its 1024-file-descriptor soft limit with roughly 922
`CLOSE-WAIT` sockets. Push shards and Beijing polling then reported `Too many open files`. The
gateway client cache key included the current request thread ID, so short-lived API/audit threads
created new persistent client entries that remained in the global cache after the threads exited.

### Change And Verification

- Key gateway TCP client slots only by publish lane and gateway address, preserving main/BJ/manual
  isolation while bounding the normal cache to one client per lane/address.
- Add a regression test proving the same lane/address maps to one slot across short-lived threads.
- webClx request `151034-18c8f2f31ead11b9` passed the focused transport tests; request
  `151138-18c8f2f31ead11bc` passed the workspace regression (148 tests) and authentication CLI build.
- Deployment must recheck fd count, `CLOSE-WAIT`, reader failures, gateway ACK failures, and BJ
  polling after the old process is replaced; existing leaked descriptors cannot be reclaimed in place.

## 2026-08-06 - Complete native Rust authentication for account 1522

### Phenomenon And Root Cause

Wine could authenticate account `1522`, while the Rust path only built the upper-layer Ask text
and probed TCP reachability. The real 7100 exchange is a three-stage object sequence. The initial
request is ordinary ZSTD, followed by two vendor dictionary envelopes. During reconstruction, the
outer metadata's compressed-length field was initially treated as a naturally aligned `u32` at
offset `148`; the successful packet stores it at the non-aligned offset `146`. Writing at `148`
overwrote a reserved zero field, so the server returned a normal ZSTD rejection instead of entering
the dictionary exchange.

### Change

- Add `auth_7100_client` to build the credential-bearing authentication object at runtime, wrap it
  in the verified `网络包 / penc / ZSTD` envelope, and drive the two follow-up messages.
- Parse the response roles as `zstd_dictionary / download_file / zstd_dictionary`; only that full
  sequence produces `authenticated=true` and `status=登录成功`.
- Add `auth_7100_login`, which reads the password only from `NETZIP_TDX_PASSWORD` and never returns
  or logs it.
- Keep account/password bytes out of source and tests. Tests use fixture credentials and assert the
  non-aligned length field plus the adjacent reserved field.

### Verification

- webClx request `135618-18c8f2f31ead11b1` passed the focused unit tests and built the CLI.
- The generated 850-byte inner object matched the successful Wine inner object byte-for-byte; the
  outer packet differed only at the incorrectly patched metadata field before the root fix.
- Three independent Rust TCP sessions authenticated account `1522`. The first returned packet
  lengths `441/1540/395`; the next two returned `440/1540/395`. All three parsed the same complete
  role sequence and returned `authenticated=true` with `status=登录成功`.
- This verifies native 7100 authentication only. It does not claim that the remaining local 2000
  bridge or all post-login business protocols have been replaced.

## 2026-08-06 - Route Rust panel authentication through production account 1522

### Phenomenon And Root Cause

The legacy panel connection path still normalized its account from persisted sample state,
which could silently submit the historical `168/168` credentials. The Rust 7100 login request
builder already existed, but credential selection was not centralized and the panel response
could serialize the password back to API clients.

### Change

- Normalize the Rust panel account to `1522`; `NETZIP_TDX_ACCOUNT` may override it for controlled
  staging, while old persisted accounts are ignored.
- Read the password from `NETZIP_TDX_PASSWORD` first, with the existing permission-controlled
  runtime state retained only as a compatibility fallback.
- Mark the panel password field `skip_serializing` so API responses do not return the secret.
- Keep the existing 7100 protocol probe as the authentication attempt and preserve an explicit
  `protocol_login_supported=false` result when the reply/decryption chain is incomplete.

### Verification

Offline unit tests cover the default account and password-source selection. Full compilation and
runtime authentication still require the production password to be injected by the service
environment; no password is stored in source, logs, or this document.

## 2026-07-31 - Drive native 0547 renewal from vendor response tokens

### Root Cause And Accepted Design

Wine-off packet capture proved that the original native sessions sent only the initial `0x0547`
request; continuing traffic came from the low-frequency audit poller. Wine uses `0x420c/0x2a02`
renewals carrying the per-security token returned at decoded record offset `+0x21`. Native now
records only confirmed response tokens, preserves the observed 800ms request and 2s response
eligibility gates, and schedules renewals across the 53 retained 100-symbol sessions. A shared
sliding one-second gate permits at most 160 renewal frames, bounding the response-driven traffic
without discarding unused capacity.

### Trading-Hours Evidence

- With Wine paused, a 247.5s session received 320,155 records with 6,079 renewals, zero reader
  failures/recoveries, and seven successful audits. Thirty fixed liquid symbols advanced
  continuously without the previous 20-30s global silence.
- After Wine was restored, a 12s capture measured Wine at about 139.5 requests/s. The earlier
  unconstrained native response-driven result of about 147 requests/s was therefore vendor-scale
  behavior rather than an intrinsic request storm.
- The accepted sliding-window build measured 140.5 native requests/s over 12s. Complete one-second
  buckets peaked at 159, below the 160 hard limit. Two complete resident sessions received about
  554k and 556k records with 35,096 and 35,559 renewals; both had zero reader, recovery, and audit
  failures.
- Final reset request `143124-18c742bcea915315` left exactly 53 native and 20 Wine connections.
  The native process used about 6.6% CPU, all native receive queues were empty, all three services
  were active, and the post-reset warning journal was empty. quoteGateway's ingest-failure counter
  is process-cumulative across the controlled restart experiments and must not be interpreted as
  a failure count for the final session; its current error fields were empty.
- The first final post-reset session completed in 252.5s with 545,680 received records, 32,164
  renewals, zero reader failures/recoveries, and seven successful audits. Eight cached audit-poll
  sockets accumulated unread unsolicited bytes between audits while all 53 push sockets remained
  drained; this identified a separate audit-connection lifecycle issue, not push-reader
  backpressure.
- The audit lifecycle was then fixed by making only push-internal audit polling sessions
  ephemeral; the public polling endpoint still returns healthy sessions to its cache. Deployment
  and reset requests `144220-18c74c92ca84d0f5` and `144313-18c74c92ca84d0f6` showed the same
  sequence across two audits: 53 resident push connections rose temporarily to 61, then returned
  through 60/55 to exactly 53 with a zero receive queue. No idle audit connection remained cached.

### Rejected Optimizations

- A globally smoothed 7ms slot reduced native traffic to about 56 requests/s because 53 blocking
  readers frequently missed slots; fixed-symbol freshness became worse. A sliding-window ceiling
  retains the hard bound without wasting capacity.
- A Wine-shaped 20-session layout, with two or three initial 100-symbol batches per connection,
  was tested with both 250ms and 100ms reader observation slices. The shorter slice restored about
  150 requests/s, but effective token throughput remained near 2,200 symbols/s and thirty-symbol
  freshness regressed to roughly 4-8s behind Wine, with a 15s outlier.
- Waiting for either 32 due tokens or 100ms increased payload size but reduced request frequency;
  effective throughput and user-visible freshness did not improve. The 20-session and coalescing
  changes were fully removed before the final regression and deployment.

## 2026-07-31 - Parallelize the full-market 7709 polling cycle

### Phenomenon And Root Cause

The resident publisher was called full push, but it polled 5,534 symbols as 53 primary and 5
fallback request batches in one serial loop. On 2026-07-30, 354 successful rounds had a 38.0s
median interval and 63.3s p90; the afternoon median was 49.3s. The service process was not CPU
bound. Latency accumulated while waiting for sequential 7709 request/reply batches, with
incomplete frames adding fresh-session retries.

### Change

- Distribute batches round-robin across four persistent 7709 sessions by default.
- Accept `worker_count` per request and `NETZIP_FULL_PUSH_WORKERS=1..16` from the resident runner.
- Keep each worker's batches serial and retain the existing three-attempt session-reopen boundary.
- Isolate cached sessions by stage, upstream, and worker index.
- Preserve source-time deduplication and primary-before-fallback routing.
- Report total/stage elapsed time, actual worker counts, and slowest batch durations.

### Verification

- TDD RED request `075927-18c71d0caddaf4e2` failed only because worker validation and batch
  sharding did not exist.
- GREEN request `080341-18c71d0caddaf4e4` passed the bounded-worker test after implementation.
- Full workspace request `080434-18c71d0caddaf4e5` passed 125 asserted tests with zero failures.
- Deployment request `080534-18c71d0caddaf4e7` installed release SHA-256
  `b196f7a308fe0031aa3dcdac1da879a507ab072dcf9fef62a0c16fbe36151565`; both resident services
  were active with zero restarts and the installed runner selected four workers by default.
- Runtime acceptance still requires an open-market comparison of `elapsed_ms` and source quote
  lag against the 2026-07-30 serial baseline.

## 2026-07-30 - Reproduce the OEM public cumulative-amount boundary

### Phenomenon And Root Cause

Raw Rust `0x0547` amounts differed from Wine public `OEM_REPORT.amount` in both directions, so a
fixed correction could not be valid. Production `网际风.exe` stores the raw amount at `0x483a9c`,
then `0x40aeb0 / 0x40acc0` apply a security-category-specific lossy `f32` encode/decode cycle
before `0x4a7890` exposes the public object. `0x40a130` owns the category grouping. The difference
therefore belongs to the public-object conversion boundary, not the wire parser or quoteGateway.

### Change

- Preserve `Tdx0547Record.amount` and `amount_raw` as raw diagnostics.
- At the CompactQuote boundary, reproduce the verified index and price-relative `f32` operations
  step by step. Apply them only to SH/SZ category ranges independently confirmed from production
  `实时.dat`; unverified categories retain the decoded value.
- Mark gateway rows as `netzip-rust-7709-0547.v3`. quoteGateway was deployed first and permits
  only an exact-datetime v2-to-v3 cache migration; ordinary stale/conflict behavior is unchanged.

### Evidence And Verification

- Wine literals for `SH510300`, `SH511010`, `SZ159919`, `SZ399001`, and `SH688001` all match
  bit-for-bit. A convertible-bond sample confirms the direct mode.
- Across the quoteGateway worklist, all `5181` Wine/Rust rows with identical datetime and volume
  matched the reconstructed formula; rows with different input snapshots were excluded.
- webClx requests `025042-18c6d0a033f1868a` and `025330-18c6d0a033f1868c` record the CompactQuote
  RED/GREEN cycle. Request `025606-18c6d0a033f1868e` passed the cross-category literals.
- quoteGateway request `025704-18c6d0a033f1868f` recorded migration RED; request
  `025823-18c6d0a033f18690` passed GREEN. Its 211-test regression passed in
  `030241-18c6d0a033f18695`, and deployment `030332-18c6d0a033f18696` completed before the
  NetzipRs producer protocol changed.
- NetzipRs full-workspace request `031200-18c6d0a033f1869c` passed formatting and 123 tests with
  zero failures. Deployment request `031306-18c6d0a033f1869d` then installed
  `/home/bin/netzip/netzip_service` SHA-256
  `0c4ac2f9dd4a6df37f99604436146251db03808213538619927f0476d6e1a79b`, after the
  quoteGateway v3 consumer was already online.
- A post-deploy worklist publication used quoteGateway's authoritative
  `required_quote_trade_date=2026-07-29`, published `SZ000001` and `SZ000002`, and was accepted as
  two updates with zero source stale rejections, conflicts, or ingest failures. The downstream
  cache retained `source_protocol=netzip-rust-7709-0547.v3` and public amounts `1705880320` and
  `782175744`, exactly matching the NetzipRs CompactQuote response.
- Repeating the same worklist publication in the resident process returned
  `published_count=0` and `unchanged_count=2`; no second gateway batch was sent. Both
  `netzip-rs.service` and `netzip-rs-full-push.service` remained active with `NRestarts=0`, the
  timer unit was absent, and the post-deploy warning journal was empty.

## 2026-07-28 - Separate raw 0547 time from OEM public quote time

### Phenomenon

Direct `0x0547` replies ended at `15:30:50` for Shanghai and `15:30:00` for Shenzhen,
while the Wine `Ask` public reports for the same instruments ended at `15:00:00`. Prices and
volume fields remained aligned, so treating the difference as another parser cursor problem would
have changed unrelated fields.

### Root Cause

The production `网际风.exe` local bridge caps its internal source time at `150000` before
converting `HHMMSS` to seconds and writing the public OEM object. The branch is at `0x4839a3` and
the conversion routine is at `0x438800` in SHA-256
`de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29`. Direct Rust access to
the same `7709` server retained the later raw time, proving that the behavior belongs to the local
public-object boundary rather than the upstream protocol.

### Change

- Keep `time_hhmmss_raw` and `extra0_time_hhmmss` unchanged for protocol diagnostics.
- Format public `datetime / quote_datetime` with the verified `15:00:00` upper bound.
- Apply the same public bound when `extra0` is the only available time hint.
- Mark corrected quoteGateway rows as `netzip-rust-7709-0547.v2`; quoteGateway accepts only the
  explicit v1-to-v2 cache migration and keeps ordinary stale rejection unchanged.
- Leave price, cumulative volume, current volume, and amount decoding untouched.

### Verification

The TDD boundary covers `153050`, `153000`, `150000`, `113200`, and an invalid value. A service
test verifies that `153050` becomes `2026-07-24 15:00:00` in the quoteGateway payload while the
record still contains raw `153050`; a separate regression preserves a diagnostic
`extra0_time_hhmmss=15:30:12` while exposing public `15:00:00`.

The final workspace regression, webClx request `194041-18c6694faaae503f`, passed 121 tests across
the parser, service, compatibility, and protocol crates. Deployment request
`194509-18c6694faaae5043` installed SHA-256
`bcfe12ef569a8ff97a24f9811b49ba85a8d1b69e149e5883de7549e0cd300da3` after quoteGateway's
v2 consumer had already been deployed. A closed-session worklist publication delivered 5,523
current quotes in 57 v2 TCP/MsgPack batches; direct and gateway samples agreed on
`SH600000=15:00:00/1020605`, `SZ000001=15:00:00/1061012`, and
`BJ920001=15:00:00/16888`. Both resident services remained active and the timer units remained
absent. Repeating the full worklist in the same resident process produced
`published_count=0` and `unchanged_count=5523`, so closed-session residency did not create
duplicate downstream batches.

## 2026-07-28 - Keep the Rust full-market publisher online without a timer

### Phenomenon

`netzip-rs-full-push.service` exited after each trading session and depended on a timer at
`09:14:50` and `12:59:50` to return. Removing only the timer would therefore leave the publisher
offline. Repeated snapshot polling could also send identical quote batches when upstream source
times had not advanced.

### Root Cause

The shell runner treated an out-of-session start as successful completion. The service had no
install target and used `Restart=on-failure`, so a normal exit was not restarted. The publisher
also had no source-side last-published timestamp cache; equal timestamps were filtered only after
reaching quoteGateway.

### Change

- Keep the runner alive outside trading sessions and sleep without calling quote or publish APIs.
- Install and enable `netzip-rs-full-push.service` directly with `Restart=always`.
- Remove the timer unit and clean any installed legacy timer during deployment.
- Publish only quotes whose `trade_date + quote_datetime` is newer than the last successful send.
- Report unchanged rows as `unchanged_count` and back off when a round has no new quotes.

### Verification

Verification evidence is recorded in the webClx build and install logs for the deployment that
introduced this change. Runtime acceptance requires the publisher service to remain active during
an idle window, the timer unit to be absent, and quoteGateway counters to stay unchanged when all
upstream quote times are unchanged.

## 2026-07-31 - Publish Beijing quotes through the resident native source

### Design

The vendor native `0x0547` subscription path covers the 5,204 Shanghai and Shenzhen worklist
symbols but does not provide equivalent Beijing coverage. Keep that proven 53-session path
unchanged and route only `BJ*` worklist symbols through the existing `7709` snapshot protocol.
The Beijing side uses four workers, batches at most 100 symbols per request, and repeats every
three seconds. It owns a request-local session cache, so the four connections are reused during a
resident push request and closed when that request ends. Failures are counted separately and do
not stop Shanghai or Shenzhen readers.

Both paths use the same source-time deduplication, authoritative worklist names, quoteGateway
batch schema, publication gate, and `netzipRust7709` source identity. The interval is configurable
with `NETZIP_NATIVE_BJ_POLL_INTERVAL_SECS` in the bounded range 1 through 30 seconds; activation
sets the conservative default of three seconds without replacing unrelated defaults.

### Verification

- Runner and activation contract tests passed in webClx request
  `175705-18c75322044ea3b6`; full workspace regression passed in
  `175720-18c75322044ea3b7`.
- Deployment `175744-18c75322044ea3ba` installed the implementation, and controlled reset
  `175850-18c75322044ea3bb` restored exactly one resident runner.
- Post-close acceptance `175931-18c75322044ea3bd` ran for 15 seconds against the full 5,535-symbol
  worklist. The unchanged Shanghai/Shenzhen topology reported 53 shards, 36,428 received records,
  zero reader failures, and zero recoveries.
- All 331 Beijing symbols completed three polling rounds with zero failures. The first round
  published 331 records; the following two rounds classified all 662 records as unchanged.
- quoteGateway returned `920000`, `920001`, `920008`, and `920009` with
  `feed_source=netzipRust7709`, `available_sources` containing `netzipRust7709`, and the existing
  `source_protocol=netzip-rust-7709-0547.v3`.
- After the acceptance request returned, `netzip_service` retained only its HTTP listener and one
  loopback gateway connection. No upstream `7709` connection remained, confirming that the local
  Beijing session cache was released at the request boundary.

## 2026-08-06 - Isolate and observe gateway ACK/send failures

### Phenomenon And Root Cause

The Rust publisher discarded TCP send/ACK errors before falling back to HTTP. A read timeout on the
fallback response then propagated through the main push loop and ended the active session, while the
Beijing poller could continue independently. The loops were separate, but both used one global TCP
client slot and lock, so transport state was not isolated. Gateway ingest failure counters could remain
zero even when the sender could not receive an ACK.

### Change

- Allocate gateway TCP clients by explicit publish lane (`main`, `bj`, `manual`).
- Record per-lane attempts, TCP successes/failures, HTTP fallbacks, final failures, consecutive
  failures, last transport, and last error; expose a snapshot in full-push responses.
- Preserve the exact error in logs and metrics, including the reason for HTTP fallback.
- Treat a failed main-market batch as a counted partial publish and continue consuming later batches;
  successful batches alone advance the last-published watermark.

### Acceptance

- Transport contract test proves main and BJ lanes use distinct client slots.
- TCP failure is visible with its lane and error, followed by an explicit HTTP fallback event.
- A failed batch does not advance the published watermark or terminate the rest of the active loop.

## 2026-08-06 - Isolate same-lane ACK ordering and bound ACK waits

Post-deployment logs showed that lane-level isolation alone was insufficient: concurrent workers
within one lane could still share one persistent ACK sequence, and the fixed 10-second read timeout
produced `EAGAIN` while quoteGateway was completing its ingest/broadcast path. TCP client slots now
include the worker thread identity, preventing same-lane workers from sharing sequence state. ACK read
timeout is configurable through `NETZIP_QUOTE_GATEWAY_TCP_ACK_TIMEOUT_SECS`, defaulting to 30 seconds
and clamped to 5-120 seconds. The full workspace regression passed with status 0.

## 2026-08-06 - Prove the real 1522 authentication split and ZSTD dictionary

### Phenomenon

Older notes treated port 7100 as the complete authentication exchange and treated `field44=12`
frames as an opaque or encrypted ZSTD-like shell. A fresh Windows capture was needed without
changing the Rust protocol or interrupting the resident production TdxW session.

### Root Cause

The vendor client separates authentication into two paths. It sends an identical `认证|测速`
request to 6100 and 7100, then opens a new 6100 flow for `认证|登录`. Standard ZSTD alone cannot
decode the later `field44=12` frames because they require the installed `Stock.字典` as raw-content
dictionary data.

### Evidence

- Real-account 7100 probe: 427-byte request, 345-byte response.
- Real-account 6100 login: 633-byte request; decompressed account field matches `1522`, while the
  password is retained only as offset 134 and declared length 12.
- `Stock.字典` SHA-256
  `8f44f49cbf10c8d203d9cabbda256da37c1f7d43e7f99b60c08d077c47b2bc68` decoded all 66 fresh
  `field44=12` samples with zero failures.
- The post-login process connected to `103.141.11.1:5188`; it did not open 7709/0547. Pre-existing
  production 7708/7719 traffic was excluded by process/timestamp correlation.
- Sanitized evidence and original block offsets are under
  `docs/forensics/7100-auth-20260806/`.

### Consequence

The next Rust step is no longer reverse-engineering an unknown dictionary. It is aligning raw
dictionary compression parameters and dynamic object templates, then separately proving the 7709
`Tdx_Encrypt` bootstrap. Do not infer a 7709 session token from this 5188 login path.
