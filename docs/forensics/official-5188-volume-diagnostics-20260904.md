# Official 5188 Volume Diagnostics (2026-09-04)

## Evidence

- Deployment: webClx request `102252-18d1fad994533cea`
- Install audit: `/home/bin/webclx/compile/runs/20260904T102257-1862763-13312/install-report-1.json`
- Runtime endpoint: `222.85.139.177:5188`
- Session: authenticated, initialized, ten slots, zero receive terminations
- Post-deploy sample: 3,828 attempted 2704 frames, 2,922 decoded, 906 failed

## Highest Diagnostic

The first retained `ladder_volumes:bitstream_exhausted` sample is:

```text
record 11 mask=0x02 header=0x0d stage=ladder_volumes bit=907:
volume_slot=7 volume_mask=0x3ff: bitstream exhausted; need 32, have 5
```

The corresponding token-prefix sample is:

```text
record 5 mask=0xbc header=0x05 stage=ladder_volumes bit=431:
volume_slot=0 volume_mask=0x3ff: token prefix did not match
```

## Interpretation

`0x3ff` requests all ten ladder volume slots, while the reader can have only a
few bits remaining. This is evidence of an upstream bitstream alignment or
token-table mismatch; it is not evidence that slot 7 is absent. The strict
decoder therefore continues to reject the frame rather than publish partial
state.

## Next Experiment

Compare the first failing frame's ladder layout, price-mask consumption, and
volume token table against a Wine callback-aligned frame. Any tolerant decoder
must remain diagnostic-only until full-frame parity is demonstrated.

## 2026-09-04 19:52 - Residue-Offset Failure Slice

The `ladder-trace-v4` failure list was joined to its matching manifest by
`payload_file`. Four of the 49 failed frames have
`decoded_value_residue_bit_offset=5`; the other 45 failures have offset zero.
This is a diagnostic slice, not a claim that all failures share one cause.

| payload | len / records | completed prefix | failure stage | failure bit | trace context |
|---|---:|---:|---|---:|---|
| `0033-1788485093356347-192-168-3-2-52896-2704.payload.bin` | 362 / 24 | 2005 | `ladder_volumes` | 2316 | record 21, mask `0xc0`, header `0x07`, layout `0x2d`, volume slot 7, mask `0x3ff`, 4 bits remaining |
| `0350-1788485096922504-192-168-3-2-52896-2704.payload.bin` | 176 / 9 | 861 | `ladder_volumes` | 1134 | record 8, mask `0xa0`, header `0x07`, layout `0x2d`, volume slot 6, mask `0x3ff`, 2 bits remaining |
| `0373-1788485097147951-192-168-3-2-52888-2704.payload.bin` | 177 / 12 | 1037 | `ohlc` | 1064 | record 10, mask `0x68`, header `0x1b`, 16 bits available for a 32-bit read |
| `0668-1788485099844840-192-168-3-2-57258-2704.payload.bin` | 94 / 5 | 285 | `ladder_volumes` | 554 | record 4, mask `0x76`, header `0x0f`, layout `0x2d`, volume slot 9, mask `0x3ff`, 6 bits remaining |

Payload prefixes and hashes are preserved by the source artifacts. The four
SHA-256 values are, in order, `00467b2e906e9ae01c62343a54dc8b1485eaeff4aa7013421fa30eca704b7730`,
`4379afc1fd5ef36dd59faf67e2fe01737e152532391b1521b88cdd81309b68a2`,
`40c077bae6d1fe776d7f546f89664aac294ac6dbc3adb2362429a2c2645c85bf`, and
`015cc6db559a94f644182f474a1a6eb9ca6f2b6b7db9187b2b3d193e55b514f9`.

Shape-matched clean controls are sparse: no clean frame has the same
`payload_len` and `delta_index_count` as `0033` or `0350`; `0373` has one clean
control (`0014-...-52888-2704.payload.bin`), and `0668` has three (`0127-...`,
`0615-...`, and `0770-...`). Therefore the current evidence supports only a
bounded prefix/bit-boundary comparison. It does not identify a replacement
token rule, prove cross-frame drift, or justify reading the Wine
`0x5b291c` mask unconditionally. The next useful fixture is a same-session
capture from login/session start with Wine reader offsets for these record
shapes.

An additional clean-trace scan confirms that residue alignment alone is not a
failure predicate. The `0033` shape (`mask=0xc0`, `header=0x07`, absolute)
occurs in nine clean records; three of those clean records also start at
`record_start % 8 == 5` and complete successfully (payloads `0476`, `0687`,
and `0870`, with layouts `0x28`, `0x2d`, and `0x2d`). The `0350`, `0373`, and
`0668` shapes have respectively one, two, and two clean records with the same
mask/header/mode, but none of those clean records starts at bit residue five.
This leaves payload-specific layout/token consumption as the unresolved
variable and reinforces the requirement for same-record Wine reader-offset
evidence.

The repeatable analyzer is
`scripts/analyze-official-5188-residue-controls.py`; its current output is
`diagnostics/20260904-live-rust/auction-0924/ladder-trace-v4-residue-controls.json`.
The script has a focused unittest at
`scripts/test_analyze_official_5188_residue_controls.py`.

## 2026-09-04 14:05-14:12 - Wine Tail Clamp A/B (Diagnostic Only)

- Compile request `140505-18d20876b433c7a5` passed the focused Wine-tail reader
  tests, strict trace equivalence, extractor policy test, workspace Clippy with
  warnings denied, and the release extractor build. The API is exported from
  `netzip-fullpull`; production callers remain on the strict path.
- The extractor now accepts an optional final policy argument. Omitting it (or
  passing `strict`) preserves the previous fail-closed behavior. `wine-tail`
  is explicit and is recorded in `seed-provenance.json`; it is not a runtime
  default or a publication switch.
- Replaying the same auction capture and baseline produced the following
  frame verdicts:

  | endpoint | policy | 2704 frames | clean | failed | decoded records |
  | --- | --- | ---: | ---: | ---: | ---: |
  | `222.85.139.177:5188` | strict | 1164 | 1115 | 49 | 15505 |
  | `222.85.139.177:5188` | wine-tail | 1164 | 1164 | 0 | 15587 |
  | `58.16.134.228:5188` | strict | 1239 | 1192 | 47 | n/a |
  | `58.16.134.228:5188` | wine-tail | 1239 | 1239 | 0 | n/a |

  The second endpoint is included as an independent frame-verdict check. Its
  decoded-record totals are not used for the callback comparison below, which
  uses the Rust endpoint's callback-aligned output.
- Rust callback parity at `+/-250 ms` changed from 7,002 matched records to
  7,009 and from 15,505 to 15,587 decoded records. Field hits changed only by
  price `+8`, amount `+11`, open `+11`, high `+10`, low `+7`, volume `+7`, and
  timestamp `+5`; five-level and ten-level book hits were unchanged. The
  additional records therefore do not establish a decoder or OEM parity gain.
- The result supports the Wine disassembly observation (bounded reads return
  zero at the declared stream tail) but does not prove that every clamped
  record is semantically valid. Keep the tolerant policy diagnostic-only until
  same-symbol, same-business-time callback comparisons show no fabricated
  values and the ladder/accumulator failure classes are independently resolved.

## 2026-09-04 Runtime Recheck After Diagnostic Build

- `GET /health` returned `{"ok":true,"service":"quoteNetzipRs","version":"0.1.0"}`.
- `GET /api/fullpull/official-5188/status` confirmed formal authentication,
  selected endpoint `222.85.139.177:5188`, ten initialized slots and zero
  receive terminations. Slots 0-6 carry `1024,1024,1024,1024,1024,1024,58`
  subscription entries; slots 7-9 carry zero entries and remain initialization
  only, matching the Wine topology.
- The primary running shadow reported `344024` attempted `2704` frames,
  `253200` complete, `82859` partial and `90824` failed, with
  `ladder_volumes` the largest error family (`19426` exhausted plus `3228`
  token-prefix mismatches). This is runtime evidence that transport and slot
  initialization are healthy while business decoding remains incomplete.
- The service still reports `business_decoder="opaque-evidence-only"`; no
  public quote publication or implicit 7709 substitution was enabled.

## 2026-09-04 Business-Timestamp Join Check

- Compile request `142245-18d20876b433c7a6` passed the explicit join-policy
  regression, workspace Clippy with warnings denied, and release parity build.
  The parity CLI now accepts `business-ts` in addition to the historical
  `nearest` mode. The report records the selected policy; the default remains
  `nearest` so prior evidence stays reproducible.
- On the same strict Rust extraction and callback fixture, `business-ts` at
  `+/-250 ms` matched `6692` of `15505` decoded records, with `8813` unmatched,
  `2214` carry-forward records and zero invalid records. Historical nearest
  matching produced `7002` matches. The lower count is expected because a
  wall-clock-nearest quote with a different business second is rejected rather
  than treated as parity.
- Business-timestamp field hits were: name `6688`, price `1135`, open `1510`,
  high `1383`, low `1281`, volume `1398`, amount `1631`, and timestamp `5735`.
  Five-level and ten-level book hits remained `59/43/52/45`, unchanged from
  nearest matching. This supports using business time as the primary key but
  does not solve the `586` same-code/same-second multi-candidate cases.
- The next join revision must add callback batch/state identity for those
  candidates. Do not globally widen the nearest window, and do not use the
  reduced match count as evidence against the business-time constraint.

## 2026-09-04 14:48-14:50 Close-Window Wine Evidence

- Without restarting either client, `scripts/capture-primary-callback-pair.sh`
  captured
  `diagnostics/20260904-live-pair/close-window-1447/wine-primary-sync-20260904T144812.pcap`.
  The file is non-empty (23,198,038 bytes; SHA-256
  `9db3013ab4541146a9583c9177855df5bb554d145e8cf467f437f99c2f50f830`) and
  contains 105,834 packets over `14:48:12.289260` to `14:50:11.733030`.
- The synchronized callback file is
  `callbacks/wine-events-primary-sync-20260904T144812.jsonl` (56,430 bytes;
  SHA-256 `41d86162c13a24955a4cc7b7386c4c2f32e161627cf72e2ea5a2e2b9c40acc31`).
  It contains 209 unique events, all connection-success `提示信息`; there
  were no `股票数据`/`quote_batch` OEM reports in this window.
- The status snapshot is
  `wine-status-primary-sync-20260904T144812.json` (680 bytes;
  SHA-256 `9dc315044612beb1f79e7f9988f78ede022395da41c5a7b094000e5fc4d6cb5a`).
  It records `login_primary=true`, `login_auth=true`, and the gateway
  forwarder counters `received_batches=0`, `received_quotes=0`,
  `attempted_posts=0`, `successful_posts=0`, `dropped_batches=0`.
- Replaying the pcap with the strict offline extractor produced
  `close-window-1447/extract-strict/manifest.json`: 30,867 application frames,
  including 28,825 `2704` frames, 25,022 clean and 3,803 failed. The Wine
  process and Rust process both had ten ESTAB 5188 sockets during capture.
  This is preserved as a short, bounded evidence window; absence of callbacks
  is recorded rather than inferred as zero market data.
- The complete frame census is retained at
  `close-window-1447/pcap-summary-full.txt`. It reports 53,104 unique packets
  (52,730 duplicate captures) and confirms the observed payload classes on the
  5188 flows. No fresh `2a10`/`0104` initialization object appeared in this
  post-login window; use the earlier cold-start fixtures for those artifacts.

## 2026-09-04 14:57-15:01 Close Boundary Evidence

### Offline failure-shape stratification

The reproducible report is
`diagnostics/20260904-live-pair/close-window-1457/failure-shapes-1458-1459.json`,
generated by `scripts/analyze-official-5188-failure-shapes.py` from the strict
manifest. It covers 18,048 `2704` frames from 14:58:00 through 15:00:00:
10,066 clean and 7,982 failed (44.2265%), with zero unparsed failure messages.
The dominant stages are `ladder_volumes` (1,967), `ohlc` (1,319),
`accumulators` (1,131), `ladder_values` (896), `record_mask` (796), and
`record_header` (594). The port-family failure rates are 35.8163% for 36xxx,
39.5927% for 42xxx, and 28.0942% for 43xxx, so this is not isolated to one
slot. Failure rises with `delta_index_count`: 2.0148% at one record,
25.4433% at two, 44.0% at six, 58.4%-58.8% at nine through twelve, and
68.35% at fifteen. This supports a multi-record bit-reader/state-transition
problem rather than an outer metadata or transport truncation.

The matching post-close report,
`diagnostics/20260904-live-pair/close-window-1457/failure-shapes-1500-1502.json`,
contains 1,199 frames: 1,146 clean and 53 failed (4.4204%). The remaining
failures are distributed across the same stages, but independent timestamp
inspection shows 45/53 (84.9%) contain only business timestamps before 15:00.
Therefore the post-close improvement is a mode/state transition effect and is
not evidence that the general `2704` decoder is fixed. The next discriminating
comparison is same `payload_len`/record count between pre-close failures and
post-close clean frames, using Wine memory indices to identify the baseline
state; no production decoder rule is promoted from this report.

### Exact-payload and shape cross-check

`scripts/compare-official-5188-payload-verdicts.py` produced
`diagnostics/20260904-live-pair/close-window-1457/payload-verdicts-close-boundary.json`.
There are 11,407 unique payload byte hashes before 15:00 and 851 after 15:00,
with zero exact hashes shared across the boundary. Within each window every
exact payload hash has a stable verdict (`pre_mixed_verdict_payloads=0`,
`post_mixed_verdict_payloads=0`); this rules out slot-local random verdicts for
the repeated bytes, but does not establish that length alone determines the
decoder branch. A coarser `(payload_len, delta_index_count)` comparison has
109 shared shapes. For example, the 18-byte/one-record shape is 6/176 failed
before the boundary and 0/4 failed after it. Shape matches therefore retain a
state/mode variable and are the appropriate input for token-trace comparison.

The shape report now carries per-shape failure stages. For the shared
18-byte/one-record shape, all six pre-close failures stop at `ladder_values`
while the four post-close frames are clean; for the shared 20-byte/one-record
shape, the four pre-close failures split between `ladder_values` and
`trailing_da`, while both post-close frames are clean. These examples further
support a mode-dependent branch/state difference rather than a deterministic
payload-length rejection.

The same report includes `failure_record_positions`. Failures are not confined
to the final record: the largest buckets are record 1 of two-record frames
(574), record 2 of three-record frames (361), and record 3 of four-record
frames (239), followed by interior records in larger frames. This is
consistent with an early branch/layout decision that changes subsequent reader
state, rather than a simple fixed tail truncation.

The paired Wine snapshots differ in only 39 of 184 mapped regions, totaling
1,036,685 changed bytes (about 0.35%). The largest changed regions begin at
`0x11500000`, `0x0a4f0000`, `0x09520000`, and `0x04b80000`. These offsets are
PID-specific snapshot coordinates, not protocol addresses. They are the next
offline scan targets for locating 311B/OEM public-state candidates; no address
is promoted into production decoding.

### Scope boundary: split.pwr compatibility

The local `除权V8.pwr` probe found 6,693 groups and 66,340 events, including
782 negative `give` values and no NaN/Inf. This is supplemental split/adjustment
evidence, not a `2704` decoder signal. Keep the current variant parser's
non-negative validation bounded to its proven fixture, and track strict 32-byte
header/reserved-zero checks, duplicate-symbol rejection, and empty-group
handling as separate acceptance work. Do not relax or promote those rules from
the live market diagnostics.

- The existing Wine process (`PID 3343519`, same start time) and its ten 5188
  connections were retained. No login, reconnect, restart or hook change was
  performed. The connection snapshot SHA-256 is identical before and after the
  capture (`1e53a1dcd1efd21d0f1e757dff542a8b242631a3ce47923b01f06d21d8ff6d09`).
- Raw pcap:
  `diagnostics/20260904-live-pair/close-window-1457/wine-primary-sync-20260904T145756.pcap`
  (13,621,048 bytes, 80,083 packets, SHA-256
  `27bebb43d0adf63ca8a41fadddcc739bfb466f1e651db25fec1c6fa2d69a605e`).
  Extractor timestamps span `14:57:56.719646` through `15:01:55.477549`, so
  the file covers both sides of the 15:00 close boundary.
- Strict extraction at `close-window-1457/extract-strict/manifest.json`
  contains 22,955 application frames: 19,688 `2704`, 1,369 `0d04`, 529
  `2104`, and 1,369 `5404`. Strict decoding completed 11,450 `2704` frames
  and rejected 8,238; this is preserved evidence, not a production acceptance.
- Synchronized callback JSONL is 55,350 bytes with 205 unique events, again all
  `提示信息`. It contains no OEM_REPORT/quote batch, so it cannot validate layer
  C. The Wine callback/forwarder defect is deferred to offline diagnosis after
  evidence preservation.
- Writable-private Wine memory snapshots were captured without stopping the
  process at 14:57 and after 15:01. Each contains 184 indexed regions and
  294,309,888 bytes. SHA-256 values are respectively
  `6c97f32bd6fd055f2cd25351896b98b53a0ad040b20e02ae75c1543c3d2d197b` and
  `9fd3c56d8f080107690eeb294d2c94e94efd2a326c06f2e2609dfa7f626fb816`;
  their identical index SHA-256 is
  `bb4ee4889a745b5f2a40f78e9abd9a71c06a580fef55b7eeec6bc6b70d009d88`.
  These paired snapshots retain the internal 311B/OEM public-state candidate
  memory for offline before/after-close comparison.

## 2026-09-04 Business-Timestamp Join Correction

- The first `business-ts` implementation incorrectly retained the `+/-250 ms`
  candidate slice and produced only `6692` matches. That report is superseded.
- Compile request `144219-18d20876b433c7a8` corrected the policy: candidates
  are now global for the same market, code and parsed callback business second;
  wall-clock distance is used only as the deterministic tie-break. Focused
  tests cover same-second candidates outside 250 ms, nearby wrong-second
  candidates, and same-second ambiguity.
- Replaying the same strict extraction produced `9184` matches, `6321`
  unmatched, `586` ambiguous records and zero invalid records. This agrees with
  the independent layer-split evidence (`9127` same-second records plus the
  known multi-candidate subset; small count differences come from the strict
  extraction fixture). The exact `586` ambiguity count is now exposed in the
  report as `ambiguous_business_timestamp_records`.
- Field hits for the corrected report are name `9175`, price `1538`, open
  `2071`, high `1922`, low `1768`, volume `1894`, amount `2215`, timestamp
  `7922`, and book arrays `75/60/67/60`. These are join diagnostics only; the
  remaining ambiguity must be resolved by callback batch/state identity before
  using the report to assess 311B-to-OEM projection parity.

## Record-Transactional Follow-up

Deployment `104333-18d1fad994533cee` enabled per-record baseline commits. The
post-restart sample reached 1,897 attempted frames, 1,417 decoded, and 480
failed, with zero receive terminations. The failure ratio remained close to
the pre-change rate, so frame-level baseline rollback was not the dominant
cause; subsequent work should prioritize field bit alignment and token-table
selection.

## 2026-09-04 10:45-11:08 - Deployment and Commit-v2 Verification

- Deployment `104333-18d1fad994533cee` completed successfully. Install audit:
  `/home/bin/webclx/compile/runs/20260904T104338-2075802-31135/install-report-1.json`.
  The audit reports only `/home/bin/netzip` modified; no files were missing or
  removed.
- Runtime is authenticated with formal account class `1522` via
  `121.41.70.217:7100`, selected quote endpoint `222.85.139.177:5188`, and no
  selected 7709 endpoint. It is initialized with 10/10 slots: slots 0-5 have
  1,024 subscriptions, slot 6 has 58, and slots 7-9 are initialization-only.
  `receive_terminations=0`.
- The primary shadow reader had 87,186 `2704` frames: 61,982 decoded and
  25,204 failed (28.91%). Dominant classes were
  `ladder_volumes:bitstream_exhausted` (5,204),
  `unknown:bitstream_exhausted` (4,172), and
  `ladder_values:bitstream_exhausted` (3,086). This indicates a decoder
  alignment or token-table gap, not a transport or ten-slot failure.
- Release tools from compile `104733-18d1fad994533cef` were replayed against
  `auction-5188.pcap` into `extract-rust-222-commit-v2` and
  `extract-wine-58-commit-v2`. Rust decoded 999/1,164 `2704` frames (165
  failures); Wine decoded 1,058/1,239 (181 failures). Pre-commit-v1 was
  978/1,164 and 1,034/1,239 respectively.
- Callback parity v4 reports are `parity-rust-222-commit-v4.json` and
  `parity-wine-58-commit-v4.json`. Rust: 14,298 decoded records, 6,832
  matched, 2,829 carried-forward; Wine: 14,142 decoded, 6,767 matched,
  2,825 carried-forward. Both have `invalid_records=0`; unmatched records
  remain substantial, so the runtime business decoder stays
  `opaque-evidence-only` and public 5188 publication is not promoted.

## 2026-09-04 11:38-11:47 - Record Boundary Diagnostics

- Compile request `113821-18d1fad994533cf5` passed the focused decoder and
  runtime classifier tests, workspace Clippy with warnings denied, and release
  builds for both extraction and callback-parity tools.
- The auction capture was replayed without overwriting earlier evidence into
  `extract-rust-222-commit-v3` and `extract-wine-58-commit-v3`. Results remain
  exactly 999/1,164 decoded for Rust and 1,058/1,239 for Wine. The unchanged
  totals confirm that this revision classifies errors without changing decoder
  semantics.
- Rust's 165 failures now split by baseline mode as 89 `missing_fresh`, 57
  `absolute`, and 19 `unknown`. The 19 pre-header failures split exactly into
  10 `record_header` and 9 `record_mask`; there is no residual unclassified
  stage. Wine's 181 failures split as 102 `missing_fresh`, 63 `absolute`, and
  16 `unknown`, with 6 `record_header` and 10 `record_mask`.
- The dominant Rust failure stage remains `ladder_volumes` (94), followed by
  `ohlc` (15), `trailing_da` (13), and `accumulators` (12). The dominant
  decoded mask/header pair remains `0x01/0x1d` (41 Rust failures and 51 Wine
  failures). Rust has 28 failures at record zero, so missing earlier state is
  a material amplifier, but the 57 absolute-mode failures prove it is not the
  sole protocol defect.
- Callback parity v5 reports are `parity-rust-222-commit-v5.json` and
  `parity-wine-58-commit-v5.json`. After removing the output-directory field,
  each is byte-for-byte equivalent to its v4 JSON report: Rust remains 14,298
  decoded / 6,832 matched / 2,829 carried forward, and Wine remains 14,142 /
  6,767 / 2,825. This is a diagnostic-only revision.
- Next decoder work should first align the `0x01/0x1d` ladder-volume path with
  Wine evidence. Do not skip the ladder for this special header and do not
  restore the previously rejected standalone volume-mask read without new
  byte-level evidence.

## 2026-09-04 12:03-12:07 - Partial Record Retention

- Compile request `120331-18d1fad994533cfa` passed the new partial-result
  regression, runtime tests, service serialization test, workspace Clippy with
  warnings denied, and release builds of the extractor and parity tools.
- The strict decoder still rejects the first invalid record and the legacy
  `Result` API remains fail-closed. A new diagnostic/runtime API returns only
  records completed before that error. It never returns the partially decoded
  failing record.
- Auction replay directories `extract-rust-222-partial-v1` and
  `extract-wine-58-partial-v1` preserve the same frame verdicts as commit-v3.
  Rust still has 999 clean and 165 failed frames, but 137 failed frames now
  expose 964 complete prefix records. Wine still has 1,058 clean and 181
  failed frames, with 148 failed frames exposing 1,091 complete prefix records.
- Rust parity `parity-rust-222-partial-v1.json` rises from 14,298 to 15,262
  decoded records and from 6,832 to 6,977 callback matches. Price, volume and
  amount hits rise by 84, 111 and 129 respectively; `invalid_records` remains
  zero. Wine parity rises from 14,142 to 15,233 decoded records and from 6,767
  to 6,999 matches. This removes avoidable prefix-record loss but does not
  establish business parity because unmatched records remain substantial.
- Deployment `120547-18d1fad994533cfb` completed successfully. Audit
  `/home/bin/webclx/compile/runs/20260904T120552-2852354-30003/install-report-1.json`
  reports only `/home/bin/netzip` modified, with no missing or removed paths.
  After restart, formal account class `1522` authenticated successfully, all
  ten 5188 slots were running with zero receive terminations, and the initial
  38/38 `2704` frames produced 5,165 records. The new runtime metric was visible
  as `decoder_partial_frames=0` during this clean cold-start window.

## 2026-09-04 12:26-12:45 - Stream Consumption and Baseline Mask Rejection

- Compile `122630-18d1fad994533cfc` added bounded value-stream consumption
  evidence to the offline extractor. On the fixed Rust-endpoint auction
  capture, with the stale Wine core used only to discriminate branches, the
  decoder accepted 1,115/1,164 `2704` frames and rejected 49. Clean frames
  commonly retain a large suffix: 1,098 have remaining bits and the maximum is
  23,848 bits. The independently matched 2,102-record fixture also retains a
  suffix, so complete value-stream consumption is not a valid acceptance gate.
- The 49 failed frames retain only 0-328 bits and fail within their final five
  records. This distribution remains evidence of cumulative reader drift, not
  evidence that the retained suffix is malformed.
- Disassembly of `wjf.exe` at `0x449bed..0x449c37` shows that a non-null
  baseline pointer calls ladder-move and then reads token table `0x5b291c`.
  Compile `123704-18d1fad994533cfd` verified an initial implementation of that
  literal control flow with 90 focused tests, workspace Clippy, and release
  diagnostic builds.
- Same-input replay rejected the unconditional production mapping: accepted
  frames fell from 1,115 to 1,091 and failures rose from 49 to 73. It repaired
  33 formerly failing frames but broke 57 formerly clean frames; relative-mode
  failures rose from 8 to 14 and two failures appeared directly at
  `ladder_volume_mask`. The experimental output is
  `extract-rust-222-baseline-volume-mask-v1`.
- Therefore the executable's non-null baseline pointer is not yet proven
  equivalent to Rust's current `baseline.is_some()` state on these replay
  fixtures, or another compensating control-flow detail is still missing. The
  unconditional `0x5b291c` read was reverted and was not deployed. Preserve
  the disassembly fact, but require a same-record Wine reader-offset/state
  comparison before reintroducing it.

## 2026-09-04 13:01-13:15 - Active-Market Aggregation and Branch Check

- During a 15-second active-market window, the ten authenticated slots received
  1,124 new `2704` frames: 893 decoded cleanly and 231 failed (20.55%). The
  dominant failure class remained `ladder_volumes`; no receive termination was
  observed. This confirms that the open defect is decoder-level rather than a
  missing transport slot.
- The runtime aggregation fix deployed by request
  `131132-18d1fad994533d08` now sums `decoder_partial_frames` across all slots.
  The post-deploy aggregate exactly matched the per-slot sums: 6,493 attempted,
  4,699 decoded, 1,623 partial, 1,794 failed, and 93,076 complete records. All
  ten slots remained authenticated and initialized with zero receive
  terminations.
- Compile `131452-18d1fad994533d09` replayed the 01:19 same-session cold-start
  fixture with the baseline-only `0x5b291c` mask experiment. Both variants
  decoded 38/38 frames and 5,171/5,171 records; their decoded JSON files have
  the same SHA-256
  `f696a9f62f1a9a7954b5012dc3fb0f63d195c7affeccdd98f3c2384d0bcebb95`,
  and all compared internal fields match Wine memory.
- That equality is branch-inert evidence: the fixture did not exercise the
  proposed baseline-only read. It neither validates the experiment nor
  contradicts the stale-core regression from 49 to 73 failures. The
  experimental read remains reverted. The next discriminating fixture must
  contain a repeated symbol that demonstrably enters the baseline branch and
  must capture Wine and Rust reader offsets around the ladder move and mask
  read for the same record.
- Release replay after compile `132751-18d1fad994533d0d` remained exactly
  1,115/1,164 clean frames with 49 failures, so the added diagnostics did not
  alter decoder verdicts. All 27 `ladder_volumes` failures in this stale-core
  branch-discrimination replay are `baseline_mode=absolute`: they skip
  ladder-move, report `baseline_ladder=none`, and therefore cannot test the
  baseline-only `0x5b291c` hypothesis. The next diagnostic revision records
  the raw ladder layout, flags, price mask, anchor token and record start bit
  to distinguish a wrong fresh volume-mask derivation from earlier cumulative
  reader drift.

## 2026-09-04 18:06 - Paired Wine Snapshot Candidate Scan

- The paired writable-private snapshots `wjf-memory-1457.bin` and
  `wjf-memory-1501.bin` are each 294,309,888 bytes with 184 indexed regions.
  Running `scripts/coldstart/scan-internal-records.py` with the same bounded
  timestamp window found 6,110 common `(market,symbol_index)` heuristic keys
  (6,110 before, 6,142 after); all 6,110 common raw candidates changed between
  snapshots, while 5,848 retained the same snapshot-file offset.
- Candidate concentration is not proof of slot identity: 5,830 before and
  5,912 after were in the region beginning at virtual `0x11500000`, with the
  remainder split across regions beginning `0x01010000` and `0x0eed2000`.
  The addresses are process-specific and must not become protocol constants.
- Field deltas among common candidates were timestamp 6,055, close 3,893,
  volume/amount 5,977 each, ladder prices 6,021 and ladder volumes 6,023;
  last-close did not change. This supports using the report for state-change
  triage only. It does not establish a payload-to-record join or justify a
  `2704` decoder change.
- Reproducible report:
  `diagnostics/20260904-live-pair/close-window-1457/memory-record-candidate-compare.json`.
