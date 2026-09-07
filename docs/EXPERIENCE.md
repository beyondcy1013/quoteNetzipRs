# NetzipRs Experience

## 2026-09-04 - Account class boundary for online acceptance

`168/168` is a free test account and may be subject to time-window,
permission, connection-limit, or other server-side behavior that differs from
the formal service. Treat it only as `test-168` for short, single-session
diagnostics and hypothesis checks. Do not use its coverage, failures, or
responses as production protocol evidence. Online 5188/P6/P7/stability
acceptance must use a stable formal-authorized account, with the account class
(not credentials), server, login time, connection count, and 2a10/2704 results
recorded. Repeat account-dependent observations with the formal account before
changing shared decoder behavior.

The reproducible seed builder `scripts/coldstart/build-last-close-seed.py`
produced `diagnostics/20260904-live-rust/ten-slot-0909/last-close-seed-map.json`
from the morning fixture: 301 tables and 32,997 `(market, code)` seeds, with
zero invalid 23-byte tails and zero conflicting duplicates. The report is an
offline lifecycle input only; runtime injection still requires a focused
day-cut/reconnect regression.

Compile request `190656-18d2108898e7393d` passed `fmt --check`, executed the
`shadow_decoder_seeds_previous_close_by_market_and_index` regression (1 passed),
executed the no-secret status serialization regression (1 passed), then passed
Clippy with `-D warnings` and the `quoteNetzipRs` release build.

Compile request `190929-18d2108898e7393e` additionally verified the seed
metric contract: the status serialization test, multi-slot aggregation test,
and `shadow_decoder_seeds_previous_close_by_market_and_index` each ran and
passed (1/1), followed by successful Clippy and release checks.

An independent JSON audit confirms the seed report has 32,997 unique
`(market, code)` keys, zero invalid tails/conflicts, and the known
`SH603059=2468` settlement value.

Workspace request `191408-18d2108898e7393f` passed `fmt --check`, all workspace
tests (including 17 `tuwenca-codec` contract tests), all-target Clippy with
`-D warnings`, and the workspace release build. No deployment or login was
performed.

## 2026-09-04 18:40 - 586 same-second conflicts are leftover×print pairs

All 828 auction callback multi-groups are size 2 with two fingerprints and
the same last_close; typical pair is price 0 vs the auction print ~70 batches
apart. Night same-session OEM has zero such groups. 148 decoded groups zip
2=2; 290 have one 2704 vs two callbacks; never extra decoded records. Prefer
the later/non-zero quote; do not unmatched all 586 and do not let this pull
bitstream work. Script:
`scripts/coldstart/disambiguate-586-batch-identity.py`. Shadow now injects the
morning 0104 seed when available and counts missing seeds. Did not touch
`official_5188.rs`.

## 2026-09-04 18:45 - Later batch is the live quote on all 586 pairs

Every one of the 586 conflicts is exactly one price=0 leftover plus one
live print. Later callback sequence equals the live quote 586/586. Wall-clock
nearest picks the leftover 290/586 and all 143 of its "price hits" are 0=0.
Morning 0104 last_close hits 586/586 under every policy. Do not use the zero
incoming close to retune bitstream. Script:
`scripts/coldstart/score-586-prefer-live.py`. Did not touch
`official_5188.rs`.

## 2026-09-04 18:50 - STAR lots are half-up plus a 1-lot floor

Wine STAR 688/689 lots are `(v+50)//100` with a minimum of 1 when v>0, not
banker's `(f32/100).round()`. That closes STAR book volumes 615/615 and the
two cumulative-volume residuals. Full-record night parity with ±1s datetime
becomes 5,105/5,166; the remaining 22 book misses are non-STAR internal 0 vs
OEM 1. Script: `scripts/coldstart/ablate-star-book-lots.py`. Did not touch
`official_5188.rs`.

## 2026-09-04 18:55 - Empty book size with a price displays as 1 lot

All 22 remaining night book misses are internal volume 0 vs OEM 1 at a
level whose price is non-zero and already matches. Filling 1 when
`volume==0 && price!=0` yields 5,166/5,166 book volumes. Only the 39 close
seconds remain (5,127/5,166). Not a bitstream defect. Script:
`scripts/coldstart/classify-zero-vs-one-book.py`. Did not touch
`official_5188.rs`.

## 2026-09-04 19:00 - 39 datetime misses are 399-index close flooring

All 39 night datetimes beyond ±1s are SZ 399xxx indices: internal
15:00:03–15:00:39, OEM 15:00:00. Flooring any Asia/Shanghai time after
15:00:00 to 15:00:00 matches 5,166/5,166. Combined with STAR half-up and
the 1-lot empty-size fill, same-session night full-record parity is
5,166/5,166 (amount rel<1e-3). Not a bitstream result and not a publish
gate. Script: `scripts/coldstart/classify-datetime-overshoot.py`. Did not
touch `official_5188.rs`.

## 2026-09-04 19:05 - Shadow new(0) last_close is 0/9184; morning 0104 is 9184/9184

Auction wine-tail records joined by business second and max sequence: 9,184.
`Official5188OemState::new(0)` hits last_close 0/9,184. Live `0x12b` hits 106
(the v2 report number). Same-morning 0104 +11 hits 9,184/9,184. Wiring the
existing OemState seed closes this field without bitstream work. Recipe:
`oem-projection-recipe-v2.json`. Script:
`scripts/coldstart/score-oemstate-last-close-seed.py`. Did not touch
`official_5188.rs`.

## 2026-09-04 19:10 - Auction 6297 unmatched are poll skew, not a ±1s license

Wine-tail decoded 15,481 with metadata: 9,184 exact business-second joins
and 6,297 with no same-second callback. 498 codes never appear in this
callback capture (231 close=0); 5,799 appear at another second. Nearest
callback is ±1s for 3,929 of those, but a unique one-sided ±1s neighbor
is leftover-only on 2,172 rows. Widening to ±1s would pair leftover
quotes the same way wall-clock nearest did on the 586 same-second pairs.
Keep exact business second plus max sequence. Script:
`scripts/coldstart/classify-unmatched-no-same-second.py`. Did not touch
`official_5188.rs`.

## 2026-09-04 19:15 - Exact-second join closes last_close, not auction live price

Same 9,184 joins: leftover 4,145 and live 5,039 both hit last_close 100%
from morning 0104. Live price is 46/5,039. Of those live rows, 1,975 have
internal close=0 while OEM already prints, and 3,018 have a different
non-zero price (145 with |close|>1e6). This mid-session wine-tail is not
the same public state as Wine OEM; do not retune the night recipe or the
bitstream. Main-session runtime 18:58 now seeds OemState from login 0104
opaque_tail[+11] by (market, symbol_index) with unwrap_or(0). This session
did not edit runtime or official_5188.rs. Script:
`scripts/coldstart/score-joined-auction-fields.py`.

## 2026-09-04 19:20 - OemState merge does not rescue auction wine-tail

Capture-order sparse merge on the same 9,184 joins lifts live price only
46→65. Of 1,975 incoming close=0 live rows, 1,157 get a prior close from
this extract but only 19 then match OEM; 818 have no history here.
Leftover price hits fall 1,349→1,126 because carry-forward fills Wine's
explicit leftover zeros. Do not score leftover-only seconds as OemState
targets. The merge machine is right; this mid-session extract is not the
same-session dump. Did not edit official_5188.rs or runtime. Script:
`scripts/coldstart/replay-oemstate-then-join.py`.

## 2026-09-04 19:25 - Cross-session last_close seeds must stay market+code

Morning 0104 last_close hits 9,184/9,184 by (market, code) and only
2,929/9,184 (31.9%) if the same tables are applied by symbol_index onto
this wine-tail extract. 09-03 vs 09:15 index maps move 29,086 codes
(10.8% stable). Same-session runtime may key 0104 by index; reconnect
must rebuild from the new table. The offline seed map is already
market+code — do not inject it by index into another session. Did not
edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-last-close-key-index-vs-code.py`.

## 2026-09-04 19:30 - Trade fields stay leftover until that symbol's first print

All 5,039 live joins are at or after 09:25; all 3,845 joins at 09:24 are
leftover. A global 09:25 gate would not drop these live prints, but it
would un-zero 300 leftover rows still at price 0 after 09:25 (175 waiting
for that code's first print, 125 never live in this capture). Every
leftover join is before that code's first live callback or never-live.
Publish last_close from 0104 immediately; keep trade fields 0 until the
symbol's first live print. Did not edit official_5188.rs. Script:
`scripts/coldstart/classify-preopen-vs-first-print.py`.

## 2026-09-04 19:35 - First-print is public OEM live, not a nonzero 2704 close

Of 4,145 leftover OEM joins, 2,796 already have a nonzero incoming 2704
close (1,349 are 0=0). Publishing on first nonzero 2704 would emit a
price while Wine still reports leftover 0. Keep trade fields 0 until that
symbol's first live OEM print; leftover callbacks stay out of OemState
parity. Live incoming close=0 remains 1,975. Did not edit official_5188.rs.
Script: `scripts/coldstart/classify-leftover-2704-close.py`.

## 2026-09-04 19:40 - Leftover nonzero +0x10 tracks overnight 0x12b, not 0104

Of 2,796 leftover OEM rows with a nonzero 2704 close, only 49 equal the
morning 0104 last. 2,401 (86%) are within 5% of live 0x12b. Example
600228: internal 47.55, 0x12b 48.26, 0104 last 10.28. The close slot still
holds overnight residue after OEM has already switched last_close to 0104
and zeroed the trade price. Do not open first-print on that residue. Did
not edit official_5188.rs. Script:
`scripts/coldstart/classify-leftover-nz-close-source.py`.

## 2026-09-04 21:24 - The 3 live amount hits are leftover-bid1 first-prints; only the book misses

Read-only dump of the 3 exact-second live amount hits. They are 000025,
000823 and 002703 (mask 193) from sections 63–64. close/open/volume/amount/
last all match same-second OEM; bid1 is leftover i32 (−2.39e6 / −1.66e6 /
−1.85e6). all_six stays 0. The closest wine-tail public quotes still
cannot be published. Night dump bid1 remains 5,166/5,166. Did not edit
official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-joined-live-amount-hits.py`.
Details: layer-split section 67.

## 2026-09-04 21:22 - Live rows that match close and bid1 still have all_six 0

Read-only six-field score on exact-second live joins. 5,039 OEM-live rows
have all_six 0 (amount 3, matching the earlier joined-field live amount
hit). The 11 close∧bid1 rows (6 leftover-gated nonzero book hits plus
0=0) have amount 0/11, volume 5/11, open 6/11. Sample 002342 matches
close/bid1/last but volume 354,128 vs OEM 1,543. Matching price and
bid1 is not a public quote. Night dump all_six remains 5,166/5,166.
Do not publish persist 311B as auction live. Did not edit
official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-joined-live-all-six.py`.
Details: layer-split section 66.

## 2026-09-04 21:19 - Same-second live close hits still miss OEM bid1; only 6/46 match

Read-only exact-second + max-seq join on the full wine-tail: 9,184 joined,
5,039 OEM-live. Among 46 live rows whose close equals OEM, bid1 is leftover
i32 on 8, zero on 15, other-miss on 17, and OEM-equal on only 6. Across all
5,039 live joins, leftover i32 bid1 is 719 (14.3%) and OEM-equal bid1 is
19 (0.38%). Sample 000019 close=6.92 with bid1 −1.05e7. Matching close is
not a public book. Night dump bid1 remains 5,166/5,166. Do not publish
persist 311B as auction live. Did not edit official_5188.rs or runtime.
Script: `scripts/coldstart/score-runtime-2023-joined-live-bid1-leftover.py`.
Details: layer-split section 65.

## 2026-09-04 21:16 - Same-second OEM already has the public book; 2704 bid1 is still leftover

Read-only exact-second + max-sequence join on the 5 first-print live
frames. All five match `09:25:00` with one candidate. close 5/5, bid1
0/5, all_six 0. Same-second OEM bid1 equals last OEM bid1 (000025
15.50). Incoming bid1 stays leftover −2.39e6 / −1.66e6 / −1.85e6, or
6.42 vs 6.43, or 0. This is not a later-snapshot artifact. 003816 is
close-only while same-second OEM already has open/volume/amount/bid1.
Do not publish persist 311B as auction live. Night dump all_six remains
5,166/5,166. Did not edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-first-print-same-second.py`.
Details: layer-split section 64.

## 2026-09-04 21:13 - First-print leftover bid1 is on the incoming 2704, not dropped by merge

Read-only walk of the 5 first-print indexes (2 records each). Sparse merge
copies a book slot only when incoming bytes are nonzero. 0/5 have live
incoming bid1 equal OEM while persist misses. 000025/000823/002703 live
frames already carry leftover bid1 (−2.39e6 / −1.66e6 / −1.85e6);
002696 is 6.42 vs OEM 6.43; 003816 live close 4.39 with bid1 still 0
(mask 56). Merge copied what the wine-tail printed. Night dump bid1
remains 5,166/5,166. Do not treat these as a +0x68 or merge-drop defect,
and do not publish persist 311B as auction live. Did not edit
official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-first-print-incoming-bid1.py`.
Details: layer-split section 63.

## 2026-09-04 21:10 - Night dump persist all_six is 5166/5166; auction bid1 miss is leftover

Read-only same project_state (bid1 at +0x68) on the 01:19 dump seq≥21.
5,166 OEM-live persist rows match close, open, volume, amount, bid1 and
last_close together (0 dirty, 5 no-OEM). Auction section 61 stays all_six
0 and first-print bid1 0/5. The getter is correct on the same-session
dump; auction −2.39e6 bid1 is leftover book, not an offset defect. Do
not publish persist 311B as auction live. Crate path is now
`crates/netzip-fullpull/src/official_5188.rs` (mtime still 20:11). Did
not edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-night-dump-all-six.py`.
Details: layer-split section 62.

## 2026-09-04 21:06 - First-print close hits are not a public OEM quote; all_six=0

Read-only persist vs last OEM on 5,421 OEM-live persist-clean rows.
Zero rows match close+open+volume+amount+bid1+last together. The 5
first-prints-from-zero match close 5/5, last 5/5, open/volume/amount
3/5, bid1 0/5. Sample 000025 matches price/open/volume/amount but
bid1 is leftover −2.39e6 vs OEM 15.50; 002696 matches only close,
with leftover volume 8.45e6 vs OEM 3,780. Close-mismatch 4,627 still
has last_close 4,627/4,627 and amount 0. SH live 2,382 close-hits 5;
SZ 3,039 hits 60. Do not publish persist 311B as the auction quote.
Night dump remains the layer-C control. Did not edit official_5188.rs
or runtime. Script:
`scripts/coldstart/score-runtime-2023-first-print-fields.py`.
Details: layer-split section 61.

## 2026-09-04 21:02 - Auction 65 close hits: 52 already equal OEM on first frame

Read-only on the 65 persist-clean indexes whose close equals OEM live.
SZ 60 / SH 5. Later incoming close=0 is sparse-merge, not a price move.
52/65 already match OEM on the first wine-tail frame (9 also equal
morning 0104 last; 43 already at live ≠ last). 8 move from a nearby
nonzero (sample 002012 7.29→7.18). Only 5 go from close=0 to OEM
(sample 000025 0→15.51). That is 5/5,421 of OEM-live persist-clean.
Do not publish persist close as the auction print. Night dump 5,166
remains the layer-C control. Did not edit official_5188.rs or runtime.
Script: `scripts/coldstart/score-runtime-2023-auction-65-coincidence.py`.
Details: layer-split section 60.

## 2026-09-04 20:57 - Night dump persist close is 5166/5166; auction live remains 65

Read-only same persist replay on the 01:19 same-session dump. 5,171
publishable, 0 dirty, last_close 5,166/5,166. Every OEM-live row has
close equal to OEM price (5,166/5,166; SH 1,865 and SZ 3,301). Auction
persist-clean from section 58 stays 65/5,421. The night dump 311B is
the public OEM state; the wine-tail end-of-stream 311B is leftover vs
auction live. Do not treat the auction 65 as a bitstream or projection
defect, and do not publish persist close as the auction print.
Snapshot still has no quotes. Did not edit official_5188.rs or runtime.
Script: `scripts/coldstart/score-runtime-2023-night-dump-close-vs-oem.py`.
Details: layer-split section 59.

## 2026-09-04 20:55 - Persist-clean last_close is 5669/5669; auction live price is 65

Read-only on the 5,886 two-map persist-clean indexes (not the 236
rollback extras). last_close vs last OEM is 5,669/5,669. Of 5,421
OEM-live rows, close equals OEM price on only 65 (1.2%); 729 would
emit zeros and 4,627 mismatch. Sample 23830 is now the right ticker
600228: OEM 10.40 vs leftover close 48.27. Do not publish persist close
as the auction print. First-print remains public OEM live. Wine-tail
live misses are not bitstream. Snapshot still has no quotes. Did not
edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-persist-clean-vs-oem-live.py`.
Details: layer-split section 58.

## 2026-09-04 20:52 - Recovered last-Ok is schema-Ok, not the current OEM leftover/live

Read-only on the 236 indexes section 56 would republish after rollback.
229/229 with OEM match last_close. 223 are already OEM live: 76 would
emit amount=0 and close=0; 146 have a nonzero 311B that does not match
OEM price (only 1 close equals OEM price). Sample 512720 OEM 1.184 vs
rolled-back zeros; 600188 OEM 21.79 vs close 2.19. Do not publish
recovered last-Ok as the live quote. First-print remains public OEM
live. Wine-tail price misses are not bitstream. Decoder::new still
fills both maps from one code_tables slice. Did not edit
official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-recovered-last-ok-vs-oem.py`.
Details: layer-split section 57.

## 2026-09-04 20:48 - Split maps plus rollback: 0 wrong tickers and 0 dirty terminal states

Read-only combined recipe on the wine-tail. Two-map persist is 5,886
publishable / 275 dirty / 0 wrong / 5,669 last. Adding project-err
rollback is 6,122 publishable / 0 dirty / 0 wrong / 5,898 last. Of the
275 dirty terminals: 236 recover the last Ok 311B (sample 22906 keeps
12,963,405 instead of −100,525,162); 39 never had an Ok state and stay
absent. Snapshot is still not wired. Runtime still uses one table and
does not roll back. Did not edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-two-map-plus-rollback.py`.
Details: layer-split section 56.

## 2026-09-04 20:45 - Split the two maps: decode-session index→code plus morning-by-code last

Read-only recipe vs runtime 20:23 same-table maps. Wine-tail indexes:
6,186; 24 missing decode-session code; 1 missing morning-by-code seed;
6,161 have both; STAR prefix cross 0. End-of-stream persist would
publish 5,886 quotes with 0 wrong tickers and 5,669/5,669 last_close vs
true-code OEM (stale same-table was 4,377 wrong / 1,509 last). The 275
end-of-stream negative amounts are unchanged, so splitting maps does
not replace project-err rollback. Snapshot still has no quotes. Did not
edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-correct-two-map-snapshot.py`.
Details: layer-split section 55.

## 2026-09-04 20:41 - Wiring persist oem_states would publish self-consistent wrong tickers

Read-only hypothetical: ShadowSnapshot still has no quotes. End-of-stream
persist states on the wine-tail with the stale 09:15 map would emit
5,911/6,186 indexes (275 still negative). 4,377 of those publishable
quotes are the wrong ticker (74.0%). Published last_close matches the
mapped stock's OEM last on 5,884/5,911 (99.5%), so a last_close check
cannot catch the identity bug. Sample 23830 would show 600199 金种子酒
7.31 instead of 600228 / 10.28. Night same-session is 5,171/5,171 right
tickers and 5,166/5,166 last vs seq≥21 OEM. Do not wire snapshot until
index→code is decode-session 0104 and failed projects roll back. Did
not edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-snapshot-would-publish.py`.
Details: layer-split section 54.

## 2026-09-04 20:38 - Healing a negative amount can keep a dirty book; rollback recovers the 158

Read-only on runtime 20:23. The 23 dirty→Ok transitions on the wine-tail
include 12 heals that still carry leftover OHLC/book/volume from the
negative-amount record (sparse merge will not copy zeros). Sample 25185
keeps all ten book slots while amount becomes 194,790,854. Rolling back
a failed project is 15,118 updates / 387 missing: exactly +158 vs
persist, matching the zero-amount or timestamp-only rows section 52
left stuck. The remaining 387 are incoming rows that themselves stay
negative; do not send them to the bitstream. Night persist ≡ rollback
at 5,171/5,171. Did not edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-rollback-vs-poison.py`. Details:
layer-split section 53.

## 2026-09-04 20:32 - Negative-amount merge is not rolled back; zeros cannot heal it

Read-only on runtime 20:23 merge order. Full wine-tail replay is
15,505 decoded = 14,960 updates + 545 missing (conservation holds;
missing_index=0). The 545 are post-merge project failures after a
negative amount was already copied into oem_states: 116 first inserts,
182 later poisons of an Ok state, 247 later still stuck. 158 of the
stuck rows are incoming amount=0 or timestamp-only, so sparse merge
cannot overwrite the leftover negative. 29 later positive amounts heal
the gate and count as updates (23 indexes). Night same-session dump is
5,171/5,171 with 0 dirty. Do not treat the 545 as bitstream loss. Do
not read conservation as a clean oem_states. Did not edit
official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-dirty-merge-persist.py`. Details:
layer-split section 52.

## 2026-09-04 20:23 - project().is_err is a schema gate, not a session gate

Read-only on runtime 20:23: merge calls `project_from_code_table_metadata`
twice and discards the quote. ShadowSnapshot still has no PublicQuote.
Night same-session probe is 5,169/5,171 schema-Ok with 0 wrong tickers
(2 missing index). Stale 09:15 × wine-tail exact-second joins: 8,952/9,184
schema-Ok, and 6,098 of those still have the wrong ticker (3,414 unique
indexes). The only schema failures are 232 negative-amount 2704 rows;
70 of them were the right ticker. Those 232 pass the seed-state project,
merge into oem_states, then fail post-merge project with no rollback.
Do not read project-Ok or missing==0 as proof of decode-session identity.
Did not edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2023-project-gate.py`. Details: layer-split
section 51.

## 2026-09-04 20:18 - 0xe5 vs seed map is a session check, not a join key

Read-only on runtime 20:18: `reseed_code_tables` rebuilds the decoder in a
unit test only; ShadowReader has no reseed API. Treating six-digit 0xe5 !=
`symbol_codes[index]` as not-ready is 0 false-reject / 0 residual poison on
the night same-session probe (5,171). On stale 09:15 × wine-tail it rejects
6,267/9,184 (6,220 were wrong tickers) but still passes 40 convertible
near-misses where leftover 0xe5 equals the stale map. Do not join on 0xe5;
do not treat this gate as a substitute for session-bound 0104. Did not edit
official_5188.rs or runtime. Script:
`scripts/coldstart/score-0xe5-seed-map-gate.py`. Details: layer-split
section 50.

## 2026-09-04 20:14 - Stale index→code would publish the wrong ticker

Read-only on runtime 20:14: OemState is now constructed with
`from_code_table_metadata`, so record 0x12b is the mapped 0104 last, not 0.
project() is still not called. If it were, with the 09:15 table on this
wine-tail, 6,260/9,184 quotes would publish the wrong code and name (3,499
unique indexes). last_close would match on 2,929; identity from 09-03 plus
morning-by-code remains 9,184/9,184 for code, name, and last. Sample 23830:
600228 返利科技 10.28 vs published 600199 金种子酒 7.31. Did not edit
official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2014-metadata-seed.py`. Details: layer-split
section 49.

## 2026-09-04 20:10 - Two-step seeds from one stale table still hit 2929/9184

Read-only on runtime 20:10 and crate `project_from_code_table_metadata`.
Same-login 301 morning tables have 0 index/code conflicts (32,997 rows).
Feeding that table into the wine-tail via index→code→row last_close is
2,929/9,184 — identical to the old index key — with missing=0. Identity from
09-03 plus morning last by `(market, code)` remains 9,184/9,184. 21 joins
cross the 688/605 STAR prefix. Runtime still never calls project(). Did not
edit official_5188.rs or runtime. Script:
`scripts/coldstart/score-runtime-2010-two-step-seed.py`. Details: layer-split
section 48.

## 2026-09-04 20:06 - Do not join on the 2704 embedded 0xe5 code

Night same-session 311B preserves 0104 code/amount_mode/tail/last 5,171/5,171.
Auction wine-tail last-2704 code at 0xe5 matches the 09-03 identity on only
1,841/6,162 indexes. On the 9,184 exact-second joins, 09-03 index→code equals
OEM 9,184/9,184; the wire 0xe5 code equals OEM on 3,197/9,184. Sample index
23830: wire 600206, OEM/09-03 600228, 09:15 600199. Name is never in the
311-byte record. OemState.merge does not copy code/tail. Keep a same-session
0104 side map for project(code, name, scale). Did not edit official_5188.rs.
Script: `scripts/coldstart/score-resolver-overwrite-wipes-0104.py`. Details:
layer-split section 47.

## 2026-09-04 20:03 - Crate public_timestamp matches night OEM; join must not use wire u32

Read-only on crate 20:02: `to_public_quote` now emits `public_timestamp()` and
leaves `timestamp()` unchanged. Reimplementing that UTC+8 15:00:00 clamp on
the night probe∩seq≥21 set is 5,166/5,166 vs OEM datetime (raw wire 4,341)
and 0 disagreements with the section-44 datetime floor. 825 u32s change;
399 is only 39 of them. Auction wine-tail would floor 0/15,481. The parity
example still joins `index.timestamp` to callback datetime — fine for 09:xx,
825 misses on a close dump. Publish/OEM-datetime join uses public_timestamp;
bitstream merge keeps the wire u32. Did not edit official_5188.rs. Script:
`scripts/coldstart/score-crate-public-timestamp.py`. Details: layer-split
section 46.

## 2026-09-04 20:01 - project() needs the whole 0104 row, not only last_close i32

Runtime still stores only `(market, index) → previous_close i32` and never
calls `OemState::project`. On 9,184 exact-second auction joins, projecting
that i32 with the same-row `10 ** opaque_tail[0]` hits last_close 9,184/9,184
and names 6,183/6,183. Hardcoding scale=100 drops to 8,626/9,184 (unique OEM
5,329/6,183): the 854 misses are all `places=3` funds/bonds. Publishing
`oem_states.record` without `project()` is 0/9,184 because merge does not copy
0x12b. Publishing the resolver record after 2704 is 106/9,184 — leftover
overnight residue. Night seq≥21 unique OEM last_close is 6,189/6,189 with the
same-session 0104 row. Do not add 0x12b to merge; reconnect must rebuild
code/name/scale, not only the i32. Did not edit official_5188.rs or runtime.
Script: `scripts/coldstart/score-project-needs-0104-row.py`. Details:
layer-split section 45.

## 2026-09-04 19:51 - Floor the public timestamp u32; do not special-case 399

Night OEM datetime strings match 4,341/5,166 from the raw u32 and
5,166/5,166 after flooring Asia/Shanghai local time after 15:00:00.
825 u32 values change; only 39 are SZ399 (the >1s set). 688/603/funds also
overshoot by 1s and were hidden by the ±1s gate. Crate should replace
PublicQuote.timestamp, not the formatter. The service snapshot test still
asserts oem_state_updates == decoded_records, which 19:48 not-ready
breaks. Did not edit official_5188.rs. Script:
`scripts/coldstart/score-datetime-string-floor.py`. Details: layer-split
section 44.

## 2026-09-04 19:49 - Runtime not-ready skips empty tables; stale full maps still merge

Read-only: official_5188_runtime.rs 19:48 no longer calls OemState::new(0)
when a (market, index) seed is absent. On this wine-tail (0 same-session
0104 tables) that skips all 15,505 decoded records. Reusing 09:15 0104 by
index still merges 15,505/15,505 with 12,437 wrong-code last_close (80.2%)
and a zero missing-seed counter. Auction internal timestamps are all 09:xx;
0 need the 15:00:00 floor. Did not edit runtime. Script:
`scripts/coldstart/score-not-ready-vs-stale-full.py`. Details: layer-split
section 43.

## 2026-09-04 19:45 - Same-session index seeds are 5166/5166; datetime floor is exact

Night 01:19 same-session 0104 keyed by (market, index) matches probe codes
5,166/5,166 and OEM last_close 5,166/5,166. 0x12b equals 0104 +11 on this
overnight dump. Contrast section 41 stale-index poison. Crate still emits
raw `timestamp()`: exact 4,341/5,166; 825 locals after 15:00:00; flooring
those yields 5,166/5,166 exact with 0 false floors. Did not edit
official_5188.rs. Script:
`scripts/coldstart/score-night-same-session-index-and-datetime.py`.
Details: layer-split section 42.

## 2026-09-04 19:42 - Stale index seeds poison last_close without the missing counter

The 09:24 wine-tail extract has zero same-session 0104 tables and 6,162
unique (market, index) keys. Reusing the 09:15 login 0104 by index leaves
**0 missing** keys, so `unwrap_or(0)` and `decoder_missing_previous_close_seeds`
would not fire. 4,590/6,162 indexes (74.5%) map to a different code; none of
those share last_close i32. Runtime never calls `OemState::project` (crate
tests only). Reconnect must drop old seeds; do not treat a zero missing-seed
counter as correct last_close. Did not edit official_5188.rs. Script:
`scripts/coldstart/score-stale-index-seed-poison.py`. Details: layer-split
section 41.

## 2026-09-04 19:39 - Parity leftover×live special case equals max-sequence here

Read-only: callback_parity.rs mtime 19:36. On 9,184 exact-second joins the
only multi-candidate shape is 586 leftover×live pairs with identical
last_close; 8,598 are unique. The 19:36 special case and always-max-sequence
pick the same quote 9,184/9,184 and both emit 4,145 leftover vs nearest's
4,435 (290 leftover false picks). No leftover×leftover or live×live pairs.
Prefer landing max-sequence without a size-2 shape gate. Did not edit the
example. Script: `scripts/coldstart/score-join-1936-special-case.py`.
Details: layer-split section 40.

## 2026-09-04 19:55 - Decoded 2704 fields cannot predict OEM live

On the 9,184 exact-second joins, close/volume/amount/open/bid1/mask all
score ~0.44–0.54 precision as live predictors. 1,975 live OEM rows still
have 2704 close=0. Mask classes overlap leftover and live completely.
Wall-clock ≥09:25 is 5,039/5,039 live with 300 leftover false positives —
the same global gate section 33 already rejected. First-print is not in
the decoded 311B fields on this wine-tail. Did not edit official_5188.rs.
Script: `scripts/coldstart/score-2704-live-predictors.py`. Details:
layer-split section 39.

## 2026-09-04 19:50 - close≈0x12b must not be used as a leftover gate

On the 9,184 exact-second joins, treating nonzero +0x10 within 5% of live
0x12b as leftover would suppress 2,401/4,145 leftover rows (57.9%) but
**false-suppress 2,055/5,039 live OEM rows (40.8%)**. Sample 600150: internal
6.47 ≈ 0x12b 6.40 while OEM live is 34.50. Do not ship this heuristic into
join, OemState, or projection. Did not edit official_5188.rs. Script:
`scripts/coldstart/score-close-eq-12b-as-leftover.py`. Details: layer-split
section 37.

## 2026-09-04 19:50 - Crate 19:31 landed STAR lots; datetime and last_close remain

Read-only: official_5188.rs mtime 19:31. STAR cumulative and book volumes now
use integer half-up plus a 1-lot floor; non-STAR empty-size→1 is in. Remaining
vs night recipe v2: datetime is still the raw `timestamp()` (no 15:00:00
floor); `to_public_quote` last_close still reads 0x12b; missing 0104 seeds
still `unwrap_or(0)`. Parallel session did not compile or edit the owned
file. Details: layer-split section 38.

## 2026-09-04 19:45 - Wine-tail +0x10 does not leave 0x12b when OEM goes live

3,023 codes have both leftover and live same-second joins. Only 17
transition from 0x12b residue to OEM price. 952 stay on 0x12b after OEM
is live; 1,912 match neither. Across all 5,039 live joins, +0x10 is near
0x12b 2,055 times and near OEM 834 times (exact 46). First-print cannot
be inferred from 2704 leaving 0x12b on this extract. Did not edit
official_5188.rs. Script:
`scripts/coldstart/classify-leftover-to-live-close.py`.

## 2026-09-04 16:30 - Next-day last_close is 0104 settlement, not last trade

Morning 0104 last equals auction OEM last on 6,183/6,183. Night +0x10 close
equals that last on 5,146/5,160; the other 14 miss the callback and hit the
0104 settlement instead (603259 156.63 last trade vs 156.12 official last).
Login 0104 last is stable across 32,997 codes. Night dump also misses 1,023
ordinary SH A-shares that 0104 still covers. Do not snapshot live +0x10.
Script: `scripts/coldstart/morning-0104-vs-night-close.py`. Details:
layer-split section 22. Did not touch `official_5188.rs`.

## 2026-09-04 16:25 - Morning 0104 last_close is next-day OEM last

The 09:15 rust-auth pcap has five 0104 tables and zero 2704 frames.
`opaque_tail` i32@+11 matches auction callback last_close on 6,183/6,183
codes (SH603059 2468=24.68). The 01:19 table still has 2533 and hits 3.17%.
Day-cut last lives in the morning login 0104, not in live `0x12b` and not
in a mid-session 2704 close. Evidence:
`diagnostics/20260904-live-rust/ten-slot-0909/morning-0104-last-close.json`.
Details: layer-split section 21. Did not touch `official_5188.rs`.

## 2026-09-04 16:25 - Day-cut last_close cannot be snapshotted mid-session

Next-day OEM last_close still matches night 311B close at 4,914/4,928
(99.72%). The first non-zero close in the auction 2704 stream hits only
22/4,250 (0.52%); 3,440 codes have already moved by 09:25:01. Mid-session
`0x12b` is the T-2 0104 last (76 hits). Product must snapshot last_close
from the same-session initial dump, not from the first live increment.
Script: `scripts/coldstart/daycut-last-close-sources.py`. Details:
layer-split section 20. Did not touch `official_5188.rs` or the owned
parity example.

## 2026-09-04 16:20 - Frozen recipe projects 5,028/5,166 full OEM records

Applying the night recipe to the 311B dump and comparing post-dump OEM
yields 97.33% full-record parity (datetime ±1s, amount rel<1e-3, STAR lots).
The only fail sets are 97 STAR book-lot rounding, 39 pre-15:00 last trades,
and 2 cumulative volumes. Name/price/last/OHLC/amount/five-level prices/empty
levels 6-10 are 100%. Script: `scripts/coldstart/project-311b-to-oem.py`.
Details: layer-split section 19. Did not touch `official_5188.rs` or the
owned parity example.

## 2026-09-04 16:15 - Frozen 311B-to-OEM recipe on the night fixture

Name matches 0104/probe at 5,166/5,166. Datetime is the 311B timestamp in
Asia/Shanghai: 84.03% exact, 99.25% within 1s. The 786 one-second misses
are 15:00:01 internally vs OEM 15:00:00 close flooring. Recipe:
`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/oem-projection-recipe-v1.json`.
Details: layer-split section 18. Did not touch `official_5188.rs` or the
owned parity example.

## 2026-09-04 16:10 - Night OEM five-level book is 100% on prices

Same-session post-dump OEM: levels 6-10 are zero on all 6,189 quotes.
Five bid/ask prices match 5,166/5,166 using internal `+0x58` as
bid5..bid1, ask1..ask5. Book volumes are 98.12% after STAR `round(/100)`;
the remaining 97 are mostly 688 one-lot rounding. The 47 amount rows past
1e-4 all stay under 1e-3 with matching volume — still the f32 boundary.
Script: `scripts/coldstart/night-oem-book-and-amount.py`. Details:
layer-split section 17. Did not touch `official_5188.rs` or the owned
parity example.

## 2026-09-04 16:00 - Same-session night OEM matches the 311B dump

The 01:19 Wine callbacks contain 17 quote batches. Seq 16 is stale 2026-07-24
public state (negative control). Seq 21+ is post-dump OEM dated 2026-09-03
15:00. Using 0104 decimal places, 5,166 overlapping codes match price, OHLC,
bid1/ask1 and last_close at 100%: last is `0x12b`, price is `+0x10`. Amount
is `i64 as f32` within 1e-4 for 99.09%. STAR 688/689 OEM volume is
`round(internal/100)`. Overnight last and next-day auction last are two
day-boundary states, not two getters. Script:
`scripts/coldstart/night-oem-vs-311b.py`. Details: layer-split section 16.
Did not touch `official_5188.rs` or the owned parity example.

## 2026-09-04 15:50 - Live 914 is missing state, not an OEM amount formula

The 914 auction joins with Wine already live split as 677 wrong/incomplete
snapshots, 118 negative amounts (layer A), 96 internal price still 0, and
only 17 raw `i64 as f32` amount hits. The 7709 PriceRelative encoder adds
one extra record. Do not port 7709 amount modes onto 5188 OEM. The auction
pcap starts after baselines, so 09:25 Wine OEM has the open auction while
Rust 2704 is mid-session residue. Script:
`scripts/coldstart/classify-live-914-conversion.py`. Details: layer-split
section 15. Did not touch `official_5188.rs` or the owned parity example.

## 2026-09-04 15:40 - OEM last_close is yesterday's 311B close, keyed by code

Same-day 0104 exists in the 01:19 Wine pcap (32,639 rows) and still misses
callback last_close (181/5930). The 14:20 ~1% rate was an index-map join:
Wine night and the 09:24 Rust auction do not share symbol_index (only
4,634/32,606 codes stay put). Joining night 311B **close at +0x10 by
(market, code)** hits **4,914/4,928 (99.72%)**. Field `0x12b` and 0104
tail i32@+11 stay ~3%. Snapshot that close at day start; live +0x10 becomes
today's price. Zeroing trade fields before 09:25 turns the 597 pre-open
leftovers into 0=0 and does not move the live 914 conversion gap. Script:
`scripts/coldstart/seed-last-close-and-preopen.py`. Details: layer-split
section 14. Did not touch `official_5188.rs` or the owned parity example.

## 2026-09-04 14:20 - Public-state replay does not pass the 1511 amount≠0 gate

Capture-order merge (zero-skip scalars, sparse ladder, mask 0x18 timestamp-only)
plus business-second state-groups assigns all 586 multi-candidates uniquely, but
fingerprint collapse is 0 and field hits do not move. Of 1511 amount≠0 joins,
555 still see Wine price=amount=0 at that second; the live 956 stay at
price/amount/volume 22/17/24, identical to raw 311B. Same-day 0104 last_close
is missing (09-03 table and night 0x12b both ~1% vs callback last). Script:
`scripts/coldstart/replay-public-state-oem.py`. Details: layer-split section 13.

## 2026-09-04 14:05 - Exact business-timestamp join recovers unmatched, not OEM fields

Same auction partial-v1: joining on `internal_ts == callback datetime` matches
9127 records (250ms wall-clock: 6977) and recovers 2426 same-second pairs
outside the window. The 276 wall-clock-only matches have 0% timestamp
agreement — they are the wrong public state. Non-zero price/amount/volume
barely move (34/17/24). On the 1511 records with internal amount≠0, amount
hits 17 and bid1 both-nonzero hits 2. Layer B should switch to business-second
primary keys; layer C (OEM merge/conversion) remains open. Script:
`scripts/coldstart/join-by-business-timestamp.py`. Details: layer-split
section 12. Did not touch `official_5188.rs` or the owned parity example.

## 2026-09-04 13:51 - Widening the OEM join window recovers unmatched but mismatches state

250ms→5s drops no-window 8262→803 while timestamp hit-rate falls 95%→63%.
Non-zero price stays ~29-41. Do not enlarge the wall-clock window to pass
layer B. Post-09:25 still has only 29 non-zero price hits. Details: layer-split
section 11. Did not touch `official_5188.rs` or the owned parity example.

## 2026-09-04 13:05 - OEM ablation: unmatched is join; 0=0 inflates scalar hits

Auction partial-v1 × Wine callbacks: 8262/8285 unmatched are outside the
±250ms window. Price/amount/volume "hits" are almost all 0=0 (29/16/23 real).
OEM levels 6-10 are zero in this capture, so five- vs ten-level array
equality is not why book hits sit at 35-41; bid1 alone is 50/6977.
Script: `scripts/coldstart/ablate-oem-projection.py`. Details:
`docs/forensics/official-5188-layer-split-20260904.md` section 10.

## 2026-09-04 12:22 - Three layers: 311B memory ≠ OEM_REPORT callback

Do not merge today's auction callback misses into the `2704` bitstream
mustfix. Night cold start already matched Wine's in-memory 311-byte table
(5171/5171, including bid1/ask1). Commit-v5 book-array hits of 35–41 are
counted on records that **already decoded**, using a 10-slot OEM compare
against a 5-level projection and a ±250ms join. ACK 62B is the short-manifest
server reply, not a wrong opcode. Write-up:
`docs/forensics/official-5188-layer-split-20260904.md`.

## 2026-09-04 01:20 - Night Wine cold start: 0104 seed is not the 2704 baseline

H4 in `docs/codex/tasks/cold-start-parity-plan-2026-09-04.md` is **rejected**.
Wine's global table after a night cold start (01:19 supervisor restart, PID
1054222) holds 0104 metadata plus the server's initial `2704` dump; the dump
is decoded from a **fresh/zero slot**, not by treating 0104 昨收/涨跌停 as the
`mask&1` baseline.

Discriminating replay (`examples/official_5188_coldstart_probe.rs` mode 0 vs 1)
on tonight's capture, compared with `/proc` 311-byte records at t+150s
(`scripts/coldstart/compare-probe-with-memory.py`):

- mode 1 (0104 inserted as baseline, strict decoder): negative control, does
  not match Wine memory.
- mode 0 (0104 metadata only + `decode_official_5188_values_with_fresh_fallback`):
  38 frames / 5171 records / 0 missing Wine slots; first+second+later buckets
  all matched ts/OHLC/volume/amount/昨收/bid1/ask1
  (`parity-vs-memory-t150.txt`).
- t+45s vs t+150s business bytes were identical, so the night table is a
  connect-time dump, not a later fill.

Provenance: this Cursor thread plus Codex `01a05a9a-92c9-7813-9649-70d7483eaf0e`
(cwd `quoteNetzipRs`, 「继续核对本项目与 quoteNetzipWine」). Product shadow
already uses the fresh-fallback decoder; publish stays
`opaque-evidence-only` until 09:25 same-symbol Wine `OEM_REPORT` parity.

Do not treat 09:37 mid-session "0104 seed produced wrong prices" as a
contradiction: that capture started after the dump, so a fresh slot is the
wrong history. Strict `requires missing baseline` remains correct for
mid-stream replay.

## 2026-09-03 - Official 5188 interleaved initialization succeeded; live business push frames verified

webClx deploy `133840-18d1a65e45a96d02` installed `/home/bin/netzip/quoteNetzipRs` (sha256 `4eabda34d7…`).
Subsequent service testing revealed and verified:

1. **ACK-Stage 3610 Length Dynamic Variance**:
   - In `quoteNetzipRs/src/auth_7100_client.rs`, `Official5188ControlStage::accepts_payload_len` initially hardcoded `44 | 67` bytes for the ACK-stage `3610` response from the 7100 control connection.
   - Live execution with default ACK rows returned a 62-byte `3610` frame (`大智慧C_ACK control response requires one 67-byte 3610 payload, got 3610 with 62 bytes`).
   - Aligned the validator with the authoritative crate `netzip-fullpull`'s range check `(40..=80).contains(&payload_len)` (which accounts for manifest length scaling: 67B for 1387B manifest, 63B for 1103B, 62B for default rows, 44B for 592B).
   - Recompiled and deployed release binary (`13:45:13`, PID `1974319`).

2. **End-to-End Handshake & Live Push Receiving**:
   - `POST /api/auth/login` established the 7100 control session and extracted 4 active 5188 quote endpoints.
   - `POST /api/fullpull/official-5188/connect` completed the full interleaved handshake:
     - 7100 control exchange for `登录包` (95B), `ABK` (94B), `ACK` (62B).
     - 5188 channel interleaved exchanges (`3110` / `3210`), `0104` code tables for 4 markets (SH, SZ, B$, etc.).
     - Triplet client responses and primary subscription partitions generation (`2a10`).
     - Resulted in `initialized: true`, `code_table_count: 4`, `post_initialization_frame_count: 8`.
   - `GET /api/fullpull/official-5188/status` confirmed state transitioned to `lane: "initialized-receiving-business-frames"`.
   - The shadow reader actively received continuous live data streams:
     - 93+ frames (>144 KB) within seconds, 0 dropped frames, 0 connection errors.
     - Confirmed rich payload types: `2704` (tick/depth deltas, 58+ frames), `3e04` (bulk market snapshot), plus `0310`, `0d04`, `1b04`, `2104`, `3001`, `3a01`, `3f04`, `4104`, `4804`, `5104`, `5404`.
   - Proved that the 5188 push protocol handshake in Rust is fully functional on live production endpoints.

## 2026-09-03 - Formal 7100 login succeeded; 5188 TCP is up without init

webClx deploy `132154-18d1a65e45a96d01` installed
`/home/bin/netzip/quoteNetzipRs` sha256 `7d5268f501313c9f…`. Formal
`POST /api/auth/login` (account class 1522) returned:

- `login_success_confirmed=true`
- `response_roles=["auth_login","download_file"]`
- `response_packet_lengths=[438,1880]`
- `active_server_count=4`
- `selected_quote_endpoint=222.85.139.177:5188`
- `control_session_retained=true`
- `selected_auth_endpoint=121.41.70.217:7100`

`POST /api/fullpull/official-5188/connect` opened that endpoint. Lane is
`connected-awaiting-initialization`; the service still has no production
interleaved init/subscribe, so this is not official 5188 full-push delivery.
Probes on 6100/7100 both saw `auth_login` instead of `auth_probe_response`
and were recorded as probe errors; they did not block the subsequent login.

## 2026-09-03 - Dictionary-envelope login success was mislabeled as zstd_dictionary

After the 19-field login and WouldBlock retry landed, formal `POST /api/auth/login`
failed with `unexpected login_read response: zstd_dictionary, expected auth_login
or auth_probe_response`. The crate already accepts the first control packet as a
`认证` object even when it is dictionary-compressed (`field20=4`, `field44=12`).
The product classifier returned `zstd_dictionary` from the envelope alone, so
`login_read` aborted before `decode_login_object` ran. Tdx_Encrypt packets still
lack `登录成功` and remain `zstd_dictionary`.

## 2026-09-03 - Formal login os error 11 is a read timeout plus a stale login template

`POST /api/auth/login` failed with raw `Resource temporarily unavailable (os error 11)`
for both the formal account and test account `168`. TCP connect to
`121.41.70.217:6100` and `:7100` succeeded in ~35 ms and produced no unsolicited
bytes. Linux `SO_RCVTIMEO` reports a blocked `read_exact` as `WouldBlock` /
EAGAIN, so the previous connect-only retry could not see the real failure.

The resident `connect_auth_sequence` was still sending the August hex-spliced
login template, then waiting for `zstd_dictionary` after a finish packet. The
verified live chain in `netzip-fullpull` uses the 19-field `认证/请求登录`
manifest, logs in on 7100, and treats L1 `download_file` as sufficient. A
single-connection 19-field login write received a server reply in under 100 ms,
so the hang was the stale template / extra finish-read, not host FD exhaustion.

The client now builds the crate's 19-field packet, retries WouldBlock/EINTR on
connect/read/write until a deadline, emits credential-free `stage/endpoint/kind`
diagnostics, keeps both probe-interleaved dictionary sequences, and defaults
the service login port to 7100. Official 5188 remains
`pending-production-wiring` until authenticated 5188 initialization and live
business frames are confirmed.

Compile queued as webClx `123215-18d1a65e45a96cf5`.

## 2026-09-02 - Correct initial authentication protocol layering

The formal `vendor_pm` flow matrix contains a 632-byte client packet that
decodes to an 854-byte `认证` object with 19 fields, followed by a 436-byte
15-field `认证` response and then the dictionary-backed 12-field `加密包`
per-5188 initialization packets. The Rust entrypoint now sends the ordinary
ZSTD 19-field manifest for initial authentication and requires the 15-field
success response (`提示信息=登录成功`). The 12-field builder remains available
only for the assigned 5188 connection lifecycle. The focused capture-shape
regression passed in webClx request `010044-18d1330bb455c509` (log:
`/home/bin/webclx/logs/quoteNetzipRs/3015_build.log`).

The external claim of byte-identical 854/462-byte reconstruction is consistent
with the local shape evidence but is not promoted as a byte-level invariant:
the current pcap fixture does not expose a clean standalone compressed payload
fixture suitable for an exact comparison. The next proof is a sanitized raw
frame fixture plus current formal-account acceptance. A prior test that rebuilt
the TCP stream directly was removed after its retransmission handling produced
a false ZSTD corruption failure.

## 2026-08-07 10:35 +08:00 - Rebuild 6100 follow-up requests from their decoded number

### Phenomenon And Root Cause

The formal login path replayed captured 271-byte and 333-byte follow-up packets. Offline decoding
showed that they were not opaque session ciphertext: all 26 retained `field44=12` samples decode
with the verified raw-content `Stock.字典`. Within each request family, the only decoded change is a
little-endian `编号` value at offset 124 for C2 and offset 116 for C3. Patching compressed bytes
directly was unsafe because a C3 number change can alter most compressed bytes and change the frame
length from 159 to 160 bytes.

### Change

- Decode the retained C2/C3 template, patch only its request number, and recompress with the
  verified dictionary at ZSTD level 3.
- Recalculate packet length, compressed length, decoded length, and object-span length in the
  follow-up outer envelope.
- Keep initial numbers 1 and 2 in the login flow so its wire bytes remain identical to the accepted
  captured baseline; expose builders that can generate later numbers.

### Verification And Boundary

- `auth_7100_client::tests`: 10 passed, including byte-for-byte equality with both captured
  templates and decoded round trips for advanced numbers.
- Representative sample hashes and varying offsets are recorded in
  `docs/forensics/7100-auth-20260806/crypto_transform_notes.md`.
- No credential was loaded, no login was attempted, and no production process or service changed.
- Dynamic construction is proven offline for the retained 271/333/334-byte family. The fresh
  267-byte variant, live acceptance of advanced numbers, reconnect/failover, and 7709
  `Tdx_Encrypt` equivalence remain open.

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

## 2026-08-31 - Prove the 7709 endpoint pool, per-shard failover, and restoration

The native full-push worker count now converges to the 53 subscription shards. A controlled
two-endpoint deployment placed an unreachable endpoint first; the complete session reported exactly
53 endpoint failovers, 53 reader failures, and 53 reader recoveries, with zero publication, audit,
Beijing-poll, TCP, HTTP-fallback, or ACK failures. This proves each shard can rotate independently
to the next endpoint without stopping downstream publication.

After restoring the built-in five-endpoint configuration in deployment
`112317-18d09f02146d4728`, the next complete session reported `endpoint_pool_size=5`, zero failovers,
53 workers and 53 shards, and zero reader/recovery/publish/audit/Beijing-poll failures. Main,
Beijing, and manual lanes completed 622, 116, and 228 TCP successes respectively with no fallback.
Socket inspection found all 53 full-push worker connections on the default first endpoint
`120.195.71.160:7709`.

Do not equate transport acceptance with Wine replacement. The same post-restore quality report had
zero peer-comparable samples and `can_replace_all=false`; Rust event-to-arrival p50/p95/p99 was
21.187/45.192/45.314 seconds versus Wine's 2.002/4.970/5.443 seconds. Keep Wine until the evidence
gate passes and representative live latency is competitive. A gateway `ingest_failures` increment
during deployment is acceptable only when repeated snapshots prove the counter stops increasing,
errors remain empty, and receive sequence/timestamps continue advancing.
# Terminology notice (2026-09-01)

## 2026-09-01 - Keep 0547 outside the official full-push constructor

The authenticated 5188 transport previously rejected only 7709. Since the
project treats 0547 as the companion supplementation protocol, that allowed an
invalid endpoint to pass the constructor's boundary check before connection
failure or protocol confusion. The constructor now rejects both numeric ports
7709 and 547, with a focused regression. This is a boundary guard, not evidence
that the 5188 business decoder or authenticated session binding is complete.

Fresh verification after the guard passed all 63 `netzip-fullpull` tests, the
workspace library tests, and the ignored `vendor_pm.pcapng` replay. The replay
continues to show reconstructable 5188 transport with unknown frame kinds and
trailing bytes; these remain raw evidence and do not justify a field decoder.

The session layer now exposes `receive_until_disconnect`, which repeatedly
decodes complete application frames and surfaces peer closure to the caller.
A local split-write TCP fixture passed, proving callback ordering across read
boundaries without introducing a silent supplementation fallback. Live
authentication binding and reconnect behavior remain `needs-verification`.

The transport now has an explicit same-endpoint `reconnect_authenticated`
operation. It discards incomplete bytes from the closed connection and requires
the caller to send the captured Wine initialization again. A local two-connection
fixture passed; service-level authentication binding and retry/backoff policy
remain open.

Client-direction extraction later rejected the combined
`reconnect_with_wine_initialization` design. The 95-byte `3610` frame varied on
every complete connection, `2d10` had two connection groups, and every `2a10`
subscription differed. The helper was removed before service integration.
Reconnect may clear transport state, but current-session initialization must be
generated again rather than replayed from a capture.

Eight README examples still labelled a 7709 host as
`<authenticated-fullpull-host>`, contradicting the authority ledger. They now
use `<supplement-host>`, and a repository test rejects any recurrence. Endpoint
examples are part of the ownership boundary, not harmless placeholder wording.

Historical entries below use `full push`, `native push`, and `full-push service` for a resident 7709
worklist scanner. Under the current authoritative boundary, that path is a 7709 supplementation or
transition publisher. Official full-push means quoteNetzipWine's formal-account, post-login,
non-7709 server-driven chain. See `docs/capability-boundaries.md`. Historical command names and
observations are retained as recorded evidence, not current product terminology.

# Build callbacks are not behavioral test evidence

- A webClx compile note may name a behavior while the recorded command is only
  `cargo build --release`. Treat that callback as build evidence only.
- Requests `141417`, `141929`, `142307`, `142435`, and `142757` all had this
  shape. Request `180303-18d115af25cb0404` subsequently ran the five named
  regression tests explicitly and passed them.
- For protocol conclusions, cite the command and individual test result from
  the build log, not only the request note or successful release status.
# Mutable Wine callback fixtures are explicit forensic tests

- Tests named `current_wine_*_callback_matches_*` compare a dated captured
  callback against files in the live staged Wine data directory. Wine can
  update those files independently, so the two inputs are not an atomic
  snapshot and cannot be a deterministic default workspace gate.
- Keep these comparisons as explicitly ignored forensic tests. Run them only
  after taking a synchronized snapshot of the callback fixture and all source
  data files. Stable synthetic and checked-in fixture tests remain enabled by
  default.
- WebClx request `173604-18d115af25cb03fe` demonstrated the race: the shared
  `netzip-fullpull` tests passed, while live finance/file/realtime/split inputs
  no longer matched their dated initialization callback files.

## 2026-09-02 Wine 主站掉线根因与修复（决定性）

- 现象：正式账号冷启动后「股票主站登录成功」约 3–70 秒后被断开，回落股票备用；
  早先多轮抓包窗口 ≤60s 掩盖了此规律（formal-primary-0006 在 ~65s 本地 FIN+RST）。
- 根因（脱敏 Ask 标记 + pcap + 事件时间线证实）：
  quoteNetzipWine `DllRuntime::initialize()` 默认发送的旧式同步
  `Ask(请求=登录&模块=认证&…&等待=10000&编号=0)` 与网际风自身的配置自动登录
  (用户/配置文件.ini 自动登录=1) 冲突/超时，返回 0 后 vendor 立即断开股票主站并
  切备用，还把配置文件写回 `登录股票主站=0`；之后若不经 set-vendor-login-primary
  直接 restart，就只连备用（多数白天 run 的直接原因）。
- 判别证据：
  - ask-diagnose-1931：Ask returned 0 […模块=认证…编号=0] 后 24ms 断开主站；
  - init-none-primary1-1952：`登录股票主站=1` + 跳过旧式登录 Ask 后，10 条
    58.16.134.228:5188 保持 ESTAB，150–237s 持续推送、零断开，effective
    login_primary=true / login_backup=false。
- 修复：`initialize()` 默认 `QUOTENETZIPWINE_INIT_LOGIN_MODULES=none`（只发只读
  初始化 Ask，主站/备用登录交由网际风按配置文件自动登录）；`run-host-wine.sh`
  白名单传递该变量；保留 auth,backup 显式开关。Ask 诊断带脱敏命令标记。
- 操作注意：`pkill -f quoteNetzipWine.exe` 会误杀命令行含该串的脚本自身；
  清进程须按 /proc cwd + comm 精确匹配。启动链经 systemd supervisor，
  实验 env 需 `systemctl set-environment`（或写 env 文件），命令行 env 不进入服务。

## 2026-09-02 Wine 5188 收盘后保活 = 服务器 0x0139 心跳（无业务数据）

- 收盘后主站 5188 唯一持续帧是 kind `0x0139`（wire `3901`），payload 恒 2 字节 `00 00`，
  每 ~0.8–1.2s 一条（closing-parity-2020：290 帧/180s，10 条连接均如此）。
- Wine 端同时有 6187 只全市场 quote_batch 周期重放（180s/2700 批），是本地 OEM 缓存
  周期回调，不是网络新帧——**不要**把它当 5188 业务解码对照真值。
- Rust 已补 `Official5188Kind::SERVER_HEARTBEAT` 常量 + 方向分类 + 测试；
  心跳是维持长连的信号，上层收到应忽略，不得当 decoder 输入或“行情更新”证据。
- 业务字段 parity 只能在开盘窗口抓（收盘后无增量 2704/3e04 新帧）。
### 2026-09-03 15:05 - Wine amount mode adjustment replay

- Static disassembly of Wine `0x449df0..0x44a1a0` confirms that the amount
  prediction path uses `10^metadata[0x2d]` and the metadata multiplier at
  `0x2e`. Mode `0` jumps directly to token decoding with a zero prediction;
  other modes first calculate the volume/price projection. For mode `0` or
  `8` with a non-zero adjustment at `0x30`, Wine then transforms the token
  result as `(result + 1) * (adjustment + 1)`.
- Rust now models this branch and has a focused mode-8 regression test.
- Fixed Wine replay after this change: `1625/3048` 2704 frames decoded,
  versus `1627/3048` before the branch. The accumulator failure bucket rose
  from `650` to `652`; no production promotion is justified. Keep the
  business decoder `opaque-evidence-only` until metadata/state provenance is
  corrected and same-symbol callback parity improves.

Follow-up differential replay isolated four changed frames: three mode-0 or
metadata-corrupted records changed from success to accumulator negative-value
rejection, while one previously failing record decoded successfully. This
confirms that the cross-date core snapshot does not provide the exact
temporary/public state needed by the mode branch; do not weaken the
non-negative guard or promote the branch based on this fixture alone.
