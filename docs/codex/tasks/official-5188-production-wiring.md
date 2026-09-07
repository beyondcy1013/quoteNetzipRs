# Official 5188 production wiring

Status: exploration; not complete.

## Latest offline verification (2026-09-02 20:40, fixed-accept-2000)

- Five primary 1024-entry partitions are now byte-for-byte verified against a
  stable primary session: five connections (60844/60860/60872/60882/60890)
  each sent one 2a10 partition equal to the Rust model partition 1..5 built
  from that connection's own SH/SZ 0104 tables (eligible 5214 >= 5120).
- Two additional dynamic partitions are precisely located and NOT covered by
  the five-partition model:
  - P6: one connection (60894) sends 1024 mixed entries (SH578+SZ446), with
    930/1024 non-primary codes (SH 000xxx index/other, SZ 302...).
  - P7: one connection (60904) sends 59 SZ entries (159xxx fund/ETF band).
- Required next step: reconstruct P6/P7 rule (likely leftover eligible slices
  and a SZ fund/ETF independent rule) and verify across a second lifecycle
  before production wiring can claim full-market subscription coverage.

## Current checkpoint (2026-09-02)

- Authenticated login, L1.ini 5188 discovery, interleaved login/ABK/ACK stages,
  and receive-only shadow observation are implemented and deployed as research
  capabilities. The shadow reader sends no initialization/subscription frames
  and publishes no quotes.
- Two independent lifecycles confirm seven `2a10` partitions encoded as
  `market[2] + symbol_index(u32 LE)`. A third lifecycle changes the small
  partition from 58 to 59 entries, proving that the discrete set is dynamic;
  historical subscription payloads are not production defaults.
- The third timestamped lifecycle proves that each connection receives SH/SZ/B$
  `0104` code-table headers before sending `2d10 x3`. Those headers provide the
  connection group and per-market version timestamp; the client triplet is a
  deterministic field-for-field acknowledgement. Runtime code no longer needs
  to guess `word2` or A/B assignment. The shared crate now retains all four
  decoded `0104` tables and generates the five verified primary partitions;
  Native still has no per-slot initialization job and remains receive-only.

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
2. Generate each current-session client `3610`, validate the server `3110` and
   `3210` stage responses, and construct `2d10 x3` plus all seven `2a10` sets
   only from independently established runtime rules. The five primary sets
   are now implemented; dynamic sets 6/7 and per-slot assignment remain open.
   Captured frames remain shape fixtures only.
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

The early primary Wine captures proved `login_primary=true` but contained only
`3901` control/keepalive frames. New `formal-primary-0006` and `cold-sync-2311`
lifecycles contain the complete `3210 -> 2d10 x3 -> 2a10 -> 2704` sequence, so
official 5188 business delivery is now proven. Production promotion remains
blocked by Native per-slot wiring, dynamic `2a10` sets 6/7 and their assignment,
reconnect behavior, and same-symbol/same-timestamp 2704-to-Wine callback field
parity, not by absence of business frames or by `2d10` derivation.

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

The 6100 lead was resolved with the vendored `Stock.字典` in webClx requests
`064045-18d1330bb455c578` (`3105_build.log`) and
`064353-18d1330bb455c579` (`3106_build.log`). `pcap_reassemble` now supports
DLT 276 as well, so both directions were deduplicated and reconstructed from
the same capture without manual byte offsets:

- client to server: two 586-byte packets, each decoding to an 848-byte,
  19-field `认证` object whose request is `心跳包`; account and password fields
  are redacted by the analysis tool;
- server to client: two 328-byte packets, each decoding to a 308-byte,
  7-field `认证` response whose request is `心跳包` and source is `认证服务器`;
  the remaining fields are answer number, current login count, total/current
  bandwidth, and account expiry;
- neither direction contains a security code, quote ladder, trade value,
  `0104`, or `2704` object.

Therefore this formal fixture's 6100 traffic is authenticated heartbeat
control traffic, not a second full-push business transport. It must not be
fed to the 5188 decoder or used to clear the production business-parity gate.
The reusable `official_6100_decode` example accepts complete packets or a
reassembled stream and calls the shared crate's bounded
`auth_7100::decode_stock_dictionary_netpacket` API.

## Callback-window value parity checkpoint (2026-09-02)

`examples/official_5188_callback_parity.rs` now provides a repeatable offline
comparison between an `official_5188_extract` directory and Wine callback
JSONL. It resolves every record through the captured `0104` table, assigns it
to the nearest callback batch within a bounded time window only when
`market+code` is present, compares numeric fields by IEEE-754 `f32` bits, and
reports timestamp differences instead of silently accepting them.

The formal fixture replay completed through webClx request
`070921-18d1330bb455c57b` (`3108_build.log`, status 0). With a 250 ms window:

- all 2,102 decoded records resolved to `0104` metadata;
- 1,792 records matched callback sequences 34, 35, or 36 by time and code;
- sequence 34 matched all 1,059 callback symbols and sequence 35 matched all
  436 callback symbols; sequence 36 matched 297 of 445 callback symbols;
- names matched 1,792/1,792, but exact field hits remained incomplete:
  `price=355`, `last_close=1380`, `open=441`, `high=411`, `low=387`,
  `volume=377`, `amount=92`, `ask_prices=333`, `bid_prices=321`;
- 974/1,792 internal timestamps exactly matched callback local time converted
  to Unix seconds; the other records include the observed close-time `+1s`
  internal value and remain explicitly mismatched.

`SH603059` is assigned to sequence 34, not the later sequence 54 snapshot.
Its price, last close, OHLC, and volume match, while the decoded record has
`amount=107380`, incomplete ladder state, and timestamp `1788246001`; the Wine
callback has `amount=25934334`, a complete five-level ladder, and local
`2026-09-01 15:00:00`. The retained core also contains both a metadata-only
slot and a complete public-state slot for this identity. This rules out batch
54 comparison as the sole cause and shows that a decoded 311-byte `2704`
record is not yet a publishable quote by itself. The next parity slice is the
Wine public-state/OEM-report merge after `0x44aa30`, not an arithmetic patch in
`to_public_quote`. The production decoder gate remains closed.

## Fresh pcap amount-prediction and baseline checkpoint (2026-09-02)

The stale decoded-directory rerun was superseded by webClx request
`073904-18d1330bb455c57f`, which rebuilt
`/tmp/official-5188-rust-value-replay-amount-prediction` directly from
`formal-primary-0006` and then compared against the paired callback JSONL.
The decoder now reconstructs the SH603059 internal amount as `25934317`; the
previous stale replay value `107380` is therefore fixed. Wine still exposes
`25934334`, so the known OEM `f32` conversion remains separately open. The
fresh report had `decoded=2102`, `metadata=2102`, `matched=1792`, and
`amount=83`.

A core-layout experiment confirmed that Wine memory contains both a
metadata-only slot and a complete public slot for the same
`(market, symbol_index)` identity. Accepting the timestamped complete slot in
the existing scan reproduced the first 166-record frame byte-for-byte, but it
selected only 2,099 baselines versus 2,102 and reduced full-capture parity to
1,943 decoded records. The extractor therefore keeps the original conservative
metadata-only scan and records complete-slot selection as a separate lead
requiring explicit slot-table or capture-session segmentation. Separately, the
callback tool found that a Wine batch can contain duplicate market/code rows.
The old batch-level `HashMap` selected an arbitrary duplicate and distorted
per-symbol parity. The parity
tool now compares each decoded row against every matching quote in the same
bounded window and keeps the closest duplicate. The aggregate parity values
remain unchanged, and SH603059 still differs by the known callback amount
conversion plus its partial public-state fields; remaining close-time and
public-merge mismatches are not resolved.

The follow-up webClx full workspace regression passed through request
`074659-18d1330bb455c580` (`3113_build.log`, status 0): 124 shared-crate
tests and the complete quoteNetzipRs workspace targets, including 93 lib
tests, 65 service tests, and both extract/parity example tests. Local checks
also passed `cargo fmt --all --check` and
`cargo check -p netzip-fullpull --all-targets`. Production remains gated at
`lane=pending-production-wiring` and
`business_decoder=opaque-evidence-only`.

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

## Wine amount-branch static checkpoint (2026-09-02)

The PE32 Wine binary was disassembled at `0x449df0..0x44a1a0`, the amount
accumulator called from the `0x44aa30` public-state merge path. The branch
selected by value mask `0x80` first chooses the baseline/non-baseline token
tables (`0x5b2628`/`0x5b25f0`), then reads the metadata tail through the record
pointer at `current+0xf3`:

- `metadata+0x2c` is the mode byte;
- `metadata+0x2e` is a 16-bit multiplier;
- `metadata+0x30` is a 32-bit adjustment value.

The branch performs two signed 64-bit divisions and an auxiliary helper call
before adding the baseline amount. When the mode is zero, or mode eight with a
non-zero adjustment, it increments the adjustment and adds it to the
intermediate amount (`0x44a139..0x44a15c`). This explains why the current Rust
prediction can be close while still missing the SH603059 callback amount by
17 (`25934317` internal versus `25934334` Wine). The exact scale and helper
semantics are not yet established from a paired record, so no adjustment is
applied in `amount_prediction` until a fixture proves the formula. The
production lane remains `pending-production-wiring`.

The same SH603059 sample isolates the ladder merge boundary. The retained
complete public slot has price integers
`[2505,2508,2509,2510,2511,2517,2519,2520,2525,2530]` and volumes
`[19,14,8,22,2,16,12,2,44,25]`, which project exactly to the Wine callback's
five bid and five ask levels. The decoded 2704 temporary record for the same
identity has mask `0x80`, `clear_ladder=true`, prices
`[2514,2515,2516,2517,2518,0,0,0,0,0]`, and volumes
`[10737,13,292,9936,24972,0,0,0,0,0]`. Therefore the mismatch is not a
price-scale or array-order issue: the post-`0x44aa30` public-state copyback
must select/merge the global slot after the value pass. This pair is now a
fixed regression fixture for that merge; the value decoder's internal output
remains unchanged until the copyback state machine is reconstructed.

The shared crate now exposes `to_public_quote_with_public_state`, an explicit
projection boundary for this merge. It keeps scalar fields from the decoded
2704 record and takes only the ten-slot ladder from a caller-selected complete
public-state record; it does not select a baseline or enable production
publishing. On the formal replay, applying this explicit retained-slot
selection to 574 same-window records improved ladder price/volume field hits
from 236 to 705 with no observed regressions. This is evidence for the merge
boundary and a reusable API contract, not a production parity sign-off.

## Accepted parity differences (2026-09-02)

## Open-market restart capture (2026-09-02 13:25-13:41)

- Capture directory: `diagnostics/20260902-live-pair/open-market-132540/`.
- A full-port packet capture was started before the Wine restart and retained
  approximately 927 MiB of traffic. The restart deployment failed its health
  gate (`quoteNetzipWine` request `133434-18d1330bb455c5a4`).
- The first failure (request `132650-18d1330bb455c5a0`) was the PE32 config
  `ReplaceFileW` sharing violation. The temporary skip gate removed that
  error, but the next run reached `Stock DLL Start(callback) returned 0`.
  Wine local proxy ports `16801-16805` were listening, while the adapter
  process never became healthy. This is startup-order negative evidence, not
  a protocol or decoder result.
- Rust `pcap-5188-summary` over the full-port capture reports zero 5188 flows;
  the callback file contains only 200 `股票数据` events from the bounded
  polling endpoint. No input/output parity claim may be derived from this
  window. The next live capture must first prove `Start(callback)` success and
  retain a complete callback cursor window before comparing 5188 frames.

For the current offline parity investigation, the following two known
differences are recorded and intentionally ignored as non-blocking:

- amount: Rust `25934317` versus Wine callback `25934334` on the SH603059
  fixture;
- timestamp: the observed closing-session internal value is `+1s` relative to
  the Wine callback time.

These exclusions do not waive the remaining production gates: verified
public-state ladder merge, authenticated live delivery, reconnect behavior,
representative coverage, and callback parity for the other fields.

## Native opt-in scaffold checkpoint (2026-09-02 17:0x)

- `netzip-drivers/src/native.rs`（netzip_win GUI 原生驱动）新增一个显式实验
  开关 `NETZIP_NATIVE_5188_INIT=1`：启用时在保留的 7100 控制 socket 上调用
  `Auth7100ControlSession::initialize_official_5188_interleaved_with_code_tables`，
  随后从同连接四张 `0104` 动态生成 `2d10 x3` 与五个主 `2a10` 分区并发送。
  默认不设置开关时保持 receive-only shadow（不发送任何初始化字节）。
- 该脚手架按净改动口径验证：webClx `170653-18d1330bb455c5b9` /
  `3157_build.log`（fmt、workspace tests、strict Clippy、release 全绿）。
- 在线启用该开关仍有两个已知阻塞，**不得在未解决前把开关当作生产接线**：
  1) 登录阶段 `3610` 需要当前会话字段值（本地IP、账号权限、券商等）；当前
     脚手架用零值构造，服务器按证据会回 94B 而不是登录阶段要求的 95B，
     该路径预期会以清晰错误失败，属诊断实验而非成功路径。
  2) 7100 控制 socket 的下载/L1 阶段已消耗编号 1、2；每连接登录控制请求的
     编号选择与 Wine 的 `2..11/ABK 28,29/ACK 30,31` 对应关系未独立验证，
     复用保留 socket 时可能编号冲突。
- 动态第六/第七 `2a10` 集合、per-slot 编排、重连、2704 业务解码与 callback
  parity 仍全部未接入；服务发布入口仍是 7709 补数 worker。生产 full-push
  门禁保持关闭。

## L1 运行时字段供给（2026-09-02 19:5x，修正上面 17:0x 条目）

- 上面 17:0x 条目所列阻塞 1（登录阶段零值字段→94B 非 95B）已由运行时字段
  供给修复：fullpull `DownloadedServerEntry` 现保留 `大智慧服务器L1.ini` 的
  券商/账号权限/接口版本；`Auth7100ControlSession::current_login_control_fields`
  从保留登录结果的 5188 L1 路由条目供给当前会话登录字段，无匹配即
  fail-closed，不落回抓包静态值。本地 IP 由调用方 UDP 探测。
- netzip_win `native.rs` opt-in 路径已改用该运行时字段；新增端到端回归
  `runtime_l1_fields_flow_into_the_login_control_packet`（L1 字段→448B 登录包
  解码校验）。webClx `195131-18d1330bb455c5cb` / `3165_build.log`：fullpull
  157 tests、workspace、strict Clippy、release 全绿。
- 剩余阻塞收窄为：7100 控制 socket 编号复用（下载/L1 已用 1、2，每连接登录
  控制请求编号与 Wine `2..11/ABK 28,29/ACK 30,31` 对应未独立验证）；动态
  第六/第七集合；per-slot 编排与重连；2704→Wine callback parity。服务发布
  入口仍为 7709 补数 worker；生产 full-push 门禁保持关闭。

## 真实主站基准与跨登录类型证据（2026-09-02 20:1x，并行取证同步）

- 并行取证确定 Wine 主站掉线根因 = 旧式同步 `Ask(模块=认证&编号=0)` 与网际风
  自动登录冲突（Ask 返回 0 → 断开主站切备用并写回 `登录股票主站=0`）；修复为
  默认 `QUOTENETZIPWINE_INIT_LOGIN_MODULES=none`。Rust Native 自主 19 字段登录
  不受该 Ask 冲突影响；启用初始化需以 login_primary=true 健康会话为基准。
- 跨登录类型订阅指派一致性（只读）：1938（login_backup）与 1952
  （login_primary=true）在 58.16.134.228 上 7 条 2a10 逐条同构
  （6×1024+59=6203，P1–P7 起止 symbol index 相同），叠加 formal-primary-121528
  共三生命周期一致。见 `docs/forensics/cross-login-type-subscription-identity-20260902.md`。
- 在线接线前置 checklist（供下一个真实主站健康窗口）：
  1) 记录 effective login_primary=true 且事件含「股票主站登录成功」「初始化完成」；
  2) 保留 7100 control session（current_login_control_fields 可派生）；
  3) 单连接执行 interleaved 初始化并核对 3610 载荷 95/94/67 门槛与 3110/3210
     应答；4) 0104×4 → 2d10×3 → 五个主 2a10 发送后观察 2704 持续帧；
  5) 全部通过后再谈 per-slot 与 callback parity。

## 下个交易时段执行计划：P6/P7 顺序敏感度 + 全集订阅验证（2026-09-02 21:20 编写）

背景：接收清单∩0104=6203=全部订阅集合已双窗口证实；P1-P5 顺序逐字节复刻；
P6/P7 集合已证实但 vendor 26 段枚举序未复刻，实现用 0104 SH-then-SZ 降级序。
待在线判定：**订阅顺序是否影响服务器 2704 收流**（若集合正确即收全量，顺序
可保守降级；若按序逐符号推送则顺序必须复刻）。

前置（沿用并行会话已修根因）：login_primary=true 健康 Wine 会话 + 跳过旧式
Ask（QUOTENETZIPWINE_INIT_LOGIN_MODULES=none）+ `用户/只接收股票代码表.csv`
与 0104 同日。

步骤（开盘后按序执行，每步记录 webClx 请求号/抓包目录）：
1. 记录 effective login_primary=true、事件「股票主站登录成功」「初始化完成」。
2. 用 audit-official-5188-partition-alignment.py --receive-list 对当天窗口复验
   `all_subscribed_in_receive_intersection=true`（跨日一致性回归）。
3. 构造 Rust 全集发送（P1-P5 已证序 + P6/P7 降级序），经 NETZIP_NATIVE_5188_INIT
   opt-in 单连接发送；对照组用仅 P1-P5。
4. 观察两组的 2704/3e04 覆盖：统计收到的唯一 symbol 数 vs 发送订阅集合大小。
判定：
- 全集发送组收到 ≈6203 唯一符号且无持续告警 → 顺序不敏感，集合实现可全推；
  此时把 receive-list 全集原语接入 Native per-slot（P6/P7 降级序）。
- 全集发送组仅收到 ≈P1-P5 对应或大量缺漏 → 顺序敏感，需反推 26 段枚举序
  （0104 GBK name 分类 + 跨日稳定段序），成功后再接生产。
- 发送被拒/断连 → 记录错误，回滚到 shadow，不推进。

边界：全程 opt-in/实验窗口，不写生产默认；不做静态模板；full-push 门禁在
「全集 2704 覆盖 = 订阅集合」在线证实前保持关闭。参考脚本：
scripts/audit-official-5188-partition-alignment.py（--receive-list）、
scripts/audit-official-5188-2704-association.py。

## 2026-09-03 open-market verdict (negative for Rust decoder baseline)

- Paired open-market capture (open-capture-0937, 09:37:48-09:43:22) has real
  2704 delta streams: 37655 frames, every frame uses_baseline=true, no
  absolute baseline appears after the session is established.
- The extract cross-frame resolver lets some frames "decode" but the results
  are silently WRONG: 600259 -> 0.06 (gateway quoteNetzipWine says 78.98),
  600765 -> 0.0 (13.57), 688365 -> 226 (15.15). A uses_baseline delta applied
  against a pseudo-baseline yields a bogus record, not an error.
- Rust decoder parity CANNOT be validated from mid-session captures. The
  absolute baseline (per-symbol full value) only arrives on a fresh data
  connection at/near 09:25/09:30 when the server first pushes each symbol.
- Required next capture: cold-start a fresh 5188 data connection at 09:25
  call-auction and keep the first ~60-120s of 2704/3e04/0104 including the
  per-symbol absolute frame, THEN mid-session deltas. Only that window can
  establish the baseline resolver and prove Rust values match Wine callbacks.
- Until then business_decoder stays opaque-evidence-only; never treat the
  bogus "decoded" records as parity.


## 2026-09-03 10:45 - Fresh-path amount decode gap (next open-window task)

- Confirmed the server NEVER sends absolute 2704 frames on this account:
  every 2704 delta frame in 08:12 cold start (11 frames/1619 records) and the
  10:2x live dump (199 frames) is uses_baseline=true. 0104 code tables carry
  symbol metadata only (no price). Absolute baseline lives entirely client-side.
- The decoder supports a fresh-ladder path (baseline=None uses alternate token
  tables + merge_flag=0), but decode_official_5188_values errors on the first
  fresh frame: `amount decoded as negative` because the mask&0x80 amount branch
  does `delta_amount.wrapping_add(baseline_amount=0)` -- if the fresh token
  table (0x5b25f0) yields a real absolute amount delta this goes negative and
  the 09-02 B-bucket non-negative guard rejects it.
- Task: reconcile fresh-path amount semantics (real first-frame absolute amount
  vs cross-frame delta) with the B-bucket guard. Coordinate with the parallel
  netzip_win session that owns the B-bucket fix. This is the last decoder gap
  before live delta -> correct absolute value on a fresh session.
- Open-window capture plan stays: connect at 09:25 with dump-from-connect, so
  the very first 2704 frame(s) per symbol (fresh path) are captured and decoded
  with the corrected fresh-amount logic.

## 2026-09-03 10:52 - Fresh-path fix dependency clarified

- amount_prediction needs current.bytes[0x120]=exponent and [0x121..0x123]=
  multiplier, sourced from 0104 metadata tail. Official5188InternalRecord
  fields are PRIVATE: examples cannot seed a metadata record (no public
  zeroed()/set_i32()/bytes access). A clean fresh fix therefore requires a
  public constructor or a crate-level API (e.g. Official5188InternalRecord::
  from_code_table_metadata or a resolver seed helper) in netzip-fullpull.
- live2704_probe example kept: reads NETZIP_NATIVE_DUMP_DIR 2704 payloads,
  parses delta envelopes/indexes, and decodes against an optional baseline
  core (9/2 core reproduces 44 frames/970 symbols). webClx compile ids
  4004-4014. Any fresh fix must keep this example green.

## 2026-09-03 11:15 - Fresh amount seed experiment: negative result

- Seeding the resolver with metadata-only records (exponent=2/multiplier=100,
  the Wine sh603059 amount fixture values) for all 32617 code-table symbols
  did NOT let live 2704 deltas decode: frames-ok dropped to 14/199 (vs 44/199
  with the cross-date core) and decoded last_int values were near-zero or
  noise. The fresh amount delta is relative to a real server-side amount
  baseline, not something a locally guessed exponent/multiplier seed can
  recover.
- Conclusion: correct absolute live values require either (a) precise Wine
  in-memory record-table location (frida-grade, not blind /proc scans), or
  (b) the server reference frame at fresh-connection time. Blind metadata
  seeding and /proc scans are both dead ends - do not retry.
- Added Official5188InternalRecord::zeroed_with_amount_metadata (public,
  unused, pure addition) in case the frida-owning session needs it; 164
  fullpull tests still green. live2704_probe restored and verified
  (44/199 frames, 970 symbols with the 9/2 core); webClx 4016.

## 2026-09-03 16:30 - Runtime vs current build evidence and next gate

- Runtime `/home/bin/netzip/quoteNetzipRs` remains healthy and authenticated on
  formal account. `/api/fullpull/official-5188/status` reports retained control
  session, initialized data session, endpoint 222.85.139.177:5188, 19,983
  2704 frames, 0 receive terminations, `opaque-evidence-only`.
- The 16:22 build 4064 adds complete 0104 metadata including byte44
  `amount_mode`, but its release hash differs from the runtime binary. It has
  not been deployed; the running service is a valid 13:48 evidence runtime.
- Cold-start replay of build 4064 confirms all observed 0104 byte44 values are
  zero and all 1,619 decoded records therefore remain mode 0. The byte44 path
  is structurally verified, but this fixture provides no discriminating
  non-zero mode evidence.
- Production promotion gate remains unchanged: same-time/same-symbol Wine
  callback value parity, gateway delivery, and recovery. Current live Rust
  data is evidence-only and must not feed stock selection as official 5188
  business output.

## 2026-09-03 16:40 - 4064 replay evidence and runtime gates

- Rebuilt both paired captures with the 4064 extractor. The mid-session 09:37
  fixture decoded only 347/37,655 indexed 2704 frames; callback parity with
  the cold-start 0104 metadata matched 295 records but exposed only sparse
  field agreement. This confirms stale/wrong baseline behavior, not parity.
- The 14:15 fixture decoded only 2/3,048 indexed frames without its missing
  callback fixture. The 16:33 tool build (4065) is green.
- The production service remains intentionally on the 13:48 runtime; 4064 is
  an evidence build. Its public 5188 lane is still receive-only and
  `opaque-evidence-only`, while the transition publisher remains 7709.
- Current resident sources: Rust transition publisher reports received
  524,955/converted 524,360 records in its final 15:01 trading-window session
  and published 28,051 changed quotes; Wine reports ten healthy 5188 ESTAB
  connections but its adapter queue is not processing quote callbacks
  (`received_batches=0`, only `连接 网际风.exe 成功` events). Therefore stock
  selection is currently fed by the Rust 7709 transition lane, not Wine and
  not Rust 5188 business decoding.
- A manual post-window push attempt correctly timed out after its requested
  ten-second reader duration because the stockScreener queue blocked until the
  quoteGateway accepted its large stale-refresh batch. The temporary reader
  self-terminated; no residual worker remained. Production run gate remains
  09:14-15:01 and should not be forced outside the trading window.

## 2026-09-03 build-provenance correction

- Supersedes the 16:30 and 16:40 attribution of replay results to build 4064.
  Build logs show that requests 4064 and 4065 omitted release examples.
- The on-disk extractor, parity tool, and live probe predated both requests.
  Results under directories named `extract-4064` are retained only as
  historical old-tool output and are not evidence for the byte44 change.
- Re-establish provenance by explicitly building all three examples, recording
  SHA-256 digests, and replaying each fixture into a newly named output tree.

### Provenance restored by webClx 4067

- Request `170424-18d1a65e45a96d23` passed 172 tests with 1 ignored, Clippy, and explicitly
  built extractor, callback parity, and live probe release examples.
- Rebuilt extractor SHA-256:
  `8ceb14ac4ef07f1dee8da3bbb76928f7820936a2bb6e727488606d49e12cb9b1`.
- Fresh output trees use `verified-4067`. Results: cold 0/11 decoded frames,
  09:37 347/37,655, and 14:15 2/3,048. The latter two reproduce the old counts;
  the cold result contradicts and supersedes the former 11/11 claim.
- Cold 0104 byte44 is not all zero: eleven values occur, dominated by mode 3
  (22,149 rows) and mode 1 (5,294 rows). Decoder failures now expose mode 1
  negative amount behavior. Mode semantics remain a publication blocker.
- Same-window 09:37 parity matched 295/1,271 decoded records, but field hits
  remain price 1, amount 0, and volume 14. Keep `opaque-evidence-only`.

## 2026-09-03 16:46 - Post-close runtime gate and stale-baseline probe

- Rust 5188 stays healthy and receive-only: frame count continues to rise with
  zero terminations; the formal 7100 control socket is in CLOSE_WAIT by design
  after login, while the selected 222.85.139.177:5188 session remains ESTAB.
- Wine has ten 5188 ESTAB sockets, but its adapter HTTP status is now blocked
  behind repeated `连接 网际风.exe 成功` status traffic; the gateway reports
  zero Wine quote availability. A runtime restart is deferred until the next
  capture window because post-close 5188 sockets alone are not parity evidence.
- The quoteGateway confirms post-close state: netzipRust7709 has 99.89%
  available same-day quotes, but 0% are fresh within 3s; Wine has 0% available.
- Discriminating stale-core probe: no-core 14:15 replay fails 3,048/3,048 on
  missing SZ 2375 baseline. The 2026-09-02 stale core allows 1,477 frames /
  5,042 symbols to decode but yields implausible zero/negative/extreme values.
  This proves structural parsing, rejects cross-session baselines, and leaves
fresh same-session baseline plus amount-mode semantics as the remaining gate.

## 2026-09-03 18:28 - 4081 strict-baseline deployment checkpoint

- webClx deployment request `182647-18d1a65e45a96d31` completed successfully
  (`4081_build.log`, `4081_install-report.json`). The install audit reports one
  modified artifact and no missing or removed paths at
  `/home/bin/netzip/quoteNetzipRs`.
- The installed release and the workspace release artifact are byte-identical:
  SHA-256 `d53df077d3022979cee6635d6e34000ba23d115a9bc9f6fb7859298b739c101f`.
  Both `quote-netzip-rs-supplement.service` and
  `quote-netzip-rs-full-push.service` are active, and `/health` is healthy.
- The deployed 4080/4081 change enforces strict baseline provenance: 0104
  metadata-only rows cannot satisfy relative 2704 values, and missing real
  baselines are reported as errors instead of producing pseudo-quotes.
- After service restart the retained 7100 session is absent until the next
  authenticated login window; `/api/fullpull/official-5188/status` therefore
  reports `authenticated=false`, `initialized=false`, and no shadow reader.
  This is an expected lifecycle state, not evidence of decoder parity.
- Production promotion remains closed. The current service contract continues
  to publish only the verified 7709 transition lane; 5188 remains
  receive-only evidence until a fresh same-session baseline, complete P6/P7
  subscription coverage, reconnect/recovery, and same-time Wine callback field
  parity are all demonstrated.

## 2026-09-03 21:15 - Multi-slot 5188 connect (not Wine-identical yet)

- Production `POST /api/fullpull/official-5188/connect` now opens one 5188
  socket per non-empty receive-list partition instead of stuffing P1–P5 onto
  a single connection. Control numbers are `login=2+slot`, `ABK=12+slot`,
  `ACK=22+slot`. P6/P7 membership uses `只接收股票代码表.csv ∩ 0104` in
  SH-then-SZ fallback order (26-segment vendor order still
  `needs-verification`).
- Wine still shows ten ESTAB sockets (seven send `2a10`). The extra three
  sockets are not opened. Business decoder remains
  `opaque-evidence-only`; this change is topology/subscription coverage only.

## 2026-09-04 01:20 - Shadow decoder matches Wine memory on night cold start

- H4 (0104 seed as `2704` baseline) is rejected. The vendor empty slot is a
  fresh/zero record; 0104 only seeds metadata. Crate API:
  `decode_official_5188_values_with_fresh_fallback`. Strict
  `decode_official_5188_values` stays for mid-stream replay.
- Same-session evidence: Wine restart 01:19, probe mode 0 vs `/proc` 311-byte
  records at t+150s, field match on ts/OHLC/volume/amount/昨收/bid1/ask1.
  Tooling: `examples/official_5188_coldstart_probe.rs`,
  `scripts/coldstart/`.
- Publish gate unchanged: wait for 09:25 Wine callback parity. Do not login
  extra 5188 sockets overnight.

