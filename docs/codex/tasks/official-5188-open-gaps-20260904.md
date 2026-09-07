# Official 5188 Open Gaps TODO (2026-09-04)

Scope: complete the authenticated official 5188 realtime path before publishing it to
quoteGateway or stockScreener. This list is a point-in-time execution queue derived from
session `01a05a9a-92c9-7813-9649-70d7483eaf0e` at 12:03 Asia/Shanghai.

Current verified baseline:

- Formal authentication succeeds and selects `222.85.139.177:5188`; no 7709 endpoint is selected.
- Ten data slots are initialized and receiving, with zero observed receive terminations.
- Cold-start baseline semantics are confirmed: `0104` supplies metadata only; the session's initial
  `2704` dump is decoded from a fresh/zero slot and becomes the baseline for later deltas. In the
  2026-09-04 01:19 same-session Wine memory comparison, 38 frames and 5,171/5,171 internal records
  matched with zero missing slots for timestamp, OHLC, volume, amount, last close, bid1 and ask1.
  H4 (`0104` as the business baseline) is rejected; H5 (fresh initial dump) is confirmed for this
  cold-start fixture.
- The public 5188 lane remains `business_decoder=opaque-evidence-only`.
- Current public production quote acquisition remains the explicitly labeled 7709 transition lane.

Latest evidence update (2026-09-04 21:22):

- Same-session `0104` is now a validated previous-close source, not a missing-data
  condition. The opaque tail at `[11..15]` has 99.92% SH and 99.70% SZ coverage
  in the extracted tables (99.96% in the independent spot check). The remaining
  seed gap is lifecycle provenance: a reconnect/day-cut must discard both old maps
  and rebuild `(market,index)->code` plus `(market,code)->full 0104 metadata` from
  the new session. A zero missing-seed count is insufficient because stale index
  maps silently produce wrong tickers and settlements.
- The 311B shadow path is still evidence-only. Night same-session replay is a
  positive control (`all_six=5,166/5,166`), while the auction wine-tail is not an
  OEM-live source (`all_six=0/5,039`; close+bid1 only 11 rows, amount 0/11).
  Persisted/recovered states must not be published as live quotes without an
  online callback join and state-group decision.
- P6/P7 replica construction is closed for offline and same-version cross-session
  checks; the only subscription gate left is a synchronized official/replica
  byte-for-byte `2a10` comparison with a formal account in the same trading
  window. Morning-versus-evening payloads are not a valid comparison because the
  `0104` receive universe can drift intraday.
- The stale-seed replay gate is now executable through the authoritative Rust
  `replay_2704_push` path. Against the 01:19 versus 19:57 extracts it reports
  1,441/1,848 index-to-code drifts and 1,798/1,848 stale previous-close values;
  this quantifies why reconnect/day-cut/restart must discard both maps. The
  split.pwr variant parser now rejects non-zero group reserved bytes and
  duplicate symbols, with regression coverage and a 891-group real-sample
  replay. Full protocol closure is still pending because the 32-byte filename
  header is intentionally treated as leading padding and empty groups remain an
  accepted fixture shape; document/validate those policies against another real
  payload before marking the supplement lane closed.

mustfix:

- [done] mustfix: finish and verify the partial-result decoder API - preserve complete
  records before a later record in the same `2704` frame fails, without returning the failed record
  or relaxing token validation. Compile `120331-18d1fad994533cfa` passed the focused regression,
  runtime/serialization tests, strict workspace Clippy, and release extractor/parity build. Replay
  recovered exactly 964 complete Rust prefix records from 137 failed frames and increased callback
  matches by 145 with zero invalid records. Deployment `120547-18d1fad994533cfb` exposed
  `decoder_partial_frames`; the post-restart formal session retained ten live slots and decoded its
  initial 38/38 frames into 5,165 records. In the 13:01 active-market window,
  a 15-second delta contained 1,124 frames (893 clean, 231 failed; 20.55% failed)
  with `ladder_volumes` still dominant and zero receive terminations. After the
  aggregation fix deployed by `131132-18d1fad994533d08`, aggregate counters
  exactly matched the ten per-slot sums: 6,493 attempted, 4,699 decoded, 1,623
  partial, 1,794 failed, and 93,076 complete records.

- [in_progress] mustfix: eliminate evidence-backed `2704` bitstream failures. The auction capture
  begins after session baselines, so its 89 Rust `missing_fresh` failures are not a reliable protocol
  defect by themselves. A branch-discrimination replay with a stale Wine core reduced Rust failures
  from 165 to 49 and Wine-endpoint failures from 181 to 47; the core is not valid for value parity,
  only for proving baseline presence. The remaining Rust failures are 39 absolute, 8 relative and 2
  pre-header; 27 are at `ladder_volumes`, followed by 7 accumulators and 6 OHLC. Acceptance:
  fixes must be tied to Wine/capture evidence, pass focused fixtures, and reduce both endpoint failure
  counts without weakening bounds checks or inventing token behavior. A literal baseline-only
  `0x5b291c` volume-mask read seen at Wine `0x449bed..0x449c37` was tested and rejected as an
  unconditional Rust mapping: on the fixed Rust auction replay it increased failures from 49 to 73,
  breaking 57 formerly clean frames while repairing 33. The experiment was not deployed. Resolve
  the Wine baseline-pointer/Rust resolver-state equivalence or the missing compensating branch with
  same-record reader-offset evidence before attempting this rule again.
  Compile `132751-18d1fad994533d0d` and fixed replay preserved the 1,115/1,164
  clean-frame verdict exactly. Its 27 `ladder_volumes` failures are all absolute
  mode with no baseline and no ladder-move consumption, so they do not exercise
  `0x5b291c`; current diagnosis has moved to the fresh layout/volume-mask path
  and earlier-record reader drift, with token validation unchanged.
  A separate strict short-tail selector was verified by webClx requests
  `141850-18d2e6663e8874d7` and `143654-18d2e6663e8874d9`: when fewer than 13
  value bits remain and every remaining index is baseline-only, the partial
  decoder now omits those indexes without synthesizing records or committing
  resolver state. On the complete initialization fixture this reclassified
  exactly 665 early `record_mask`/`record_header` partial frames as clean and
  exposed 1,399 omitted tail indexes. The remaining 3,946 frames with
  substantive decode errors, including 1,518 `ladder_volumes`, and 343 frames
  with no completed prefix are unchanged. Extractor manifests and shadow
  snapshots expose the omission count separately; it is not a decoded quote,
  semantic reject, or reason to close this gap.
  A Monday active-market replay of the isolated clone lane
  (`sync-run2-5188.pcap`, destination `192.168.3.2:51470`) under webClx request
  `144831-18d2e6663e8874dc` covered exactly 184 `2704` frames. The selector
  affected only 2 frames/2 indexes; the current partial/fresh replay reported
  149 frames without an error and 35 with substantive errors. Do not compare
  that 35 directly with the older 12-frame strict/Wine-tail recovery signature:
  those runs used different decode policies and resolver-state propagation.
  Reproduce both from the same binary and seed provenance before treating the
  delta as a regression or changing token/mask behavior.
  Focused fail-closed contracts passed under webClx request
  `145317-18d2e6663e8874de`: an empty baseline-only tail is omitted without a
  record, while the same empty non-baseline tail still fails at
  `stage=record_mask`; fullpull Clippy with warnings denied also passed.
  In the no-drop close-window capture, the frequently cited
  `mask=e8/header=03/absolute` bucket contains 257 failing frames but 5,414
  failing records (a frame can contain several failed records). Keep frame and
  record counts separate when measuring any decoder change.
  The next discriminating experiment is now defined: pair successful and
  failed records by `(mask, header, baseline_mode, ladder_layout,
  ladder_volume_mask)` and compare `ladder_values_bits`, volume-mask start,
  and remaining value bits from their trace/payload files. This is diagnostic
  only and must not alter token tables, mask semantics, or state initialization.

- [in_progress] mustfix: validate the separate `311B -> OEM_REPORT` projection and callback join. This
  is not the same defect as frames rejected by the bitstream decoder: partial-v1 has 15,262
  successfully decoded records, 6,977 nearest-window callback matches and 8,285 unmatched records.
  The tool joins to the nearest batch within `+/-250 ms`; timing ambiguity can select a different
  public-state version. A 250/500/1000/2000/5000 ms window ablation produced respectively
  6,977/8,138/10,367/13,036/14,436 matches and 8,262/7,101/4,872/2,203/803 records with no
  same-code callback in the window. This confirms that temporal joining dominates unmatched, but
  the internal business-timestamp agreement simultaneously fell from 95.5% to
  90.3%/78.9%/67.8%/63.1%; globally widening nearest-window matching therefore creates substantial
  wrong-state matches. The join must first constrain the same code plus business timestamp/state
  group, then widen only the transport-time search among those candidates or reconstruct the batch
  mapping. It also projects the 311-byte record's five levels per side into OEM's ten
  levels per side, fills levels 6-10 with zero, and then requires full-array equality. Therefore the
  current 35-41 order-book array hits must not guide token-table or `ladder_volumes` bitstream fixes.
  Low scalar callback hits must likewise be split into join, public-state merge, scale/conversion and
  internal decode; the night memory fixture already confirms internal volume/amount and bid1/ask1.
  Acceptance: prove same business-state selection without a global loose nearest-window match,
  compare the represented five levels separately from absent OEM levels 6-10, validate scalar
  conversion/merge semantics, and then demonstrate representative same-symbol/same-business-time
  OEM parity with zero invalid records. Compile `162519-18d2108898e7392d` now verifies the Rust
  projection's evidence-backed OEM f32 price/amount semantics and SH 688/689 `round(/100)` volume
  and five-level book-volume conversion; focused tests, all-target shared-crate Clippy, and the
  release parity build pass. Frozen same-session replay remains 5,028/5,166 (97.33%) full-record
  parity: price/OHLC/last-close/five-level prices and amount-within-1e-3 are 5,166/5,166, with
  residual failures limited to 97 book-volume lot boundaries, 39 datetime offsets over one second,
  and two cumulative-volume rows. Levels 6-10 are compatibility zeros only and are not a realtime
  decoder or layer-C blocking gate. Day-cut previous-close state and online business-second/state-
  group selection remain unimplemented, so this item is not done. Compile
  `163354-18d2108898e73930` adds and verifies `Official5188OemState`: callers keep one state per
  symbol, zero-valued sparse fields carry forward, `mask & 0x38 == 0x18` updates only timestamp,
  and previous close is injected explicitly from the day-cut snapshot. Format checks, focused state
  and projection tests, all-target Clippy, and release parity build pass. The API is not yet wired
  into production publication; this preserves `opaque-evidence-only` while runtime state-group
  ownership and reconnect/day-cut lifecycle are completed. Compile
  `164103-18d2108898e73934` wires the state machine into the receive-only shadow decoder and exposes
  only `decoder_oem_state_symbols`/`decoder_oem_state_updates` diagnostics. Nine runtime tests, the
  service aggregation and no-secret serialization tests, workspace all-target Clippy, and release
  build pass. No quote is emitted and `business_decoder` remains `opaque-evidence-only`.
  Compile `180134-18d2108898e73936` rechecked the conservation assertion
  `decoder_oem_state_updates == decoder_decoded_records`; the focused shadow-merge test, service
  Clippy and release `quoteNetzipRs` build all passed. This remains diagnostic-only and does not
  authorize public quote publication.

- [in_progress] mustfix: reconstruct P6/P7 dynamically from the current receive list and `0104` tables -
  (2026-09-04 zcode fixture independently reverified from the seven official `2a10` payloads and
  their SH/SZ code-table JSONs: official P6/P7 is one category-ordered sequence cut at the 1024
  boundary, not two independently ordered partitions. The fixture contains `6*1024+122=6266`
  entries; P1-P5 exactly equal the current `0104` eligible prefix. P6 starts with 94 SZ301 entries
  followed by one SZ302 entry, not 95 SZ301 entries. P6 ends at SZ value/index/code
  `67845/2309/159877`, and P7 continues at `67846/2310/159880`; P7 then contains 85 SZ159 entries
  followed by 38 SZ200/201 entries from `68305/2769/200011` through `68342/2806/201872`.
  All 6,266 values satisfy `0x10000 | symbol_index` and fit the SH 26,506/SZ 4,601 tables. The
  high-level category order is consistent with the changing 2026-09-02 fixture. Under the
  reproducible `market + three-digit code prefix + contiguous ordinal` grouping, P6+P7 has 150
  runs; the earlier "95 SZ301 entries" and "138 ranges" statements are withdrawn. Neither run
  count is an implementation constant.) -
  do not hard-code historical segment sizes or captured payloads. The hub now resolves each wire
  value through `wire_value & 0x7fff`, applies the category ordering learned from both changing
  fixtures, and re-splits the single leftover sequence at 1024. Fixture-B regressions cover the
  additional SH118/SZ395 debt, SH 5xx/75x/787 fund, SZ158/180/181 fund and SZ201 B-share families;
  direct tests cover `67845 -> 2309`, `92001 -> 26465`, and P6=1024/P7=122 boundary continuation.
  (2026-09-04 zcode FULL offline parity: the cap-wjf fixture-A leftover was expanded to its 1,147
  (market,index,code) entries via the capture's own 0104 tables and fed through the hub
  `order_official_p6_p7_leftover` — the output matches the official P6/P7 transmission order
  exactly, 1,147/1,147. Same-day capture comparison also confirms the pre-fix Rust order differed
  at the P6/P7 boundary and carried one duplicate entry, so this ordering is the live
  differentiator. Tooling: `scripts/extract-official-5188-partition-summary.py` (per-stream
  retransmit dedup), `scripts/verify-official-5188-partition-summary.py`, hub example
  `compare_partition_order`.)
  Acceptance still requires a current-day run with a stable formal-authorized account that reproduces official `2a10` entries
  byte-for-byte and confirms full subscribed-symbol `2704` coverage per subscribed slot. A same-day
  2026-09-04 comparison of official 6,203 entries (P7=59) with the 168 live capture (6,266,
  P7=122) found 6,031 set-overlap entries but only 1,538 positional matches; this is consistent
  with intraday `0104` universe drift (235 live-only and 172 official-only entries), not proof of
  an ordering defect. Therefore the byte-parity gate must compare official and replica captures
  from the same synchronized login window and account class; morning-vs-evening fixtures are
  diagnostic only. Wine keeps
  ten 5188 sockets but only seven send `2a10`; slots 8-10 must stay initialized without invented
  subscriptions unless new evidence proves otherwise. The audit tool now accepts the current
  extractor names (`*-0104.code-table.json` and `*-2a10.payload.bin`) and rejects malformed
  2a10 lengths/markets. Against the real 2026-09-02 callback-queue fixture it reports P1-P5
  exact equality to the 0104 eligible prefix, seven partitions sized `1024,1024,1024,1024,1024,
  1024,59`, and 6,203 total entries (`scripts/audit-official-5188-partition-alignment.py`,
  report `diagnostics/20260902-live-pair/callback-queue-fix-1722/reports/partition-alignment-current-
  extractor.json`). This is stronger fixture coverage but does not replace the required second
  changing official payload set or current-day online formal-account acceptance; status remains
  `in_progress`. The sibling implementation's `order_official_p6_p7_leftover` now reproduces
  fixture A's full 1,147-entry P6+P7 sequence entry-for-entry; the earlier duplicate/non-contiguous
  boundary is independently reproduced by that day's capture and fixed. Keep fixture A
  (`P6+P7=1,147`, total 6,267) distinct from fixture B/new-day (`P6+P7=1,146`, total 6,266): these
  sizes are observations, not protocol constants. Current-day formal-account byte parity and per-slot
  `2704` coverage remain the final gate. `168/168` is a free test account only: use it for short,
  single-session diagnostics and hypothesis checks, record it as `test-168`, and never treat its
  coverage, failures, or server responses as the production baseline. Results may vary with
  free-account time windows, permissions, connection limits, and other server-side rules; repeat
  account-dependent results with the formal account before changing shared behavior.

  (2026-09-04 evening `test-168` online structure evidence: login succeeded, four `0104` tables
  were received, and all seven `2a10` partitions arrived with sizes `1024*6+122=6266`.
  The extracted entries were unique and passed six boundary-continuity checks. This upgrades
  P6/P7 from offline-only evidence to a current-day test-account online structure pass, but it
  is not formal-account byte parity. The same short window observed 2704 clean/failed counts
  of `49/42` at 10 seconds and `55/48` at 24 seconds, so online delivery is proven while the
  decoder failure gate remains open. Evidence: `/home/codes/stock/netzip_win/diagnostics/live168-5188.pcapng`
  and `live168-2a10-summary.json`.)

- [done] mustfix: accept evidence-backed dynamic ACK response lengths instead of forcing 67 bytes.
  The server response scales with the client manifest: observed results include 44 bytes for a
  592-byte manifest, 63 for 1,103 bytes, 62 for current default rows, and 67 for a captured
  1,387-byte Wine file/CRC manifest. Rust accepts the bounded `40..=80` ACK-stage response, and the
  current 62-byte response completes all ten live initializations. A 67-byte response is not an
  independent parity target; captured manifest bytes must never be copied to manufacture it.

- [pending] mustfix: pass live-market stability and recovery acceptance - lunch-period `38/38`
  decoded frames and current zero terminations are useful health evidence but not active-market
  acceptance. Acceptance: formal-account open/live window with ten slots, complete coverage,
  callback parity, freshness, zero silent drops, bounded reconnect after socket/control interruption,
  and correct baseline reconstruction after reconnect/restart.
  (2026-09-04 zcode live-attempt evidence: the 7100 login response has a NEW server layout —
  header flag word at 28..32 is now 1 instead of zero, and the rejection object is 5 fields
  请求/来源/应答编号/错误/登录方式 with no 提示信息. The 168 free account is currently rejected with
  "交易时段（14:00-15:00），免费账号暂停登录" — including after 15:00; VIP7 was unreachable.
  `parse_control_object_with_root` now accepts flag {0,1} and `validate_real_login_response`
  surfaces the server's redacted reason. A 2026-09-04 diagnostic login captured the complementary
  success layout: flag word zero, a closed 620-byte `认证` object with 15 fields, and
  `提示信息=登录成功`; the field-value offset reader now counts UTF-16 code units
  (`label.chars().count()`) and performs bounded reads, fixing the prior UTF-8-byte-length
  misalignment (the erroneous `应答编号=131072` becomes zero). This closes the response-layout
  parsing TODO, but the capture used free test account 168/168 and is not formal-account
  stability evidence. Credential-bearing captures were deleted; only structural conclusions
  retained. See `diagnostics/20260904-live-168-evening/diag-login-success-20260904.txt`.)

- [contract-closed / live-gate-open] mustfix: publish official 5188 through the product path only after all earlier gates pass.
  The source identity contract is now present in quoteGateway source-model as
  `official_5188`, with canonical parsing/serialization and default-disabled
  payload metadata. It is excluded from `Merged` and `All`, has no real
  ingest/status route, and cannot implicitly fall back to native, 7709, or zero
  values. Source-model tests 13/13 and quoteGateway workspace check pass. Live
  publication remains blocked until strict 2704 business decoding, OEM callback
  parity, reconnect readiness, and formal same-window 2a10/slot coverage pass.
  Acceptance still requires explicit identity, coverage/freshness metrics,
  no implicit 7709 fallback, stockScreener retention, and rollback to the
  evidence-only lane.

ordinary:

- [closed 2026-09-04 zcode] `3f04` is NOT daily history: decompressed payloads are per-symbol
  GBK news/AH-comparison texts ("...H股股价，报价截至时间：20260902收盘，AH比价分析", 公告 titles);
  `3638` is the uncompressed variant (H股公告/大宗交易/权益分派). Full cap-wjf 5188 object census:
  0104 code tables, 1504 config files + split.pwr(除权) + bkcode, 3e04→stkinfo6.fin 财务, 1b04
  拼音索引, 2804 板块分类, 3001 指标公式, 3f04/3638 资讯, 2704 realtime. No daily bars on 5188;
  7100 decodes (official 字典) to autoupdate checks only; DAY.HQD untouched by 补日线 runs.
  Daily = local build from 2704 close snapshots + split.pwr. Evidence: windows_debug/progress.md 2026-09-04.
- [deferred] Continue 7709 quality or supplementation improvements. The user explicitly deprioritized
  7709 while official 5188 remains incomplete.

Evidence pointers:

- `diagnostics/20260904-live-rust/auction-0924/parity-rust-222-commit-v5.json`
- `diagnostics/20260904-live-rust/auction-0924/parity-wine-58-commit-v5.json`
- `diagnostics/20260904-live-rust/auction-0924/extract-rust-222-commit-v3/manifest.json`
- `diagnostics/20260904-live-rust/auction-0924/parity-rust-222-partial-v1.json`
- `diagnostics/20260904-live-rust/auction-0924/extract-rust-222-stale-core-diagnostic/manifest.json`
- `diagnostics/20260904-live-rust/auction-0924/extract-wine-58-stale-core-diagnostic/manifest.json`
- `diagnostics/20260904-live-rust/auction-0924/extract-rust-222-consumption-v1/manifest.json`
- `diagnostics/20260904-live-rust/auction-0924/extract-rust-222-baseline-volume-mask-v1/manifest.json`
- `docs/forensics/official-5188-volume-diagnostics-20260904.md`
- `docs/forensics/official-5188-layer-split-20260904.md`
  (parallel session: unmatched is mostly the ±250ms join; auction scalar
  hits are 0=0; OEM levels 6-10 are zero so ten-level padding is not the
  book-miss cause; ACK 62B is short-manifest scaling)
- `diagnostics/20260904-live-rust/auction-0924/join-by-business-ts.json`
  and layer-split section 12 (2026-09-04 14:05): exact business-second join
  matches 9127 vs 6977 at ±250ms; 276 wall-clock-only pairs have 0% timestamp
  agreement.
- `diagnostics/20260904-live-rust/auction-0924/replay-public-state-oem.json`
  and layer-split section 13 (2026-09-04 14:20): capture-order 0x44aa30-style
  merge + business-second state-groups. 586 multi-candidates are uniquely
  ordered but fingerprints never collapse. 555/1511 amount≠0 joins are still
  Wine pre-open zeros; the live 956 stay at 22/17/24 price/amount/volume, same
  as raw 311B. The earlier same-day-0104-last_close-missing conclusion is
  superseded by the same-session 0104 tail evidence below.
- `diagnostics/20260904-live-rust/auction-0924/last-close-seed-and-preopen.json`
  and layer-split section 14 (2026-09-04 15:40): **superseded by sections
  21-22**. The earlier conclusion that OEM last_close could use the previous
  session 311B `+0x10` was based on a pre-day-cut/night fixture and must not be
  used for next-day publication.
- `diagnostics/20260904-live-rust/auction-0924/live-914-conversion.json`
  and layer-split section 15 (2026-09-04 15:50): live 914 is 677 incomplete
  snapshots + 118 negative amounts + 96 missing auction prints; raw amount
  `i64 as f32` already hits when state matches (17). 7709 PriceRelative is
  not the 5188 OEM formula.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/night-oem-vs-311b.json`
  and layer-split section 16 (2026-09-04 16:00): same-session night OEM after
  the 2704 dump matches 5,166/5,166 price/OHLC/bid1/ask1 and last_close
  (`0x12b` overnight; `+0x10` is next-day last). Amount 99.09% within 1e-4;
  STAR volume `round(/100)`. Seq 16 July-24 OEM is the pre-dump negative
  control.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/night-oem-book-and-amount.json`
  and layer-split section 17 (2026-09-04 16:10): OEM levels 6-10 are zero;
  five-level prices 5,166/5,166; book volumes 98.12% with STAR lots; 47
  amount tails stay within 1e-3.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/oem-projection-recipe-v1.json`
  and layer-split section 18 (2026-09-04 16:15): frozen 311B→OEM recipe.
  Name 100%; datetime 99.25% within 1s (close-second floor). Do not use the
  mid-session auction pcap to disprove this recipe.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/oem-projection-parity-v1.json`
  and layer-split section 19 (2026-09-04 16:20): recipe projection vs OEM is
  5,028/5,166 (97.33%) full-record; remaining fails are 97 book lots, 39
  datetime>1s, 2 volume. Zero invalid records on this fixture.
- `diagnostics/20260904-live-rust/auction-0924/daycut-last-close-sources.json`
  and layer-split section 20 (2026-09-04 16:25): **superseded by sections
  21-22**. The night `+0x10` value is an overnight reference/last-trade value,
  not the next-day official settlement; do not use it as the day-cut seed.
- `diagnostics/20260904-live-rust/ten-slot-0909/morning-0104-last-close.json`
  and layer-split section 21 (2026-09-04 16:25): same-morning 0104 last
  matches auction callback last_close 6,183/6,183. Night 0104 is pre-day-cut.
  `to_public_quote` currently reads `0x12b` (overnight last); do not treat
  that as next-day last_close. Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/ten-slot-0909/morning-0104-vs-night-close.json`
  and layer-split section 22 (2026-09-04 16:30): the 14 night-close residuals
  are last-trade vs official 0104 settlement (603259 156.63 vs 156.12).
  Morning 0104 hits all 14. Night dump also misses 1,023 SH A-shares.
  Next-day last_close is login 0104 +11, not a +0x10 snapshot.

- [local-closed / online-evidence-open] mustfix: seed day-cut `last_close` from the same-day login `0104`
  table. The implementation gate is closed: cache the opaque-tail i32 at
  `+11` by `(market, code)` when login initialization completes, explicitly
  seed each `Official5188OemState`, keep missing-seed symbols evidence-only/
  not-ready, and construct a fresh decoder per authenticated slot so stale
  session metadata is discarded. The focused regression
  `shadow_decoder_reconstruction_clears_stale_session_metadata` passes 1/1;
  the missing-seed/not-ready and same-session previous-close tests also pass.
  The remaining acceptance item is online day-cut/reconnect evidence only:
  capture a real reconnect/new login and prove the old seed map is absent,
  same-session 0104 is rebuilt, and no-seed symbols remain not-ready. Do not
  reopen the implementation gate based on this evidence gap.

- Cross-session 0104 seed evidence (01:19 versus 19:57 independent logins)
  confirms that same-session 0104 is a valid last-close source while proving
  that the index and metadata maps must remain separate.  The opaque-tail
  i32 at `+11` is present for SH 26,489/26,510 (99.92%) and SZ 4,595/4,609
  (99.70%) in the two extracted tables; an independent Tencent spot check
  reports 99.96% coverage for the same-session table.  By `(market,index)`,
  the code drifts between sessions for SH 23,906/26,506 (90.19%) and SZ
  3,097/4,601 (67.31%).  By `(market,code)`, decimals remain stable (SH
  26,502/26,502; SZ 4,600/4,600) and names are ≥99.96% stable, while
  previous-close changes reflect the real session/day boundary (SH
  22,689/26,502 equal; SZ 273/4,600 equal).  Therefore a reconnect/day-cut
  must discard the old `(market,index)->code` map and rebuild it from that
  session's 0104, then build `(market,code)->full metadata` including scale,
  name, and `opaque_tail[11..15]`; a zero missing-seed count is not evidence
  of correctness when a stale index map can silently poison the code.  This
  supersedes the old statement that same-day 0104 lacks last-close data, but
  does not close the still-open online lifecycle/reconnect gate.  Evidence:
  `diagnostics/20260904-live-168-evening/cross-session-seed-evidence.json`,
  `diagnostics/20260904-live-168-evening/tencent-lc.json`, and
  `/home/codes/stock/netzip_win/progress.MD`.
  Capture-order OemState merge (layer-split section 31) only lifts live
  price 46→65; leftover-only seconds are not merge targets. Evidence:
  `diagnostics/20260904-live-rust/auction-0924/oemstate-merge-then-join.json`.
  Cross-session index injection of the 09:15 0104 tables onto this extract
  hits last_close 2,929/9,184 (31.9%) versus 9,184/9,184 by code
  (layer-split section 32). Reconnect must rebuild the index map from the
  new 0104. Evidence:
  `diagnostics/20260904-live-rust/auction-0924/last-close-key-index-vs-code.json`.
  Layer-split section 33: do not globally zero trade fields at 09:25.
  All 5,039 live joins are ≥09:25; 300 leftover rows remain after 09:25.
  Evidence: `diagnostics/20260904-live-rust/auction-0924/preopen-vs-first-print.json`.
  Layer-split section 34: 2,796 leftover OEM joins already have a nonzero
  2704 close. First-print is public OEM live, not internal close≠0.
  Evidence: `diagnostics/20260904-live-rust/auction-0924/leftover-2704-close.json`.
  Layer-split section 35: those nonzero closes track overnight 0x12b (86%
  within 5%), not morning 0104 (49 exact). Evidence:
  `diagnostics/20260904-live-rust/auction-0924/leftover-nz-close-source.json`.
  Layer-split section 36: 3,023 codes with leftover and live joins show
  only 17 residue→OEM close transitions; 952 stay on 0x12b after OEM live.
  Evidence:
  `diagnostics/20260904-live-rust/auction-0924/leftover-to-live-close.json`.
- `diagnostics/20260904-live-rust/auction-0924/disambiguate-586-batch-identity.json`
  and layer-split section 23 (2026-09-04 18:40): the 586 conflicts are
  leftover×print pairs in the same business second (828 groups, all size 2,
  last_close identical, night fixture 0). Prefer later/non-zero quote. Shadow
  now injects the morning 0104 seed when the matching table is available;
  missing seeds remain counted and evidence-only.
  Parallel session did not edit official_5188.rs or the parity example.
- `diagnostics/20260904-live-rust/auction-0924/score-586-prefer-live.json`
  and layer-split section 24 (2026-09-04 18:45): all 586 are 1 leftover + 1
  live; later sequence == live 586/586; nearest picks leftover 290/586 and
  its 143 price hits are 0=0. last_close vs morning 0104 is 586/586.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/ablate-star-book-lots.json`
  and layer-split section 25 (2026-09-04 18:50): STAR lots are half-up plus a
  1-lot floor, not banker's f32 round. STAR book 615/615; volume 5,166/5,166;
  full-record 5,105/5,166. Remaining 22 book misses are non-STAR 0 vs 1.
  `to_public_quote` still uses `(f32/100).round()`; parallel session did not
  edit official_5188.rs.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/zero-vs-one-book.json`
  and layer-split section 26 (2026-09-04 18:55): the 22 non-STAR book misses
  are empty size at a non-zero price, displayed as 1 lot. Book volumes
  5,166/5,166 after that fill. Remaining night full-record gap is 39 close
  seconds. Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/datetime-overshoot.json`
  and layer-split section 27 (2026-09-04 19:00): all 39 datetime>1s misses
  are SZ 399xxx close-second overshoots. Flooring after 15:00:00 yields
  5,166/5,166 datetime. Combined night full-record parity is 5,166/5,166.
  Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/unmatched-no-same-second.json`
  and layer-split section 29 (2026-09-04 19:10): 6,297 no-same-second rows
  on the wine-tail extract. 498 never in this callback capture; 5,799 at
  another second (3,929 nearest ±1s). Unique one-sided ±1s is leftover-only
  on 2,172 rows. Exact business second remains the join key. Parallel
  session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/joined-auction-fields.json`
  and layer-split section 30 (2026-09-04 19:15): 9,184 exact-second max-seq
  joins. leftover 4,145 and live 5,039 last_close 100% from morning 0104.
  Live price 46/5,039 (1,975 internal close=0 vs OEM print; 3,018 nonzero
  mismatch). Mid-session wine-tail is not same-session OEM state. Parallel
  session did not edit official_5188.rs or runtime.
- `diagnostics/20260904-live-rust/auction-0924/oemstate-merge-then-join.json`
  and layer-split section 31 (2026-09-04 19:20): capture-order sparse merge
  lifts live price 46→65. 1,157/1,975 close=0 rows fill from extract history
  but only 19 then match; leftover hits 1,349→1,126. Parallel session did
  not edit official_5188.rs or runtime.
- `diagnostics/20260904-live-rust/auction-0924/last-close-key-index-vs-code.json`
  and layer-split section 32 (2026-09-04 19:25): 09-03 vs 09:15 index maps
  move 29,086 codes (10.8% stable). Morning 0104 by code is 9,184/9,184;
  by index onto this wine-tail is 2,929/9,184. Offline seed map stays
  market+code. Parallel session did not edit official_5188.rs or runtime.
- `diagnostics/20260904-live-rust/auction-0924/preopen-vs-first-print.json`
  and layer-split section 33 (2026-09-04 19:30): all 5,039 live joins are
  at/after 09:25; all 3,845 09:24 joins are leftover. 300 leftover remain
  after 09:25 (175 waiting for that code's first print, 125 never live).
  Trade fields stay 0 until the symbol's first live print, not a wall 09:25
  gate. Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/leftover-2704-close.json`
  and layer-split section 34 (2026-09-04 19:35): 2,796/4,145 leftover OEM
  joins have nonzero incoming 2704 close. Do not open trade fields on
  first nonzero 2704. Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/leftover-nz-close-source.json`
  and layer-split section 35 (2026-09-04 19:40): 2,796 leftover nonzero
  +0x10 equals morning 0104 on 49 rows and stays within 5% of live 0x12b
  on 2,401 (86%). Overnight residue, not today's print. Parallel session
  did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/leftover-to-live-close.json`
  and layer-split section 36 (2026-09-04 19:45): 3,023 codes with both
  leftover and live joins. Residue→OEM close transition 17; 952 stay on
  0x12b after OEM live. First-print is not visible as a 2704 slot update
  on this wine-tail. Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/close-eq-12b-as-leftover.json`
  and layer-split section 37 (2026-09-04 19:50): treating nonzero +0x10
  within 5% of live `0x12b` as leftover false-suppresses 2,055/5,039 live
  OEM rows (40.8%) while only catching 2,401/4,145 leftover rows. Do not
  use this as a first-print or leftover gate. Parallel session did not
  edit official_5188.rs.
- Layer-split section 38 (2026-09-04 19:50) is a historical read-only snapshot
  and is superseded for seed behavior by compile `194918-18d2108898e73949`:
  STAR cumulative and book volumes use integer half-up plus a 1-lot floor,
  non-STAR empty-size→1 is in, and missing same-day seeds remain not-ready
  rather than `unwrap_or(0)`. The remaining lifecycle gaps are the online
  day-cut/reconnect path and the 15:00 timestamp projection rule.
- Layer-split sections 43-44 and `datetime-string-floor.json` establish a
  separate public projection rule: any wire timestamp strictly after 15:00:00
  China Standard Time is reported as that day's 15:00:00 across SH/SZ/BJ and
  all security classes. Compile `200319-18d2108898e7394a` adds
  `Official5188InternalRecord::public_timestamp` and focused pre/post-close
  tests. The internal 311-byte timestamp remains unchanged; this is an OEM
  projection rule and has no effect on the 2704 parser.
- The old service test assertion `decoder_oem_state_updates ==
  decoder_decoded_records` was invalid after no-seed records became
  evidence-only. The accounting invariant is now
  `decoder_decoded_records == decoder_oem_state_updates +
  decoder_missing_previous_close_seeds`; the focused aggregate test was
  updated accordingly. This metric remains diagnostic and is not a release
  gate until online seed lifecycle coverage exists.
  Compile `200623-18d2108898e7394d` re-ran nightly fmt, the aggregate test with
  a nonzero skipped case (`decoded=150`, `updates=142`, `missing=8`), workspace
  Clippy with `-D warnings`, and release; all passed. This closes the local
  accounting regression only, not the online lifecycle gate.
- `diagnostics/20260904-live-rust/auction-0924/2704-live-predictors.json`
  and layer-split section 39 (2026-09-04 19:55): decoded close/volume/amount
  /open/bid1/mask cannot separate leftover from live (precision ~0.44–0.54).
  1,975 live OEM rows have 2704 close=0. Wall ≥09:25 is 5,039/5,039 live
  with 300 leftover false positives (section 33). Parallel session did not
  edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/join-1936-special-case.json`
  and layer-split section 40 (2026-09-04 19:39): read-only on callback_parity
  mtime 19:36. The leftover×live size-2 special case equals always-max-sequence
  on all 9,184 exact-second joins (8,598 unique + 586 special; 0 other shapes).
  It repairs the 290 nearest leftover false picks. OemState.project already
  writes previous_close onto 0x12b before to_public_quote. Parallel session
  did not edit official_5188.rs or the parity example.
- `diagnostics/20260904-live-rust/auction-0924/stale-index-seed-poison.json`
  and layer-split section 41 (2026-09-04 19:42): the wine-tail extract has 0
  same-session 0104 tables and 6,162 unique indexes. Reusing 09:15 0104 by
  index is missing on 0 keys (no `unwrap_or(0)` / missing-seed counter) and
  maps 4,590/6,162 (74.5%) to another code, all with a different last_close
  i32. Runtime never calls `OemState::project`. Parallel session did not
  edit official_5188.rs or runtime.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/night-same-session-index-datetime.json`
  and layer-split section 42 (2026-09-04 19:45): same-session 0104 keyed by
  (market, index) is 5,166/5,166 code identity and last_close vs OEM.
  Overnight 0x12b equals 0104 +11. Crate raw timestamp is exact on 4,341/5,166;
  825 need the 15:00:00 floor; after floor exact is 5,166/5,166 with 0 false
  floors. Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/not-ready-vs-stale-full.json`
  and layer-split section 43 (2026-09-04 19:49): read-only on runtime 19:48.
  Missing (market, index) seeds now skip OemState::new(0). Empty tables skip
  all 15,505 wine-tail records. Stale 09:15 0104 still merges 15,505/15,505
  with 12,437 wrong-code last_close (80.2%) and missing-seed counter 0.
  Auction timestamps are all 09:xx (0 close-second floors). Parallel session
  did not edit runtime.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/datetime-string-floor.json`
  and layer-split section 44 (2026-09-04 19:51): flooring the unix u32 when
  Asia/Shanghai is after 15:00:00 makes OEM datetime strings 5,166/5,166
  exact (raw 4,341). 825 u32s change; only 39 are SZ399. Do not special-case
  399. Service tests still assert oem_state_updates == decoded_records.
  Parallel session did not edit official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/project-needs-0104-row.json`
  and layer-split section 45 (2026-09-04 20:01): runtime still stores only
  previous_close i32 and never calls `OemState::project`. Same-row 0104
  scale+name+i32 last_close is 9,184/9,184 (unique OEM 6,183/6,183; night
  seq≥21 6,189/6,189). Hardcoded scale=100 is 8,626/9,184 and misses 854
  `places=3` funds/bonds. `to_public_quote` on merged OemState.record is
  0/9,184; on resolver-after-2704 is 106/9,184. Do not copy 0x12b in merge.
  Parallel session did not edit official_5188.rs or runtime.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/crate-public-timestamp.json`
  and layer-split section 46 (2026-09-04 20:03): read-only on crate 20:02
  `public_timestamp()`. Night probe∩seq≥21 OEM datetime/unix join is
  5,166/5,166 after the UTC+8 15:00:00 clamp (raw wire 4,341); 0 disagreements
  with section 44. Auction would floor 0/15,481. Parity example still joins
  wire `index.timestamp` — 825 close-dump misses if reused at 15:00:01.
  Parallel session did not edit official_5188.rs or the parity example.
- `diagnostics/20260904-live-rust/auction-0924/resolver-overwrite-wipes-0104.json`
  and layer-split section 47 (2026-09-04 20:06): night same-session 311B keeps
  0104 code/tail 5,171/5,171. Auction last-2704 0xe5 matches 09-03 identity
  on 1,841/6,162 indexes. Exact-second joins stay 9,184/9,184 via 09-03
  index→code; wire 0xe5 equals OEM on only 3,197/9,184. Name is not in 311B.
  Parallel session did not edit official_5188.rs or the parity example.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2010-two-step-seed.json`
  and layer-split section 48 (2026-09-04 20:10): read-only on runtime 20:10
  `(market, index)→code` then `(market, code)→full 0104 row`. Same-login 301
  tables have 0 conflicts. Stale 09:15 two-step last_close on the wine-tail
  join set is 2,929/9,184 (missing=0), same as the old index key; 09-03
  identity plus morning-by-code is 9,184/9,184. Runtime still does not call
  `project_from_code_table_metadata`. Parallel session did not edit runtime.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2014-metadata-seed.json`
  and layer-split section 49 (2026-09-04 20:14): read-only on runtime 20:14
  `from_code_table_metadata` construction. Skipping project() no longer yields
  last_close=0. Projecting with a stale index→code map would publish the wrong
  ticker on 6,260/9,184 joins (3,499 unique indexes); code+name+last via 09-03
  identity plus morning-by-code stay 9,184/9,184. Parallel session did not
  edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/0xe5-seed-map-gate.json`
  and layer-split section 50 (2026-09-04 20:18): read-only on runtime 20:18
  private `reseed_code_tables` (unit test only; no ShadowReader API).
  Same-session night 0xe5==seed-map is 5,169/5,171 with 0 false rejects.
  Stale 09:15 on the wine-tail rejects 6,267/9,184 but still passes 40
  residual wrong tickers. Not a join key. Parallel session did not edit
  runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-project-gate.json`
  and layer-split section 51 (2026-09-04 20:23): read-only on runtime 20:23
  `project_from_code_table_metadata` as an `is_err` gate. The returned
  PublicQuote is discarded; ShadowSnapshot still has no quotes. Night
  same-session is 5,169/5,171 schema-Ok with 0 wrong tickers. Stale 09:15
  on the wine-tail is 8,952/9,184 schema-Ok and still 6,098 wrong tickers
  (3,414 unique indexes). The 232 schema failures are all negative 2704
  amount; they merge then fail project with no rollback. `project().is_err`
  is not a same-session identity gate. Parallel session did not edit
  runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-dirty-merge-persist.json`
  and layer-split section 52 (2026-09-04 20:32): read-only replay of the
  20:23 merge order on the full wine-tail. 15,505 decoded = 14,960 updates
  + 545 missing. The 545 post-merge project errors persist in oem_states:
  158 later zero-amount or timestamp-only rows stay missing because merge
  will not overwrite a leftover negative amount; 29 later positive amounts
  heal the gate and count as updates (23 indexes). Night same-session is
  5,171/5,171 with 0 dirty. Conservation is not a clean-state proof.
  Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-rollback-vs-poison.json`
  and layer-split section 53 (2026-09-04 20:38): rolling back a failed
  post-merge project recovers exactly the 158 zero-amount/timestamp-only
  rows (15,118/387 vs persist 14,960/545). 12 of 23 dirty→Ok heals still
  carry leftover OHLC/book/volume from the dirty record. Night persist
  equals rollback. Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-snapshot-would-publish.json`
  and layer-split section 54 (2026-09-04 20:41): hypothetical only —
  ShadowSnapshot still has no quotes. End-of-stream persist oem_states with
  the stale 09:15 map would emit 5,911/6,186 indexes, 4,377 wrong tickers
  (74.0%), and last_close matching the mapped stock's OEM last on
  5,884/5,911 (99.5%). Night same-session is 5,171 right / 5,166 last vs
  seq≥21 OEM. Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-correct-two-map-snapshot.json`
  and layer-split section 55 (2026-09-04 20:45): decode-session index→code
  plus same-day 0104 by `(market, code)` publishes 5,886 end-of-stream
  quotes with 0 wrong tickers and 5,669/5,669 last vs true-code OEM
  (stale same-table was 4,377 wrong). 275 negative-amount leftovers stay.
  Runtime still fills both maps from one table. Parallel session did not
  edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-two-map-plus-rollback.json`
  and layer-split section 56 (2026-09-04 20:48): decode-session index→code
  plus morning-by-code last plus project-err rollback publishes 6,122
  end-of-stream quotes, 0 wrong tickers, 0 dirty terminals, 5,898/5,898
  last vs OEM. 236 of 275 persist-dirty indexes recover the last Ok
  state; 39 stay not-ready. Runtime still same-table and no rollback.
  Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-recovered-last-ok-vs-oem.json`
  and layer-split section 57 (2026-09-04 20:52): the 236 rollback-recovered
  last-Ok rows match last_close 229/229 but are not the current OEM
  leftover/live group. 223 OEM-live: 76 would emit zeros, 146 nonzero
  mismatch, 1 close equals OEM price. First-print remains public OEM live.
  `start_with_code_tables` still cannot split the two maps. Parallel
  session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-persist-clean-vs-oem-live.json`
  and layer-split section 58 (2026-09-04 20:55): persist-clean 5,886
  quotes are 5,669/5,669 last vs OEM but only 65/5,421 live closes match
  OEM price (729 zeros, 4,627 mismatch). Sample 23830 correct ticker
  600228 leftover close 48.27 vs OEM 10.40. Do not publish persist close
  as auction live. Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/runtime-2023-night-dump-close-vs-oem.json`
  and layer-split section 59 (2026-09-04 20:57): same persist replay on
  the 01:19 same-session dump is 5,171 publishable, 0 dirty, last_close
  5,166/5,166, and close equals OEM price on 5,166/5,166 (SH 1,865,
  SZ 3,301). Auction persist-clean remains 65/5,421. Night dump 311B is
  the public OEM state; wine-tail end-of-stream is leftover vs live.
  Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-auction-65-coincidence.json`
  and layer-split section 60 (2026-09-04 21:02): the 65 close==OEM live
  hits are SZ 60 / SH 5. 52/65 already equal OEM on the first wine-tail
  frame (9 also equal 0104 last). 8 nearby nonzero ticks; only 5 print
  from close=0 (5/5,421 of OEM-live persist-clean). Later close=0 is
  sparse merge, not a price move. Parallel session did not edit runtime
  or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-first-print-fields.json`
  and layer-split section 61 (2026-09-04 21:06): persist vs last OEM on
  5,421 OEM-live persist-clean rows is all_six 0. The 5 first-prints
  match close 5/5 but bid1 0/5 (sample 000025 leftover bid1 −2.39e6 vs
  15.50). Close-mismatch amount hits stay 0. SH 5/2,382 vs SZ 60/3,039
  close hits. Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/runtime-2023-night-dump-all-six.json`
  and layer-split section 62 (2026-09-04 21:10): same persist project_state
  on the 01:19 dump is all_six 5,166/5,166 including bid1 at +0x68.
  Auction first-print bid1 0/5 is leftover book, not a getter defect.
  Parallel session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-first-print-incoming-bid1.json`
  and layer-split section 63 (2026-09-04 21:13): the 5 first-print leftover
  bid1 values are already on the live incoming 2704 (3 leftover integers,
  1 one-fen miss, 1 zero-slot). Merge did not drop an OEM book. Parallel
  session did not edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-first-print-same-second.json`
  and layer-split section 64 (2026-09-04 21:16): exact-second + max-seq
  join of the 5 first-print live frames is 09:25:00, close 5/5, bid1 0/5,
  all_six 0. Same-second OEM already has the public book; incoming bid1
  leftover is not a later-snapshot artifact. Parallel session did not
  edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-joined-live-bid1-leftover.json`
  and layer-split section 65 (2026-09-04 21:19): full exact-second live
  join is 5,039 rows; leftover i32 bid1 719 (14.3%); OEM-equal bid1 19
  (0.38%). Among 46 close-equal live rows, bid1 equals OEM on only 6.
  Matching close is not a public book. Parallel session did not edit
  runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-joined-live-all-six.json`
  and layer-split section 66 (2026-09-04 21:22): 5,039 live joins have
  all_six 0. The 11 close∧bid1 rows (including 0=0) have amount 0/11.
  Sample 002342 volume 354,128 vs OEM 1,543. Parallel session did not
  edit runtime or official_5188.rs.
- `diagnostics/20260904-live-rust/auction-0924/runtime-2023-joined-live-amount-hits.json`
  and layer-split section 67 (2026-09-04 21:24): the 3 live amount hits
  are 000025/000823/002703; five fields match and only leftover i32 bid1
  misses. Parallel session did not edit runtime or official_5188.rs.
- Compile `191408-18d2108898e7393f` completed the workspace-wide regression
  gate after the seed and shadow aggregation changes: all workspace tests,
  all 17 `tuwenca-codec` contract tests, all-target Clippy with `-D warnings`,
  and the release build passed. This is offline/build evidence only; no
  deployment, login, day-cut/reconnect runtime acceptance, or no-seed
  readiness behavior was exercised.
- Compile `193139-18d2108898e73943` verified the evidence-backed OEM book
  conversion boundaries: 5 projection-focused tests passed, including
  non-STAR nonzero-price/zero-volume fill and STAR half-up lot conversion with
  a positive one-lot floor. Workspace Clippy with `-D warnings` and release
  build also passed. This is a layer-C projection fix only; it does not reduce
  the open 2704 bitstream, online stability, or product-publication gates.
- Compile `193744-18d2108898e73946` verified the business-timestamp join
  regressions: 5 focused parity tests passed, covering window-independent
  same-second matching, rejection of a nearby wrong business second,
  ambiguity accounting, and the narrowly constrained live-over-leftover
  sequence tie-break. Workspace Clippy with `-D warnings` and release build
  passed. This does not claim online callback parity or close the publication
  gate.
- A fresh local rerun of `scripts/audit-official-5188-partition-alignment.py`
  against `diagnostics/20260902-live-pair/fixed-accept-2000/extract` confirms
  the changing-fixture shape: 7 `2a10` partitions, 6,203 total entries, 5,214
  eligible 0104 entries, and all five 1,024-entry primary partitions equal
  the current eligible prefix entry-for-entry. The tail partitions remain
  `578 SH + 446 SZ` and `59 SZ`, so this supports dynamic ordering rather than
  a protocol-size constant; formal current-day byte parity is still open.
- Compile `193416-18d2108898e73944` then passed the workspace-wide regression
  after the OEM projection conversion fix: workspace all-target tests passed,
  including all 17 `tuwenca-codec` contract tests; workspace all-target Clippy
  with `-D warnings` passed; and the release build passed. This remains offline
  regression evidence and does not close the formal-account online gates.
- Compile `193744-18d2108898e73946` passed the parity join focused suite
  (`5/5`): global business-second matching ignores the wall-clock window,
  rejects nearby wrong business seconds, counts ambiguity, and applies the
  constrained zero-price leftover/nonzero-price live sequence tie-break.
  Workspace Clippy (`-D warnings`) and release also passed. Real online
  callback parity and state-group coverage remain open.
- Compile `194233-18d2108898e73948` passed nightly rustfmt and the same
  business-timestamp focused suite (`5/5`). This confirms the join rule is
  formatted and regression-tested after the constrained sequence tie-break;
  it is still an offline helper and has not been promoted to product output.
- `docs/codex/tasks/cold-start-parity-plan-2026-09-04.md`
- `docs/EXPERIENCE.md`
- `windows_debug/zcode-handoff-20260904.md`
- (2026-09-04 zcode read-only failure-context pass on
  `extract-rust-222-ladder-trace-v4/manifest.json`,
  `diagnostics/20260904-live-rust/auction-0924/ladder-trace-v4-failure-context.json`:
  45/49 failed frames begin the failing record at a byte boundary
  (residue_bit_offset=0), so cross-frame reader drift is ruled out for those;
  4 frames start mid-byte at bit offset 5 (2 ladder_volumes, 1 ohlc) and are
  the only residue-alignment suspects. The slot gap before a failure is median
  44ms / max 90ms — failures occur in dense traffic, not idle slots. All 27
  errors exposing market+symbol_index are that symbol's FIRST appearance in its
  slot (0 had an earlier decoded record); each fails exactly once and none of
  the 27 ever yields a successful record later in the window. The other 22 fail
  before symbol_index is parsed (record_mask/record_header/timestamp_delta/
  ladder_values/trailing_da). Implication: a baseline-only replay can never
  validate these records' values — same-session capture from session start is
  the required evidence — and the 4 offset-5 frames are the only candidates for
  a genuine in-stream alignment defect.)
- (2026-09-04 bounded offset-5 slice: joining the same failure list to the
  manifest identifies `0033` and `0350` failing at `ladder_volumes`, `0373` at
  `ohlc`, and `0668` at `ladder_volumes`, all with 2-6 remaining bits at the
  failing read. No clean frame shares both payload length and record count with
  the first two; the latter two have only one and three clean controls. This
  narrows the next comparison to payload prefix, layout/mask/header and Wine
  reader offsets; it is insufficient evidence for a decoder change or an
  unconditional `0x5b291c` read. See
  `docs/forensics/official-5188-volume-diagnostics-20260904.md`.)
- Compile `201436-18d2108898e73952` passed the repaired same-session 0104
  metadata seed gate: nightly rustfmt, the complete-row OEM projection test,
  shadow decoder seed/not-ready focused tests (`2/2`), workspace all-target
  tests, all 17 codec contract tests, workspace Clippy with `-D warnings`,
  and the release build. The earlier failures were respectively a missing
  market field in the constructed 311B record, a seed-map closure borrow, and
  a rustfmt-only assertion layout; all are fixed. This remains offline
  evidence: reconnect/day-cut re-seeding, formal-account online coverage,
  2704 close-mode failures, and product wiring remain open.
- Runtime review after `201436` confirms that `Official5188ShadowReader`
  receives its full 0104 seed map only at reader construction. The service
  currently tears down readers before a fresh slot initialization, but there
  is no standalone day-cut/reconnect regression proving that stale seed maps
  are cleared before the replacement session is seeded. Keep this lifecycle
  item open until a session-keyed reset/reseed test and an online evidence run
  cover both the no-seed `not-ready` path and the complete metadata-row path.
- The authoritative `netzip-fullpull` crate now exposes
  `Official5188OemState::merge_and_project_from_code_table_metadata`, which
  clones, merges, projects, and commits only on success; a focused regression
  proves an invalid metadata scale leaves the state unchanged. The shadow
  reader now uses this API and inserts a new symbol state only after a
  successful projection; rollback/dirty-state behavior is therefore covered
  locally. Online failure-recovery evidence remains open.
- Compile `202434-18d2108898e7395b` passed after enforcing the metadata
  projection hard gate in runtime: a decoded record is merged only when the
  complete same-session 0104 row can construct and project successfully;
  there is no `previous_close`/`0x12b` fallback for an invalid or missing row.
  Nightly rustfmt, shadow focused tests, workspace all-target tests, all 17
  codec contract tests, Clippy with `-D warnings`, and release all passed.
  This proves the offline rejection path, not online reconnect/day-cut
  re-seeding or formal-account coverage.
- Static audit after the hard-gate change finds no runtime call to
  `Official5188OemState::new(...)`; OEM state creation in the shadow merge is
  preceded by complete-row construction and `project_from_code_table_metadata`
  validation. The remaining `new(...)` calls are isolated fixture tests, so
  they cannot provide production fallback data.
- Parallel-session fixture check of the 20:23 hard gate
  (`runtime-2023-project-gate.json`, layer-split section 51): construct+project
  Ok does not prove the 0104 row is from the decode session. Stale 09:15 A-share
  rows still project and would emit the wrong ticker on 6,098/9,184 exact-second
  joins. Post-merge negative-amount project failures (232) do not un-merge.
- Parallel-session ordered replay (`runtime-2023-dirty-merge-persist.json`,
  layer-split section 52): full wine-tail 15,505 = 14,960 updates + 545
  missing. 158 later zero-amount/timestamp-only rows stay missing on leftover
  negative amount; 29 later positive amounts heal and count as updates.
  Night dump 5,171/5,171 dirty=0. Conservation can hold on a dirty oem_states.
- Parallel-session rollback replay (`runtime-2023-rollback-vs-poison.json`,
  layer-split section 53): failed-project rollback is 15,118 updates / 387
  missing (+158 vs persist). 12/23 heals keep leftover book/OHLC. Night
  persist ≡ rollback. Do not treat the remaining 387 as bitstream loss.
- Parallel-session hypothetical snapshot
  (`runtime-2023-snapshot-would-publish.json`, layer-split section 54):
  wiring persist oem_states would emit 5,911 quotes, 4,377 wrong tickers,
  99.5% last_close-self-consistent with the mapped code. Night same-session
  5,171/5,166. Snapshot is still not wired.
- Parallel-session two-map recipe
  (`runtime-2023-correct-two-map-snapshot.json`, layer-split section 55):
  decode-session index→code plus morning-by-code last is 0 wrong tickers
  and 5,669/5,669 last vs OEM. The 275 dirty leftovers are unchanged.
  Runtime still uses one table for both maps.
- Parallel-session combined recipe
  (`runtime-2023-two-map-plus-rollback.json`, layer-split section 56):
  split maps plus rollback is 6,122 publishable, 0 wrong tickers, 0 dirty
  terminals, 5,898/5,898 last. 236 recover last-Ok; 39 stay not-ready.
  Snapshot is still not wired.
- Parallel-session recovered-state join
  (`runtime-2023-recovered-last-ok-vs-oem.json`, layer-split section 57):
  236 last-Ok recoveries are 229/229 last_close and 76 zero-vs-live plus
  146 live-price mismatches. Do not publish recovered schema-Ok as live.
  Snapshot is still not wired.
- Parallel-session persist-clean live join
  (`runtime-2023-persist-clean-vs-oem-live.json`, layer-split section 58):
  5,886 persist-clean quotes are 5,669/5,669 last and only 65 live closes
  match OEM price. Do not publish persist close as auction live.
  Snapshot is still not wired.
- Parallel-session night-dump positive control
  (`runtime-2023-night-dump-close-vs-oem.json`, layer-split section 59):
  same persist replay on the 01:19 dump is close==OEM price 5,166/5,166.
  Auction 65/5,421 is leftover vs live, not bitstream. Snapshot is still
  not wired.
- Parallel-session auction-65 first-frame split
  (`runtime-2023-auction-65-coincidence.json`, layer-split section 60):
  52/65 already equal OEM on the first wine-tail frame; only 5 print
  from close=0. Do not publish persist close as auction live.
  Snapshot is still not wired.
- Parallel-session first-print field join
  (`runtime-2023-first-print-fields.json`, layer-split section 61):
  5,421 OEM-live persist-clean rows have all_six 0. The 5 first-prints
  still miss bid1 (0/5). Do not publish persist 311B as the auction quote.
  Snapshot is still not wired.
- Parallel-session night-dump all_six control
  (`runtime-2023-night-dump-all-six.json`, layer-split section 62):
  same project_state on the 01:19 dump is all_six 5,166/5,166. Auction
  bid1 garbage is leftover, not +0x68. Snapshot is still not wired.
- Parallel-session first-print incoming bid1
  (`runtime-2023-first-print-incoming-bid1.json`, layer-split section 63):
  leftover bid1 is already on the live 2704; merge copied it. 0/5 dropped
  an OEM book. Snapshot is still not wired.
- Parallel-session first-print same-second join
  (`runtime-2023-first-print-same-second.json`, layer-split section 64):
  5/5 join 09:25:00, close 5/5, bid1 0/5 vs contemporaneous OEM. Leftover
  book is not a later-snapshot artifact. Snapshot is still not wired.
- Parallel-session joined live bid1 leftover
  (`runtime-2023-joined-live-bid1-leftover.json`, layer-split section 65):
  5,039 live joins have leftover i32 bid1 on 719 and OEM bid1 on 19.
  Close-equal live is 46 with bid1-equal only 6. Snapshot is still not wired.
- Parallel-session joined live all_six
  (`runtime-2023-joined-live-all-six.json`, layer-split section 66):
  5,039 live joins have all_six 0; close∧bid1 is 11 with amount 0.
  Snapshot is still not wired.
- Parallel-session joined live amount hits
  (`runtime-2023-joined-live-amount-hits.json`, layer-split section 67):
  the 3 amount hits miss only leftover bid1. Snapshot is still not wired.

- 2026-09-04 `netzip_win` follow-up adds two reusable validation artifacts. First,
  `official_5188_extract` output can be replayed through the complete-row 0104
  metadata projection into `stockdrv-compat` `RCV_REPORTV3` packets. The night
  baseline produced 13 frames / 1,848 records with 1,848 same-session seeds,
  1,848 projected states, and zero price-zero records. This is an offline ABI
  and projection regression only; it is not callback timing or auction parity.
  `crates/stockdrv-compat/examples/replay_2704_push.rs` now fails closed when
  any decoded record is unseeded or any state projection fails, rather than
  writing a partial push packet.
- The same night baseline is intentionally a clean non-ladder control: its
  masks are only `{0,128,192}` and it contains no `ladder_volumes` records.
  Its 13/13 decode pass therefore guards reassembly and ordinary value paths,
  but cannot close the active-session ladder failure gate. The synchronized
  capture entrypoint is `scripts/run-sync-dual-session.sh`; it requires an
  already logged-in official client, captures only 5188, and must be run in a
  trading window with the official and replica sessions active together. The
  resulting 2a10 verdict is diagnostic unless both sides use the same window
  and account class.
- Compile `211002-18d2108898e7395d` completed successfully (`cargo build
  --release`, status 0) after the fail-closed replay change. This verifies the
  release build only; it does not promote the offline packet into production
  or close the active-market decoder and parity gates.
- The synchronized capture tooling was tightened after review: `2704-capture-stats.py`
  now keeps each downlink TCP four-tuple in a separate sequence space, avoiding
  cross-slot reassembly when several 5188 sockets share one server endpoint.
  `export-official-2a10-sync.py` now hard-fails when the clone-flow candidate is
  tied or within one bin of the next-largest endpoint; automatic official-side
  attribution is only allowed with a uniquely dominant candidate. These checks
  prevent misleading coverage/parity reports and do not change decoder semantics.
- Compile `212146-18d2108898e7395e` completed successfully (`cargo build
  --release`, status 0) after the capture-tool hardening. The only emitted
  warning is the pre-existing unused Cargo sparse-registry config key.

Ownership constraint: the active main session owns
`/home/codes/stock/crates/netzip-fullpull/src/official_5188.rs` and
`examples/official_5188_callback_parity.rs`. Other sessions must keep those files read-only unless
ownership is explicitly handed off.
