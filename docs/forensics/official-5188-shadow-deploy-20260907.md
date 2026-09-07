# Official 5188 Shadow Deployment Evidence - 2026-09-07

## Deployment

- webClx request: `095718-18d2e6663e8874c4`
- build/install window: 09:57:24-09:58:12 China Standard Time
- installed SHA-256: `8ba70ee642ec50ec3ea2ca8cd764e404814a8d0372f57963af5e46787d07b80f`
- rollback SHA-256: `cd3b525e181c129cd0e09b8758bbbafcef5809a678303e5a78c01c6b58939829`
- install audit:
  `/home/bin/webclx/compile/runs/20260907T095723-3162165-16468/install-report-1.json`

Both services restarted once through the project install script and remained
active with zero systemd restarts. The new endpoint is research-only:
`GET /api/fullpull/official-5188/shadow/quotes` returned
`source=netzipRustOfficial5188Shadow` and `publication=disabled`. No
quoteGateway or stockScreener source switch was made.

## Online Gates

The transport gate passed with authentication, the selected 5188 endpoint,
ten initialized connections, four code tables, a receive list, advancing
business frames, and zero receive termination.

The strict trade-quote gate failed closed over ten seconds:

- attempted frames: 847
- decoded frames: 734
- partial frames: 109
- failed frames: 113
- decoded records: 14,186
- OEM updates: 13,670
- missing metadata: 516
- non-ladder errors: 81
- ladder-volume errors: 32

Record accounting held, but both `metadata_ready` and `trade_fields_clean`
failed. This build must remain shadow-only.

## Complete-Initialization Capture

- capture:
  `captures/20260907-shadow-deploy/official-5188-restart-full-init.pcap`
- SHA-256: `4f4f8bb1f0997bdf27e81af2bb96cf0a817f55cda28f4164db72bab5b164945b`
- tcpdump: 111,127 packets captured, zero kernel drops
- extraction:
  `diagnostics/20260907-shadow-deploy/extract-full-init-v1`
- strict seed provenance: four capture code tables, 32,649 metadata records,
  no external metadata and no external core baseline

The extractor exported 30,290 complete application frames. Of 26,722 `2704`
frames, 22,111 decoded cleanly and 4,611 failed; 4,268 failures retained a
decoded prefix. There were 486 `missing_fresh` failures and 234 failures with
residue bit offset 5. Failure stages were:

| Stage | Frames |
|---|---:|
| ladder_volumes | 1,518 |
| ladder_values | 616 |
| accumulators | 597 |
| ohlc | 479 |
| record_mask | 360 |
| record_header | 305 |
| trailing_da | 234 |
| aux | 188 |
| offset_2c | 121 |
| timestamp_delta | 79 |
| special_book_prices | 52 |
| special_book_volumes | 49 |
| ladder_move | 13 |

Unlike the earlier passive capture, this capture started before the service
restart and contains the same-session code tables and initial stream. The 486
`missing_fresh` cases therefore remain decoder/lifecycle evidence rather than
being dismissible as a capture-start boundary.

## Semantic Projection Failure

The shadow endpoint produced 5,162 deduplicated market/code rows, but 150 had
timestamps outside 2026-09-07. Several projected rows also contained values
that cannot be public market data:

- `SZ301068`: timestamp 1133 and negative bid/ask volumes.
- `SH603696`: timestamp 1062000 and ask prices near 1,548,666.
- `SZ301136`: volume -8,049,897,776, bid volume -2,110,124,814, and ask prices
  near 0.01 despite a last price near 12.65.

A field-level count over that snapshot found 299 negative last prices, 322
negative opens, 310 negative highs, 284 negative lows, 86 negative cumulative
volumes, 928 rows with a negative ask price, 2,085 with a negative bid price,
1,719 with a negative ask volume, and 1,393 with a negative bid volume. There
were also 2,492 rows with nonzero `high < low` and 4,215 whose nonzero last
price was outside the nonzero OHLC range. These counts show broad semantic
decode corruption rather than a small timestamp-only projection issue.

This proves that a successful structural decode plus an `Ok` projection is not
sufficient publication evidence. Some decoded prefixes are semantically
corrupt even when same-session 0104 metadata exists. The product path needs a
fail-closed semantic validation gate, while the owning decoder investigation
must trace these rows back to their `2704` record and bit offset. Do not hide
the decoder defect by counting filtered rows as successfully published.

## Decision

- Keep `publication=disabled` and preserve the existing product source.
- Treat the complete capture as the active-market decoder regression fixture.
- Add explicit rejected-projection metrics and validate trading-day timestamp,
  finite/nonnegative amounts and volumes, and coherent OHLC/book values before
  a row is exposed to any product adapter.
- Continue fixing the bitstream decoder until strict partial/failed frames and
  semantic-invalid projections reach zero, then repeat freshness, reconnect,
  and rollback acceptance before source promotion.

## Transactional Semantic Gate Deployment

webClx compile request `101519-18d2e6663e8874c6` passed 15 focused runtime
tests, two service tests, workspace all-target Clippy with `-D warnings`, and a
release build. Deployment request `101717-18d2e6663e8874c7` installed SHA-256
`bef6a5890a99344b4b8c2ac49f54d2328866a635da6be618265d9c520d25d64e`;
the rollback binary is the prior shadow build with SHA-256
`8ba70ee642ec50ec3ea2ca8cd764e404814a8d0372f57963af5e46787d07b80f`.

The runtime now clones or constructs the candidate OEM state, merges and
projects it, validates the complete public candidate, and commits the state
only after validation. Wrong-China-date timestamps, non-finite or negative
values, and nonzero `high < low` are rejected. Rejections are counted
separately from missing metadata. Runtime accounting is:

`decoded_records = oem_state_updates + missing_previous_close_seeds + rejected_public_quotes`.

The first post-deploy snapshot proved that accounting with 40,186 decoded =
23,975 updates + 1,211 missing + 15,000 rejected. The public snapshot retained
5,143 rows and had zero wrong-day timestamps, negative scalar values, negative
book values, or nonzero `high < low` rows.

The ten-second strict gate still failed, as intended: 12,151 decoded records
contained 5,768 updates, 466 missing metadata rows, and 5,917 semantic rejects;
176 frames failed, 162 were partial, and 128 errors were outside
`ladder_volumes`. Public quote semantic validation itself passed with 5,159
rows and zero invalid rows. This protects the research surface but does not
close the decoder or publication gate.

The deployment restart capture is
`captures/20260907-semantic-gate/official-5188-restart.pcap`, SHA-256
`d98ff17fd1b5bb7318c3c9f32042835e57ea4999a1253098044d5ce0d9a06164`.
It contains 105,984 packets with zero kernel drops.

Three representative symbols show where invalid state first appears in the
complete-initialization extraction:

- `SZ301136`, index 3882: frame ordinal 43, bits 776..821, mask `0xf8`; the
  decoded candidate already has `high < low` (12.85 < 13.06).
- `SH603696`, index 25095: frame ordinal 836, bits 584..744, mask `0x20`;
  the decoded ladder contains prices -4..5 before public scaling.
- `SZ301068`, index 3821: frame ordinal 7644, bits 1984..2124, mask `0x36`;
  the decoded ladder contains prices -5..3.

These are decoder trace targets. The transactional gate must not be mistaken
for a protocol fix: the rejected count must reach zero before publication.

## Broad Range Gate Deployment

webClx compile request `102233-18d2e6663e8874c8` passed rustfmt, 15 focused
runtime tests, the focused service accounting test, workspace all-target
Clippy with `-D warnings`, and the release build. The range-gate candidate was
deployed at 10:28 China Standard Time through webClx service deployment using
the project install script. Installed SHA-256 is
`4570bc88b98160f2788ae9d3a14e0ba8733f48ef16c789d00d517ee14fca0189`;
rollback SHA-256 is
`bef6a5890a99344b4b8c2ac49f54d2328866a635da6be618265d9c520d25d64e`.
Both `quote-netzip-rs-supplement.service` and
`quote-netzip-rs-full-push.service` were active with `NRestarts=0` after the
deployment. The shadow retained ten connections, remained initialized on
`222.85.139.177:5188`, and reported zero receive terminations. Publication
remained disabled.

The ten-second trade-quote readiness observation retained the accounting
invariant: 11,359 decoded records = 3,292 committed OEM updates + 468 missing
metadata records + 7,599 rejected public candidates. The public research API
contained 5,142 rows and reported zero invalid timestamps, non-finite or
negative values, incoherent OHLC rows, broad price-range violations, and broad
quantity-range violations.

The product gate still failed closed. The same interval contained 165 failed
frames, 153 partial frames, 128 non-ladder errors, 37 ladder-volume errors,
468 missing metadata records, and 7,599 semantic rejects. The range checks
therefore prevent known positive corruption from entering the research
snapshot, but do not repair the decoder. Keep quoteGateway and stockScreener
disconnected from this source until failed, partial, missing and rejected
counts all reach zero and subsequent freshness and recovery acceptance passes.

## Stored-state inheritance experiment (rejected)

The restricted `stored`-state initialization experiment was not deployed.
Although focused tests passed, replay of the complete same-session
initialization capture was unchanged in both runs: 26,722 `2704` frames,
22,111 clean, 4,611 failed, 4,268 partial, and 1,518 ladder-volume failures.
The branch was removed. Before another state-inheritance change, add
trace-only counters for `stored_present`, `baseline_present`, mask class, and
header shape to prove whether the proposed condition is reached.

## Decoder lifecycle trace (13:46)

webClx compile request `134517-18d2e6663e8874d1` passed the fullpull test
suite (193 passed, one ignored), all-target Clippy with `-D warnings`, and the
release extractor build. It replayed the unchanged full-initialization pcap
into `diagnostics/20260907-shadow-deploy/extract-full-init-lifecycle-v2`.
This was trace-only and was not deployed.

The reusable analyzer is `scripts/analyze-official-5188-lifecycle.py`; its
report is `diagnostics/20260907-shadow-deploy/lifecycle-buckets-v2.json`.
Frame totals remain exactly 22,111 clean, 4,268 partial, and 343 with no
completed prefix (4,611 errors total). Parsed error stages are 1,518
`ladder_volumes`, 616 `ladder_values`, 597 `accumulators`, 479 `ohlc`, 234
`trailing_da`, 188 `aux`, 121 `offset_2c`, 79 `timestamp_delta`, 52
`special_book_prices`, 49 `special_book_volumes`, and 13 `ladder_move`.

Of the 4,611 errors, 3,946 now carry the complete lifecycle tuple
`(stored_present, baseline_present, mask bit 0, mask_class, clear_ladder,
raw_level_count, level_count)`. The other 665 occur before those fields can be
known: 360 exhaust exactly while reading the next record mask and 305 while
reading its five-bit header. Most have zero bits remaining after a completed
record, or exactly one mask byte remaining, while the index stream still lists
one or more entries. This is strong evidence for a legal short value-stream
tail or a still-unknown termination rule, but not permission to synthesize
the missing records. Wine-tail's zero/clamp behavior remains diagnostic only.

The first-pollution traces all show `stored_present=true` and
`baseline_present=false`, but failures also occur in large relative-baseline
buckets. Therefore `stored` is not generally interchangeable with the
relative baseline, and the rejected stored-current experiments remain
rejected. No token table, mask rule, state commit, runtime publication, or
online service behavior changed in this trace pass.

The analyzer also reconstructed each target's preceding successful record on
the same TCP lane. `SH603696` had stored last 1552, while the absolute record
expanded its ladder around zero into `[-4,-3,-2,-1,0,1,2,3,4,5]`.
`SZ301068` had stored last 2762, while its absolute record expanded into
`[-5,-4,-3,-2,-1,0,1,2,0,0]`. This isolates the next experiment to the
ladder anchor reference: using the stored last only when there is no relative
baseline and the decoded current last is still zero predicts ranges
1548..1557 and 2757..2764. It does not justify restoring whole-record
`stored.clone()` initialization, which was already rejected. `SZ301136` is a
different header shape (`clear_ladder=false`, raw level count 6) and must not
be grouped into that anchor experiment.

That anchor-only experiment was run under webClx request
`135510-18d2e6663e8874d3` and rejected. Tests, Clippy, release build, and the
replay completed, and parsing counts/buckets were identical to the baseline.
The two target ladders moved to the predicted positive ranges, but the full
comparison (`anchor-experiment-v1.json`) found 14,178 changed records across
8,524 files. It removed negative prices from 8,555 records, retained them in
3,520, and introduced negative prices in 107 previously nonnegative records.
Price-dependent volume merging caused 2,200 records to differ outside the
ladder-price byte range. The code experiment was therefore removed and must
not be deployed. Stored last is correlated with the target anchor but is not
a sufficient global selector; the missing discriminator is still unresolved.

A narrower selector was then tested under webClx request
`140317-18d2e6663e8874d4`: only `clear_ladder=true`, `level_count=0`, positive
stored last, no baseline, and zero current last used the stored last as the
ladder anchor. It removed the 107 newly introduced negative-price records
seen in the wider experiment, but still changed 9,974 records across 6,642
files and caused 1,583 non-price-byte differences through later state
propagation. This remains too wide without Wine field parity, so the
experiment was removed and not deployed. Both stored-anchor experiments are
diagnostic evidence only.

The next isolated change addressed only truncated baseline-only tails. Before
starting a record, the partial/fresh decoder may stop successfully when fewer
than 13 value bits remain and all remaining index entries have
`uses_baseline=true`. It emits no synthetic record and performs no state
update for the omitted indexes; the strict all-record API remains strict.
WebClx request `143654-18d2e6663e8874d9` passed workspace tests, Clippy,
release extraction, and a complete-initialization replay assertion. The new
manifest records 665 affected frames and 1,399 omitted indexes. Frame totals
are 26,722: 22,776 clean, 3,603 partial with completed prefixes, and 343 with
no completed prefix. All 3,946 substantive error stages are unchanged, led
by 1,518 `ladder_volumes`. This improves classification and observability
without claiming the remaining bitstream gap is fixed.

The selector was also replayed against the Monday active-market clone lane
from `diagnostics/20260907-sync-open/sync-run2-5188.pcap` with an exact local
endpoint filter (`192.168.3.2:51470`). WebClx request
`144831-18d2e6663e8874dc` covered 184 `2704` frames: 149 had no error, 35 kept
substantive errors, and only 2 frames/2 indexes used the baseline-tail
omission. An apples-to-apples Wine-tail run from the same current binary,
seed provenance, capture, and lane (`144950-18d2e6663e8874dd`) reported all
184 frames clean, but it added 42 records across the 35 strict-error frames.
Only 7 additions consumed zero bits and only 8 had all inspected public
fields zero; two later frames changed their already-decoded prefix through
state propagation. Evidence is in
`diagnostics/20260907-sync-open/wine-tail-vs-short-tail-clone-v2.json`.
Therefore global Wine-tail padding remains rejected; it is not an extension
of the narrow baseline-only omission rule.

The close-window capture (`official-5188-10m.pcap`, SHA256
`d057bc73944a38534bc0646c44ef830a798cfd3f7d42e2d87d192d326bf69924`, zero
kernel drops) was replayed under webClx request `150123-18d2e6663e8874df`.
Across nine 5188 flows and 101,765 `2704` frames, 82,149 were clean and
19,616 retained substantive errors. The strict short-tail selector omitted
9,392 baseline-only indexes in 4,363 frames. Error stages were
`ladder_volumes=7,030`, `accumulators=3,191`, `ohlc=2,739`,
`ladder_values=2,602`, `trailing_da=1,225`, `aux=1,089`, with smaller
timestamp/book/ladder-move buckets. Error rates remained persistent across
the ten-minute window rather than collapsing after the first minute; this is
diagnostic evidence of an unresolved active-stream decoder issue, not proof
that packet capture or TCP delivery failed. Per-flow counts are recorded in
`diagnostics/20260907-close-window-144507/extract-summary.json`.
For targeted follow-up, `diagnostics/20260907-close-window-144507/ladder-volume-target-buckets.json`
indexes all 7,030 `ladder_volumes` failures by mask/header/baseline mode and
links representative frames to their payload and value-trace files. The
largest bucket is `mask=0xe8`, `header=0x03`, absolute mode (257 frames).
Read-only parsing of that bucket found 158 layout/slot/mask variants rather
than one fixed tail width. The common shape is `raw_level_count=0`, but the
layout is primarily `0x2d` with a `0x3ff` volume mask and varying volume slot;
other layouts (`0x28`, `0x29`, `0x2a`, `0x2c`) also occur. Required bits versus
remaining bits range from 6/0 through 32/16. This variation rules out a single
constant-byte omission or unconditional mask read as the immediate fix. The
bucket remains a focused trace target, with token and mask semantics unchanged.
For the dominant `layout=0x2d`, `volume_mask=0x3ff` family, a comparison
manifest with representative successful traces and failed frame payloads is
stored at `diagnostics/20260907-close-window-144507/ladder-volume-e8-03-2d-compare.json`.
It is an input set for the decoder owner only; no production branch was
changed.

## Ladder-volume same-key trace diff (2026-09-07, trace-only)

The previous `e8-03-2d-compare.json` mixed all `layout=0x2d` /
`volume_mask=0x3ff` failures regardless of mask/header, so its 5,414 count is
not a 5-tuple bucket. The corrected analyzer
`scripts/analyze-official-5188-ladder-volume-trace-diff.py` groups by
`(mask, header, baseline_mode, ladder_layout, ladder_volume_mask)`.

Corrected priority key
`mask=0xe8 header=0x03 baseline_mode=absolute layout=0x2d volume_mask=0x3ff`
on the close-window extract:

- 2,362 successful records
- 204 failed records (not 5,414)
- success aligned width median 232 bits (min 168, max 368)
- failure sample: previous record `aligned_end` equals this `record_start`
  (no in-frame offset gap); volumes start after an 18-bit ladder_values span;
  stream ends at bit 448 with `need 8, have 2`

Classified 204 priority-key failures (existing extract, before new
last_before fields):

- 156 `payload_physical_truncation` (exhausted at end-of-stream, previous
  record contiguous, remaining bits at record start below the success median)
- 44 `missing_layout_or_volume_mask_semantics` (same 5-tuple and price_mask
  also succeed, prefix width in the success range, leftover bits `have>0`)
- 4 unresolved
- 0 previous-record lifecycle

Absolute records never select a relative baseline (`mask & 1 == 0`), so
`baseline_present=false` is tautological for this key and does not separate
success from failure. Both sides also show `stored_present=true`. No
permission to use stored last as a global anchor.

Short-tail omitted frames/records remain 4,363 / 9,392 and are counted
separately from the 7,030 `ladder_volumes` errors. Residue offset=5 failures:
1,321. Frame totals unchanged: 101,765 `2704`, 82,149 clean, 19,616 errors
(19,145 partial + 471 failed). Record-level: 1,437,697 completed prefixes,
19,616 failed records, 9,392 omitted, 328 `missing_fresh`. Runtime
`rejected` / `missing_seed` public-quote counters are not bitstream decoder
metrics and were not used.

Three historical pollution ordinals from the full-init extract, completed
records (not the later frame error):

- SZ301136 idx3882 ordinal 43 bits 776..821 mask `0xf8`: `clear_ladder=false`,
  last=1285, no previous 2704 on the lane; `stored_present=true` is consistent
  with a 0104 seed. Not a ladder_volumes failure of this symbol.
- SH603696 idx25095 ordinal 836 bits 584..744 mask `0x20`: last=0 after
  decode, previous same-symbol last=1552, absolute so baseline unselected.
  Stored last was not used as ladder anchor. Record completed; do not
  zero-fill or guess an anchor.
- SZ301068 idx3821 ordinal 7644 bits 1984..2124 mask `0x36`: last=0,
  previous last=2762, same absolute unselected-baseline pattern. Frame is
  clean.

Decoder observability fields (`mask_has_bit0`, `stored_source`,
`committed_state_present`, `current_last_before`, `stored_last_before`,
`committed_last_before`, `baseline_last_before`, `anchor_source`) were added
to the forensic trace/error surface only. Parse branches, token tables, and
mask interpretation are unchanged. Publication remains disabled. No minimal
repair is justified yet: the 5-tuple still has both successes and failures,
and truncation vs leftover-bit semantics are not a single falsifiable rule.

Evidence: `diagnostics/20260907-close-window-144507/ladder-volume-trace-diff-v1.json`
SHA256 `6897826f5287c49b6e652024ba5ff4c68cb7828db4409935c79d318fb6f51d79`.
WebClx compile request `170637-18d2e6663e8874e0` failed in ~1s:
`cargo test` rejected multiple TESTNAME argv (`traced_absolute_record`).
Tests, Clippy, release extract, and replay did not run.

Prefix-audit on the same existing extract (no new binary), evidence
`diagnostics/20260907-close-window-144507/ladder-volume-trace-diff-v2.json`
SHA256 `1140d32ae5bb2424a23275c60987b8e4af9a79381fa918cca9e09a76b1e5ce59`:

Method: sum each completed prefix record's clean-frame 5-tuple
median/min `aligned_width`, compare to `fail.record_start` and remaining
bits at that start. Success min aligned width for this key is 168 bits.

Of the 204 priority-key failures:

- 105 `prefix_overread` vs clean-frame median+16
- 97 `ambiguous`
- 2 `true_short_payload` (prefix near expected min and remaining < 168)
- remaining < 168: 39 ambiguous + 45 prefix_overread + 2 true_short = 86
- remaining >= 168: 118

The earlier 156 `payload_physical_truncation` used remaining < success
*median* 232, which over-states truncation: many remainders still fit
the success *min* 168. Canonical sample `record_start=296` remaining 152
has prefix 296 vs expected median 301 (not overread). That is still a
this-record short tail, not prefix-token pollution and not lifecycle.

`prefix_overread` is not a single falsifiable decoder bug: 5-tuple
aligned widths already span 168..368, so exceeding the median by 16 bits
can be in-range. No stored.clone(), Wine-tail productionization, token
table, or mask-interpretation change is justified. Publication remains
disabled. New last_before fields are still null until a successful
trace-only replay.

Follow-up webClx compile request `173110-18d2e6663e8874e1`
(trace-only / no deploy / no restart) used one cargo test filter per
invocation. Analyzer unit tests passed; `netzip-fullpull` lib tests failed
with E0425: `value_step!` could not see `current_last_before` due to
macro hygiene (`build-1.log` run `20260907T173115-3320316-5188`). Replay
did not run. The observation local is now declared before the macro;
parse branches are unchanged.

Retry webClx compile request `173619-18d2e6663e8874e2`
(trace-only / no deploy / no restart) compiled the focused tests but
failed Clippy `-D warnings`: dummy `current_last_before = 0` was
overwritten before read (`20260907T173624-3373356-6483`). The
observation local is now assigned after `current` is initialized and
before `value_step!`; the prefix snapshot write is restored. Parse
branches unchanged.

Retry webClx compile request `174150-18d2e6663e8874e4`
(trace-only / no deploy / no restart).

