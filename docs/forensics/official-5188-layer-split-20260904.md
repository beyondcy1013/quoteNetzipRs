# 官方 5188：三层不要并成一个洞（2026-09-04）

状态：本会话独有对照结论，供并行会话阅读。不修改
`crate/netzip-fullpull/src/official_5188.rs` 或
`examples/official_5188_callback_parity.rs`。
发布时间线：2026-09-04 12:22 起整理；核心实验是 01:19 Wine 冷启动。

对应执行队列：`docs/codex/tasks/official-5188-open-gaps-20260904.md`
（session `01a05a9a` 12:03）。那份清单把「位流解失败」和「回调字段对不上」
写进相邻 mustfix。下面说明它们不是同一层，以及夜里已经关掉的那一层。

生产发布仍关闭：`business_decoder=opaque-evidence-only`。本文不是发布许可。

## 1. 三层分别是什么

```text
2704 位流  --A-->  311 字节内部表（Wine 全局槽）
                    --B-->  与 Wine 回调事件配对（时间窗 + 代码）
                    --C-->  OEM_REPORT / quote_batch.v1（500B，十档盘口）
```

| 层 | 比较对象 | 本会话结论 | 中午队列里看起来像 |
|---|---|---|---|
| A | Rust 解出的 311B vs Wine 进程内存同一布局 | 冷启动初始转储 **已对齐** | 仍被「回调对不上」一起追 |
| B | 一条 `2704` 记录能否在 ±250ms 内找到同代码回调 | 对拍工具的 join，不是解码器 | 计入 `unmatched=7466` |
| C | 311B 投影成十档 float 后 vs `OEM_REPORT` | 冷启动未做；盘中数字会误导 bitstream | 书盘 35–41 被当成解码缺口 |

Wine 静态已经写过：回调数组来自 311 字节内部记录，**不是** 500 字节
`OEM_REPORT` 线格式（`docs/forensics/official-5188-2704-static-decoder-20260902.md`）。
NetzipAPI 的验收合同才是 `OEM_REPORT`；带 `*` 的内部派生字段不要当线字段追。

## 2. 层 A：夜里已经做完的判别实验

时间：2026-09-04 01:19 重启 `quoteNetzipWine-wine-supervisor`（新 vendor PID
1054222），同会话 pcap + 三次 `/proc` 内存扫描（t+45/90/150s）。

产物（gitignored）：

- `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/`
- 探针：`examples/official_5188_coldstart_probe.rs`
- 对照：`scripts/coldstart/compare-probe-with-memory.py`
- 摘要：`parity-vs-memory-t150.txt`

方法：用本会话 `0104` 只做元数据播种（`seed_code_tables`），`mask&1`
且尚无业务基线时走 **fresh/zero 槽**
（`decode_official_5188_values_with_fresh_fallback`），把解出的整数字段
直接和 Wine 内存里同一 `(market, symbol_index)` 的 311B 槽比较。
**不经过** `OEM_REPORT`，也 **不经过** ±250ms join。

结果（mode 0）：

```text
frames=38 records=5171 no_mem_record=0
 first: n=38  全部字段 38/38（含 bid1/ask1）
second: n=38  全部字段 38/38
 later: n=5095 全部字段 5095/5095
```

比较字段：`ts, open, high, low, close, volume, amount, last_close, bid1, ask1`。
`all8` 在三个桶都是满的。t+45 与 t+150 业务字节相同 → 这是连接后服务器
一次性转储，不是后来慢慢补的。

负对照（mode 1 / 计划里的 H4）：把 0104 元数据（昨收/涨跌停在
`0x120..0x137`）插入 `resolve_baseline`，与 Wine 内存对不上。
0104 尾部仍是元数据，不是上一份 311B 业务记录。

已拒绝的旁路：

- 从 `数据/实时.dat` 恢复。该文件最后写入仍是 2026-09-01 05:09；
  01:19 冷启动后内存已经是完整表。
- 把 09:37 **中途接入** 的「0104 当基线会解出 600259=0.06」当成冷启动结论。
  中途接入没有初始转储，fresh 槽是错的历史；严格模式报
  `requires missing baseline` 在那种抓包上仍然正确。
- 把 `3e04` 当行情快照。同日计划已排除：它是慢速文件块。

因此：凡是「冷启动缺基线所以值全错」的表述，以 01:20 为界作废。
产品 shadow 已走 fresh-fallback。盘中增量帧另算（层 A 的**另一批样本**）。

## 3. 层 A 的剩余：盘中 `ladder_volumes` 与冷启动不是同一个洞

`docs/forensics/official-5188-volume-diagnostics-20260904.md` 与
open-gaps 12:03 队列：

- 竞价回放 Rust `999/1164` 帧成功、165 失败；Wine 端点 `1058/1239`、181 失败。
- 失败主因 `ladder_volumes`（Rust 94，其中 41 条 `mask=0x01/header=0x1d`）。
- 相对基线和绝对基线都会失败 → **不能**再用「缺基线」解释这 165 帧。
  这一点与夜里 H5 兼容：初始转储已经能解；后面这些帧是位流/token 对齐问题。

这 165 帧归主会话改 `official_5188.rs`。不要用下面第 4、5 节的回调数字
去选 token 表。

午饭 live（约 12:11）：10 槽、`decoder_attempted_frames=38`、
`decoder_failed_frames=0`、`decoder_decoded_records=5165`、
`decoder_partial_frames=0`。这是又一次干净初始转储，和夜里 5171 同量级，
**不能**拿它当盘中 `ladder_volumes` 已修好。

## 4. 层 B：7466 unmatched 首先是配对，不是解码失败

对拍工具：`examples/official_5188_callback_parity.rs`（主会话所有，只读）。

配对规则：

- 默认窗口 `DEFAULT_WINDOW_MS = 250`。
- 用 **pcap 帧完成时间** `completed_at_micros` 去套回调批次的
  `timestamp_ms`，不是先用内部 `2704` 时间戳做硬相等。
- 窗口内再按回调 `datetime` 与内部 timestamp 的差、再按批次远近取最近一条。

commit-v5（Rust 端点，open-gaps 引用）：

- 解出 14,298 条记录
- 配对上 6,832
- 未配对 7,466（含缺 0104 元数据、投影失败；工具把这两类加进 unmatched）
- `carried_forward=2,829`：5188 侧 amount=0，按 Wine `0x44aa30` 合并语义
  不算字段错误

所以「解出 14298、只配上 6832」**不能**读成「解码器只对了 48%」。
窗口外、订阅覆盖差（P6/P7 6202 vs 官方样本 6267）、回调轮询重复序列、
以及帧时间与业务时间不同轴，都会进 unmatched。要分层统计：

1. 有 0104 元数据吗？
2. 窗口内有同代码回调吗？
3. 配上之后哪些字段相等？

第 3 步才进入层 C。partial-v1 把解出记录提到 15,262、配对提到 6,977，
未配对仍是多数 → 前缀保留救的是层 A 的「整帧丢弃」，几乎不动层 B。

## 5. 层 C：书盘 35–41 不能拿去指导 `ladder_volumes`

在**已经配上**的 6,832 条里（commit-v5）：

| 字段 | 命中 |
|---|---|
| timestamp | 5,620 |
| amount | 2,986 |
| volume | 2,581 |
| price | 1,907 |
| bid/ask 数组 | 35–41 |

这 6,832 条是层 A **成功**的记录。它们不是那 165 帧失败。
把书盘 35–41 写进「消除 bitstream 失败」会修错对象。

对拍如何比盘口（工具与 `Official5188InternalRecord::to_public_quote` 相同形状）：

- 311B 只带 **每侧五档**（内部下标 0–4 买、5–9 卖）。
- 回调是 **每侧十档**。投影把档位 5–9 填 `0.0`，然后
  `same_f32_array` 要求十个 float 的 bit 全等。
- 夜里层 A 只核过 **买一/卖一**（内部槽 4 和 5），5171/5171 对齐。
  没核过 OEM 十档，也没核过档位 2–5。

因此书盘命中极低，**不能**再用「这 6832 条的 `ladder_volumes` 位流解错了」
去指导 token 表——位流解错的帧进不了这批配对。
13:05 消融进一步表明：本份竞价回调的 OEM 第 6–10 档全是 0，
五档前缀与十档全等命中几乎相同（买 39/39，卖 35/35）。
**后五档填 0 不是 35–41 的原因。** 连买一单独比也只有 50/6977。
标量命中被 0=0 主导（见第 10 节）。

负金额：公开投影仍拒绝内部负 amount。这是发布闸，不是夜里内存对照失败。

## 6. ACK 62B vs 67B：短清单的合法回包，不是发错操作码

open-gaps：官方初始化连接 ACK 为 67B，Rust 现发后服务器回 62B；Login/ABK
已对齐。

本仓库已记录的长度随 **客户端 ACK 清单字节数** 变化
（`docs/EXPERIENCE.md` 2026-09-03、`auth_7100_client.rs`
`accepts_payload_len` 注释、authority 2026-09-02 ACK 节）：

| 客户端 ACK 清单 | 服务器 `3610` |
|---|---|
| 捕获的 1387B（Wine 本机文件 CRC） | 67B |
| 1103B（9 行结构市场 + 生成 FileInfo） | 63B |
| 默认/短 MarketInfo 行 | 62B |
| 592B 生成清单 | 44B |

验收器已是 `40..=80`，所以 62B **不是**握手失败。要变成 67B，必须按
**当前本机文件状态**生成与 Wine 同类的 `<FileInfo>` / `<MarketInfo>` /
`<SDidsCrc>`（zcode 交接第 5 节），而不是把捕获的 67B 当静态 payload
写进源码。凭据和带凭据的捕获 payload 不得进仓库、夹具、日志、HTTP。

实现位置在 `auth_7100.rs` / `auth_7100_client.rs`，不在
`official_5188.rs`。

## 7. 十条 TCP vs 七条 `2a10`

Wine 常见 10 条 `ESTAB` 到同一 5188。抓包里 **七条**各发一个 `2a10`
（P1–P7）。另外三条仍是 `needs-verification`：可能是初始化/文件通道，
不是第三套订阅分区。

不要为了「连上 10」去发明三个空订阅。live 若 slots 7–9 为
initialization-only 且 `2a10` 仍是七份，与这条观察相容。
P6/P7 **集合** = 接收清单 ∩ 0104 的非主推剩余；**线序**仍是 26 段官方
分类枚举，不是 SH-then-SZ 降级序（zcode 交接第 6 节；
`docs/forensics/cross-login-type-subscription-identity-20260902.md`）。
官方新样本 `6*1024+123=6267` vs 运行时 `6*1024+58=6202` 是清单/代码表
日期差，不要把 58/123 写死。

## 8. 读中午数字时建议的顺序

1. 冷启动/午饭 38 帧级：用层 A（内存或同会话 311B），不要用 OEM。
2. 盘中失败帧：只看 `ladder_volumes` / `0x01/0x1d` 等位流样本，
   用 Wine 静态 + 同帧 hex，不要用书盘命中率当损失函数。
3. unmatched：先拆 join（窗口、覆盖、元数据），再看字段。
4. 已配对记录：先扣掉 0=0，再比买一/昨收容差；不要用十档数组当损失。
   本份 09:24 回调后五档全 0，五档/十档消融几乎无差。
5. ACK：生成本机清单，接受 44–67 的合法区间，目标是清单内容与 Wine
   同会话一致，不是单独追求 67 这个数字。
6. 全部层 C 代表代码、同业务时间对齐之前，不打开 5188 对
   quoteGateway / stockScreener 的发布。

## 10. 2026-09-04 13:05 层 B/C 消融（partial-v1 × 竞价回调）

脚本：`scripts/coldstart/ablate-oem-projection.py`（不改对拍 example）。
输入：`extract-rust-222-partial-v1` + `callbacks/full-complete.jsonl` +
09-03 `verified-4068` 四张 0104（跨日索引稳定，缺元数据仅 23）。
产物：`diagnostics/20260904-live-rust/auction-0924/oem-ablation-partial-v1.json`。

配对与官方 partial-v1 报告一致：解出 15262，配上 6977，缺元数据 23，
**窗口内无同代码回调 8262**。unmatched ≈ join，不是解码失败。

标量「命中」里的 0=0（09:24 竞价前 Wine 价为 0，计划里已写过）：

| 字段 | bit 相等 | 其中双方都是 0 | 非零真正命中 |
|---|---:|---:|---:|
| price | 1991 | 1962 | 29 |
| amount | 3115 | 3099 | 16 |
| volume | 2692 | 2669 | 23 |

昨收：回调侧 **没有** 0。`0x12b/scale` 与回调 bit 相等仅 71；双方非零时
比值在 0.99–1.01 的有 1514，其余 5463 为 other。部分昨收是 float 容差
问题，不是缺字段；「other」仍可能是 join 到了另一份 public-state。

盘口：

- OEM 第 6–10 档在本份回调里全部为 0（`oem_levels_6_10_nonzero` 空）。
- 买价五档前缀命中 39，十档全等也是 39；卖价 35 与 35。填 0 不是主因。
- 买一单独 50/6977。内部买一非零 2143，回调买一非零 6927。
  Wine 回调几乎都有买一，311B 经常没有；有的时候也对不上。
  夜里层 A 内存买一是 5171/5171，所以这是层 C/join，不是「从来不会解盘口」。

时间戳：index 字段 6666/6977；311B `bytes[0:4]` 5739/6977（与 parity
工具 `timestamp` 命中一致）。

下一步（仍不改 `official_5188.rs`）：加宽 join 窗口看 unmatched 是否下降；
昨收改相对容差；把 09:25 之后的回调单独再跑一遍，避开竞价前 0 价。

## 11. 2026-09-04 13:51 窗口与 09:25 切片（仍不改对拍工具）

同一脚本一次加载、多窗口。产物：
`oem-ablation-windows.json`、`oem-ablation-post-0925.json`。

全量 15262 条：

| 窗口 | 配上 | 窗口外 | 非零价命中 | 买一 | 业务时间戳命中率 |
|---:|---:|---:|---:|---:|---:|
| 250ms | 6977 | 8262 | 29 | 50 | 95.5% |
| 1s | 10367 | 4872 | 33 | 60 | 78.9% |
| 5s | 14436 | 803 | 41 | 78 | 63.1% |

加宽窗口能把 unmatched 几乎吃掉，但时间戳命中率下降：配上的是**另一份 public-state**，不是同业务时刻。非零价/买一几乎不涨。
**不能靠把 ±250ms 改成 ±5s 来过层 B。**

`datetime >= 09:25:00` 后还剩 7326 条；250ms 配上 3810，非零价仍是 29，买一 48。
竞价后 0=0 少了，字段仍对不上。正确的下一刀是用 **内部 timestamp = 回调 datetime** 做主键，
墙钟窗口只做并列证据。那是对拍工具的改法，文件归主会话，这里不改。

## 12. 2026-09-04 14:05 业务 timestamp 主键 join（仍不改对拍工具）

脚本：`scripts/coldstart/join-by-business-timestamp.py`。
同一份 partial-v1 × `callbacks/full-complete.jsonl` × 09-03 `verified-4068`。
主键：同市场+代码，且 `index.timestamp ==` 回调 `datetime`（Unix 秒）。
同秒多条回调只用墙钟做并列打分，**不再设 ±250ms 门槛**。
对照仍是原来的墙钟窗口。产物：
`diagnostics/20260904-live-rust/auction-0924/join-by-business-ts.json`。

配对计数：

| 规则 | 配上 | 未配上（无同秒 / 窗外） | 业务时间一致 |
|---|---:|---:|---:|
| 墙钟 ±250ms（对照） | 6977 | 7795 | 95.5%（6666） |
| 业务秒主键（本实验） | 9127 | 5645 无同秒 + 467 无该代码回调 + 23 无元数据 | **100%**（定义） |

交叉表（已扣 23 条缺元数据）：

| | 墙钟也配上 | 墙钟未配上 |
|---|---:|---:|
| 业务秒配上 | 6701 | 2426 |
| 业务秒未配上 | 276 | 5369 |

读法：

- 2426 条是同业务秒、但回调轮询落在 pcap 完成时刻 ±250ms 之外。墙钟 p50=46ms，
  p90=1039ms，最大 70s；只有 73% 落在 250ms 内。把窗口加到 5s 会把这批吃进来，
  同时把下面 276 条错态一起放进来。
- **276 条墙钟独有配对的业务时间一致率是 0。** 它们是错误 public-state。
  非零价只有 2。这就是第 11 节「加宽窗口会配上另一份行情」的直接计数。
- 业务秒主键不能消灭 unmatched：仍有 5645 条解码记录在回调里找不到同一秒，
  另有 467 条代码整份 jsonl 都没有回调。后者可能是订阅覆盖/回调过滤，不要
  并进 bitstream。

字段在「已经同秒」之后几乎不涨：

| 字段 | 250ms 配上 6977 | 业务秒 9127 | 其中内部 amount≠0 的 1511 |
|---|---:|---:|---:|
| 非零价 | 29 | 34 | 22 |
| 非零额 | 16 | 17 | 17 |
| 非零量 | 23 | 24 | 24 |
| 买一（双方非零命中率） | 6/2137 | 9/2926 | 2/879 |
| 昨收 bit 相等 | 71 | 103 | 15 |
| 昨收相对 1% | 1514 | 2097 | 345 |
| 昨收相对 1e-4 | 76 | 110 | 19 |

结论（层 B vs 层 C 分界）：

1. **层 B 的墙钟窗口可以、也应该改成业务秒主键。** 这能多配 2426 条真同秒，
   并丢掉 276 条错态。对拍 example 归主会话，这里只提供离线证据。
2. **层 B 过关之后，层 C 仍然失败。** 1511 条内部 `amount≠0`（真正带成交增量）
   里，额 17、量 24、非零价 22、买一双方非零 2。这不是 0=0，也不是窗口。
3. 7616 条内部 `amount=0` 按 Wine `0x44aa30` 应保留旧 public-state；拿这一帧
   311B 的零值去比回调，会把合并语义算成投影错误。验收必须把 amount=0
   与 amount≠0 拆开。
4. 昨收：`1e-4` 相对容差（110）≈ bit 相等（103），**不是** f32 舍入缺口。
   相对 1% 的 2097 是另一回事，可能仍是错字段或合并源，不能当转换已对齐。
   夜里 311B 内存昨收已经对齐，所以 OEM `last_close` 不是「内部 `0x12b/scale`」。
5. 09:25 之后业务秒配上 5337（对照 3810），非零价仍是全部 34 次。竞价前 0=0
   去掉以后，投影缺口还在。

下一刀（仍不改 `official_5188.rs`）：在**已经同秒且 amount≠0** 的 1511 条上
复刻 `0x44aa30` 公共状态合并与 Wine 价/昨收 getter，而不是再调窗口。
批次映射/传输时间只用来解释那 5645 条无同秒，不能用来抬字段命中率。

## 13. 2026-09-04 14:20 公共状态回放（仍不改对拍工具）

脚本：`scripts/coldstart/replay-public-state-oem.py`。
产物：`diagnostics/20260904-live-rust/auction-0924/replay-public-state-oem.json`。
按用户五步做离线验收，不改 `official_5188.rs`。

实现的合并规则（证据名，尚未过关）：

1. 按 `(market, code)`、manifest 捕获顺序维护一份 public state。
2. `0x44aa30` A 桶：价/开高低/量/额，incoming 整数为 0 则保留旧值；
   `mask & 0x38 == 0x18` 只改时间戳；盘口稀疏 copyback（incoming 非零档才覆盖，
   避免 `clear_ladder` 把 OEM 五档清成 0，对应 SH603059 夹具）。
3. 价格缩放用 `10 ** 0104.opaque_tail[0]`（OEM `pointNum` / Rust
   `price_scale_hint`），getter 为 `(i32 as f32) / (scale as f32)`；
   昨收种子优先夜里 311B `last_close`，否则 0104 `opaque_tail` 的 `+0x0b` i32，
   **不用** 2704 解出后的 `0x12b`。
4. 业务秒内回调按 `(price,last_close,volume,amount,bid1,ask1)` 指纹收成
   state-group，再按该秒内解码序号对齐。
5. 先看 incoming `amount≠0` 的 1511，再看 carry-forward。

结果：

| 步骤 | 结果 |
|---|---|
| 4. 586 多候选 | 指纹折叠 **0**（同秒重复回调全是不同态）。`order_in_second` 把 586 条全部唯一赋值，没有剩余 nearest。字段命中不变。 |
| 3. 昨收源 × 5930 只有回调的代码 | 09-03 0104/`places` 76（1.28%）；夜里内存 65（1.11%）；`opaque_tail[1]` hint 更差。缺 **09-04 同会话 0104**。缩放本身能产出像样股价，问题在昨收整数源。 |
| 5a. 1511 里 555 条 | 同秒 Wine `price=0` 且 `amount=0`（竞价前公共盘）。内部却有非零额。这是日切/竞价前 OEM 已清零，2704 仍带着上一时段状态。不能当成交验收。 |
| 5b. 其余 956 条 live | 非零价 22、额 17、量 24、买一 19。与**不做合并、直接拿本帧 311B** 相同。A 桶零跳过在 amount≠0 上本来就不会改这些字段。 |
| 5c. 7616 carry-forward | 非零额命中仍是 0。公共状态往往从未收到过非零额，保留的是初始 0，不是 Wine 已有的昨收盘。 |

样本里还有负 amount（如 601918、688116），那是解码器问题，归层 A，不在投影里修。

结论：五步都跑过了，**1511 验收未过**。第 14 节已经关掉昨收 getter 和竞价前清零这两项；剩下的是 live 量额/价格转换。

## 14. 2026-09-04 15:40 昨收 getter 与竞价前清零

14:20 的「缺同日 0104」是错因。09-04 01:19 Wine 冷启动 pcap 里有完整 0104（最大表 SH 26506 / SZ 4601 / 共 32639 行），同日 `opaque_tail` i32@+11 对竞价回调昨收仍只有 **181/5930（3.05%）**。09-03 表是 76/5930。0104 昨收不是 OEM `last`。

真正的错因是 **按 symbol_index 去夜里 311B 取昨收**。Wine 01:19 与 09:24 Rust 竞价会话不在同一张 0104 索引图上：32606 个共有代码里只有 4634 个索引没动。竞价 2704 记录头里的嵌入代码与 **09-03** 索引一致（抽样 97/97），不能拿夜里的 `SH:24685` 当 603059。

按 **(市场, 代码)** 对齐之后：

| 昨收整数源 | 有样本 | 与回调 last_close 的 f32 命中 |
|---|---:|---:|
| 夜里 311B **收盘价 `+0x10`** | 4928 | **4914（99.72%）** |
| 夜里 311B 参考价 `+0x12b` | 4928 | 132（2.68%） |
| 09-04 0104 tail i32@+11 | 5930 | 181（3.05%） |
| 09-03 0104 tail i32@+11 | 5930 | 76（1.28%） |

311B raw 里与回调昨收 tick 对齐的偏移量，冠军就是 **`0x10`（4914/4928）**。`0x68` 有 2798 次是隔夜买一靠近昨收的巧合，不是 getter。SH603059：夜里 close=2468=次日回调 24.68；`0x12b`/0104 仍是 2533。

14 条残余（药明康德 156.63 vs 156.12 等）是隔夜最后一笔成交和官方昨收差几跳，不是缩放错误。1002 个回调代码不在夜里 5171 条探针里，所以 replay 只能回退到错误的 0104。

OEM 用法：隔夜读 `0x12b`。次日读 **当天登录 0104 +11**（第 21–22 节），不要快照 live `+0x10`（最后成交，14 只会对不上官方昨收）。不要用 2704 解完后的 `0x12b`，也不要用日切前的 0104 尾。

把该种子接到竞价 partial-v1 回放（9127 条业务秒 join）：

| | last_close | last_close 非零 | 非零价/额/量 |
|---|---:|---:|---:|
| 种子 = 夜里 close | 7722/9127 | 7722 | 仍 39/17/25 |
| 种子 = 夜里 0x12b | 210/9127 | 210 | 同上 |
| 1511 incoming 额≠0 | 1199 | 1199 | 22/17/24 |
| live 914 | 774 | 774 | 22/17/24 |

昨收层关掉了。价额量没动，说明它们不是昨收 getter 的副作用。

09:25 前把 OEM 价/OHLC/量/额/盘口清零（保留昨收）是 **needs-verification 已证实**：597 条「incoming 额≠0 但 Wine 价额仍为 0」的额命中从 0 升到 400，几乎全是 0=0。live 914 完全不变。不要把这 597 条算进成交正确率。

脚本：`scripts/coldstart/seed-last-close-and-preopen.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/last-close-seed-and-preopen.json`  
夜里 0104：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/extract-night-pcap/`

层 C 还剩：live 914 上的量额/现价转换（含负 amount 归层 A），以及日初快照在没有夜里内存时怎么从同会话初始 2704 dump 的 `+0x10` 取。第 15 节已经把「转换公式」否定掉。

## 15. 2026-09-04 15:50 live 914 不是 OEM 量额换算缺口

主会话 15:48 仍在层 A（收盘窗口 `ladder_volumes` / 多记录 bit-reader）。本会话对 live 914（incoming 额≠0、业务秒 join、回调已有价或额、datetime≥09:25）做换算消融，不改 crate。

| 类 | 条数 | 含义 |
|---|---:|---|
| 价两边都非零但对不上 | 677 | 公共状态不是同一份快照（竞价 pcap 从会话中途开始） |
| 负额 / 负量 | 118 | 层 A 位流垃圾，投影修不了 |
| 内部价=0、Wine 已有开盘价 | 96 | 本帧 2704 没有开盘成交，缺初始 dump |
| 价+原始额 f32 命中 | 17 | 快照已经对齐时，`i64 as f32` 就是 OEM 额 |
| 价对、额差 ≥20% | 3 | 价碰巧相同，累计量还没追上（600150 量 4343 vs 4752） |
| 价对、7709 PriceRelative 才命中 | 1 | 002439：997071→997084，即已知的 f32 边界 |
| 价对、相对误差 <1e-4 | 1 | 002237：3000527 vs 3000521.5 |
| 内部价仍是昨夜收盘 | 1 | 几乎可以忽略 |

7709 `oem_public_amount` PriceRelative 在这 914 条上只多救 1 条，和原始 f32 命中同为 17。量的原始 f32 命中 24，和额同一量级。**5188 OEM 额不是 7709 那套按价偏移再编码。** 快照对齐时直接 `amount as f32` 即可；SH603059 的 25934317 vs 25934334 属于这 2 条边界，不是 677 条的主因。

677 条的现价相对误差：≥20% 占大多数，也有一批 <5%。这是「Wine 09:25 OEM 已经是集合竞价结果，Rust 竞价抓包却是中途增量」——gap 文档里「auction capture begins after session baselines」在层 C 上的对应物。不要把这 677 条拿去改 bitstream token。

脚本：`scripts/coldstart/classify-live-914-conversion.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/live-914-conversion.json`

层 C 验收若还用这份竞价 pcap，必须先有同会话初始 2704 dump，或换夜里那种冷启动+回调同时在的样本。收盘窗口 14:48/14:57 的回调 JSONL 全是 `提示信息`、没有 OEM_REPORT，不能当层 C 对照。同会话层 C 对照在第 16 节：夜里冷启动自己的 `quote_batch`。

## 16. 2026-09-04 16:00 同会话夜里 OEM：投影公式已对齐

01:19 Wine 冷启动回调里有 17 批 `quote_batch`，之前没拿来做层 C。Seq 16（6187 条，datetime 几乎全是 2026-07-24 15:00）是 **dump 之前的陈旧 public state**，对 311B 价/昨收命中约 0.4%，是负对照。Seq 21 起（最新 6189 代码，datetime 2026-09-03 15:00）是 dump 之后的 OEM。

探针 `scale` 对科创板是错的（688403 写成 1，0104 `opaque_tail[0]` 是 2）。**缩放只用 0104 小数位**。与 5166 条探针∩后 dump OEM：

| 字段 | 命中 |
|---|---|
| 现价 / 开高低 / 买一卖一 | **5166/5166（100%）**，现价 = `+0x10 / 10^places` |
| 昨收 | **5166/5166（100%）** = `0x12b / 10^places`，不是 `+0x10`（147 条碰巧相等） |
| 量 | 原始 f32 4551/5166；科创 688/689 再 `round(vol/100)` 后 **5164/5166（99.96%）** |
| 额 | 精确 f32 425；相对 `<1e-4` **5119/5166（99.09%）** |

SH603059：内部 close 2468 / last 2533 / 额 27112222 / 量 10847；后 dump OEM 现价 24.68、昨收 25.33、额 27112076、量 10847。额差是已知 f32 边界，不是 7709 PriceRelative。

这把 15:40 和夜里 getter「打架」说清楚了：

- **隔夜、日切前**：OEM 昨收跟 `0x12b`，现价跟 `+0x10`（上一交易日收盘）。
- **下一交易日竞价**：日切把昨收换成上一收盘（`+0x10` 快照），价额清零。

不是两套互相否定的 getter，是日边界上的两种状态。层 C 在**同会话初始 dump + 同会话 OEM**上已经过关；竞价 pcap 过不了是因为缺这份 dump（第 15 节）。

脚本：`scripts/coldstart/night-oem-vs-311b.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/night-oem-vs-311b.json`

## 17. 2026-09-04 16:10 五档盘口与额尾巴

mustfix 要求把「已表示的五档」和「OEM 6–10 填零」分开比。同会话夜里 fixture：

| 项 | 结果 |
|---|---|
| OEM 6–10 价/量 | 6189 条后 dump 报价里 **全部为 0** |
| 五档价格 | **5166/5166（100%）**，内部 `+0x58` 为 bid5..bid1, ask1..ask5 |
| 五档量 | 原始 4545/4536；科创 `round(/100)` 后 **5069/5166（98.12%）** |
| 额相对 ≥1e-4 | **47 条**，全部仍 `<1e-3`，无科创；量已经对齐。是同一套 `i64 as f32` 边界，不是第二种公式 |

剩余 97 条五档量主要是 688 小单 round/ceil 差 1 手（内部 8 股 → round=0、OEM=1）。不要为此发明新 token。竞价样本里十档命中 35–41 不能指导 `ladder_volumes`：那是中途抓包缺 dump，夜里五档价已经 100%。

脚本：`scripts/coldstart/night-oem-book-and-amount.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/night-oem-book-and-amount.json`

层 C 在同会话 dump 上：标量、昨收日切语义、五档价、6–10 填零均已证据化。还没产品化的是日切快照时机，以及盘中增量仍依赖层 A。配方见第 18 节。

## 18. 2026-09-04 16:15 OEM 投影配方（同会话夜里冻结）

剩余 OEM_REPORT 字段：

| 字段 | 规则 | 5166 条命中 |
|---|---|---|
| name | 0104 或探针名称 | **100%** 两边都相等 |
| datetime | 311B `u32` 按 Asia/Shanghai 格式化 | 精确 84.03%；**±1s 99.25%** |

差 1 秒的 786 条是 311B `15:00:01`、OEM 写成 `15:00:00`（收盘秒向下取整）。另有 39 条提前 3–39 秒，是 15:00 前最后成交，不是时区错误。不要把 Unix 时间再减 8 小时。

冻结配方：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/oem-projection-recipe-v1.json`  
脚本：`scripts/coldstart/freeze-oem-projection-recipe.py`

产品实现时按这份 recipe 投影，不要搬 7709 额公式，也不要用这份竞价中途 pcap 当否证。盘中 2704 增量仍归层 A。第 19 节是按配方整记录对拍。

## 19. 2026-09-04 16:20 冻结配方整记录对拍

用 recipe 从 311B 投影 `quote_batch` 字段，再和夜里 seq21+ OEM 比。门槛：昨收 `0x12b`、时间 ±1s、额相对 `<1e-3`、科创量 `round(/100)`、五档价精确、6–10 必须为 0。

| 门槛 | 5166 条 |
|---|---|
| 整记录通过 | **5028（97.33%）** |
| 只放过五档量 round 残差 | 5125（99.21%） |
| 名称 / 价 / 昨收 / OHLC / 额 / 五档价 / 6–10 | **100%** |
| 失败构成 | 五档量 97；时间 >1s 39；累计量 2 |

没有第四类失败。97 条仍是 688 小单 1 手；39 条是 15:00 前最后成交。这就是 mustfix 要的「同代码同业务时间 OEM parity」在同会话 dump 上的验收，零 invalid。竞价中途 pcap 仍然不是这份配方的否证。

脚本：`scripts/coldstart/project-311b-to-oem.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/oem-projection-parity-v1.json`

## 20. 2026-09-04 16:25 日切昨收不能从中途第一帧快照

配方写过：隔夜昨收用 `0x12b`，次日昨收用上一收盘 `+0x10` 快照。产品若没有夜里内存，能不能用竞价会话里「每个代码第一帧非零 close」当快照？

5930 个有回调昨收的代码：

| 源 | 有样本 | 命中 |
|---|---:|---:|
| 夜里 311B `+0x10`（按代码） | 4928 | **4914（99.72%）** |
| 夜里 `0x12b` | 4928 | 132（2.68%） |
| 竞价第一帧 close | 4250 | **22（0.52%）** |
| 竞价第一帧 `0x12b` | 5930 | 76（1.28%，等于 09-03 0104） |

与夜里 close 相比：第一帧仍相等 20，已经变了 **3440**。样本时间戳是 `09:25:01`，close 已经是集合竞价价。中途接入不能从当前 `+0x10` 或 `0x12b` 恢复次日昨收。

产品含义：中途 2704 不能当昨收快照。第 21 节表明 **当天早盘登录的 0104** 已经带上日切后的昨收。

## 21. 2026-09-04 16:25 早盘 0104 昨收 = 次日 OEM last

`ten-slot-0909/rust-auth-5188.pcap`（09:10–09:15）有 5 张 0104（SH 26510 / SZ 4609 / 共 32997 元数据），**零帧 2704**。这是登录表，不是行情 dump。

SH603059：`opaque_tail` i32@+11 = **2468**（24.68）。夜里 01:19 同一格仍是 2533；09-03 表是 2517。竞价回调昨收 24.68。

对 6183 个有回调昨收的代码：

| 0104 源 | 命中 |
|---|---|
| 09:15 同日早盘登录 | **6183/6183（100%）** |
| 01:19 夜里 | 196（3.17%） |
| 09-03 | 86（1.39%） |

索引仍不稳：与夜里共有 32634 个代码，同 index 只有 5636。按代码对齐。

修正第 14/20 节「同日 0104 也不是昨收」：那是 **01:19 的表**，日切还没写进 0104。09:15 登录表已经是次日 OEM last。产品日切昨收优先用 **当天登录 0104 +11**，不要用 live `0x12b`（主会话 `to_public_quote` 当前读的是 `0x12b`，那是隔夜 OEM last）。本会话不改 `official_5188.rs`。

产物：`diagnostics/20260904-live-rust/ten-slot-0909/extract-strict/`  
对照：`diagnostics/20260904-live-rust/ten-slot-0909/morning-0104-last-close.json`

## 22. 2026-09-04 16:30 次日昨收是 0104 结算价，不是夜里最后成交

第 14/20 节把夜里 `+0x10` 当成次日昨收，是因为 4914/4928（99.72%）碰巧相等。用 09:15 登录 0104 的 `opaque_tail` i32@+11 去对同一批竞价回调：

| 源 | 有样本 | 命中 |
|---|---:|---:|
| 09:15 0104 last | 6183 | **6183（100%）** |
| 夜里 311B `+0x10` 收盘 | 5160 | 5146（99.73%） |
| 夜里 `0x12b` | 5160 | 146（2.83%） |

整数对拍：0104 last 与夜里 close **5146 相等、14 不相等**。这 14 只夜里 close 全部对不上回调，0104 全部对上。双方都错：**0**。

SH603259：夜里最后成交 156.63，0104/回调昨收 **156.12**。SZ000408：76.86 vs **75.86**。其余 12 只同样是几跳到一块钱的结算差，不是缩放。

09:10–09:15 全部登录 0104 表的 last **32997/32997 稳定**，登录过程中没有改写。

夜里 dump 只有 1870 只 SH + 3301 只 SZ。竞价 OEM 有 2887 只 SH；缺的 1023 只是普通 A 股（600/601/603，含贵州茅台），不在那五槽 dump 里。0104 仍 1023/1023 命中。

产品规则：次日 OEM `last_close` 用 **当天登录 0104 +11**。不要快照 live/`+0x10`（最后成交），也不要读 live `0x12b`（隔夜参考价）。主会话 `to_public_quote` 仍读 `0x12b`；本会话不改该文件。

脚本：`scripts/coldstart/morning-0104-vs-night-close.py`  
产物：`diagnostics/20260904-live-rust/ten-slot-0909/morning-0104-vs-night-close.json`

主会话 `Official5188OemState::project` 已把注入的 `previous_close` 写进 `0x12b`。运行时现已在匹配的当天登录 0104 可用时注入 seed；缺失时才回退 `OemState::new(0)` 并计数。注入源应是当天登录 0104 +11。本会话不改 `official_5188.rs`。

## 23. 2026-09-04 18:40 586 同秒多候选是竞价 leftover×成交成对，不是位流

用户阻塞排序把 586 条标成缺口 2 的 join 消歧。v2 墙钟平局仍配上（9184）；v3-state-safe 因 `distinct_states>1` 全部当 unmatched（`equivalent_duplicate=0`）。

对拍 `extract-rust-222-wine-tail-v1-strict`：

| 观察 | 数 |
|---|---:|
| 多候选解码记录 | **586**（与报告一致） |
| 回调侧同秒多组 | 828，**全部大小=2** |
| 组内指纹 | 828/828 两个不同态 |
| 组内昨收 | **828/828 相同** |
| 夜里同会话 OEM 同秒多组 | **0** |

典型一对：同一业务秒、昨收相同、价 `0` 对上一个竞价价（如 600284 5.24 昨收，价 0 vs 5.25）。批次号差约 70（两次 Wine 轮询）。解码侧 **从不会比回调多**：148 组 2=2 可 zip，290 组只有 1 条 2704 对 2 条回调。

产品规则：同秒候选若昨收相同，取 **较后批次 / 非零价** 那份；零价是 09:25 前 leftover 写进同一秒。不要像 v3 把 586 全丢掉，也不要用墙钟 nearest 去赌哪份。夜里 dump 没有这类成对，所以第 19 节配方不受 586 牵连。这不是 ladder_volumes。

脚本：`scripts/coldstart/disambiguate-586-batch-identity.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/disambiguate-586-batch-identity.json`

## 24. 2026-09-04 18:45 586 消歧评分：较后批次 = 成交价，墙钟 nearest 一半选 leftover

586/586 都是 `1 条价=0 + 1 条非零价`。较后 `sequence` 与非零价 **586/586 重合**。墙钟 nearest 选 leftover **290/586（49.5%）**，这 290 次全部是有成交价却选了 0。

昨收对早盘 0104：**三种策略都是 586/586**（成对昨收本来就相同）。nearest 的 143 次「现价命中」全是 `0=0`（incoming close 也是 0）；live/later 的现价命中是 0，因为中途 2704 已经不是竞价价。不要拿这个现价 0 去改位流。

产品规则收窄为：同秒且昨收相同，取 **最大 callback sequence**（本夹具上等价于非零价）。v2 nearest 会掺进约一半 leftover，并制造假的 0=0 价命中；v3 全丢掉则白白扔掉 586 条昨收已对齐的记录。

脚本：`scripts/coldstart/score-586-prefer-live.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/score-586-prefer-live.json`

## 25. 2026-09-04 18:50 科创档量是 half-up + 不足一手作 1，不是 banker's even

夜里 97 条 `book_vol_5` 里 75 条是 688/689。`(i32 as f32 / 100).round()` 和 Python `round` 都是银行家舍入：余数 50 会落到偶数（8650→86，OEM 87）；1–49 股会变成 0（8 股→0，OEM 1）。

Wine 规则（本夹具 615/615 五档量、2/2 累计量）：

```text
if v <= 0: 0
else:
    lots = (v + 50) // 100   # 正数 half-up
    if lots == 0: lots = 1    # 不足一手按 1 手
```

用这条之后：累计量 **5166/5166**；五档量 5144/5166；整记录（时间 ±1s）**5105/5166（98.82%）**。失败只剩 39 条收盘秒、22 条非科创「内部 0 / OEM 1」（002/300/301/511/159 的单档，不是 /100）。不要把这 97 条送进位流。主会话 `to_public_quote` 仍是 f32 banker's round；本会话不改该文件。

脚本：`scripts/coldstart/ablate-star-book-lots.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/ablate-star-book-lots.json`

## 26. 2026-09-04 18:55 非科创 0 对 1 是「有价无量显示 1 手」

22 只股票、23 档：内部量全是 0，OEM 全是 1.0，**该档内部价全部非零且与 OEM 价 bit 相等**。前缀 002/300/301/511/159，amount_mode 1 或 10。

规则：`volume==0 && price!=0` 则 OEM 档量填 **1**。加上第 25 节科创 half-up 后五档量 **5166/5166**。整记录只剩 39 条收盘秒（5127/5166，99.25%）。这是展示层，不是 2704 位流，也不要对非科创做 /100。

脚本：`scripts/coldstart/classify-zero-vs-one-book.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/zero-vs-one-book.json`

## 27. 2026-09-04 19:00 39 条时间全是 399 指数收盘秒取整

`datetime >1s` 的 39 条 **全部 SZ399xxx**。311B 时间是 `15:00:03`–`15:00:39`，OEM 全是 `15:00:00`。深成指 399001 内部 15:00:24，OEM 15:00:00。其余 35 条都是 +3s。786 条 `15:00:01` 本来就在 ±1s 里，同一种取整。

规则：Asia/Shanghai 若已过 15:00:00，发布 datetime 落到 **15:00:00**。本夹具 `floor_close` 命中 **5166/5166**。

叠加上第 25–26 节科创 half-up、有价无量填 1、额相对 `<1e-3` 之后，同会话夜里整记录 **5166/5166**。这是隔夜 dump 配方闭合，不是盘中位流验收，也不是发布许可。竞价 leftover 的 day-cut seed 生命周期仍在缺口 2。

脚本：`scripts/coldstart/classify-datetime-overshoot.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/datetime-overshoot.json`

配方冻结：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/oem-projection-recipe-v2.json`

## 28. 2026-09-04 19:05 shadow `new(0)` 昨收 0/9184，早盘 0104 则 9184/9184

竞价 wine-tail 抽出按业务秒 + 最大 sequence 配上 9184 条（与 v2 `matched_records` 同数）。把 `Official5188OemState.previous_close` 当成投影昨收：

| 种子 | 命中 |
|---|---|
| `new(0)`（缺失 seed 时的回退对照） | **0/9184** |
| live `0x12b`（`to_public_quote`） | 106（1.15%，等于 v2 报告的 last_close） |
| 当天登录 0104 +11 | **9184/9184** |

v2 昨收只有 106 不是 join 失败，是读了错字段。主会话 mustfix「从当天 0104 播种 OemState」一旦接上，这 9184 条昨收即可过关，不必等盘中位流。本会话不改 `official_5188.rs`。

脚本：`scripts/coldstart/score-oemstate-last-close-seed.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/score-oemstate-last-close-seed.json`

## 29. 2026-09-04 19:10 竞价 6297 无同秒：覆盖缺口小，邻秒加宽会配上 leftover

夹具：wine-tail 抽出 `extract-rust-222-wine-tail-v1-strict` × `callbacks/full-complete.jsonl`。
有元数据解码 15481；业务秒配上 **9184**；无同秒 **6297**（与第 28 节 `no_callback_same_second` 同数）。全部落在 09 点。本报告**不加宽**窗口。

拆开 6297：

| 桶 | 条数 | 含义 |
|---|---|---|
| 代码从未出现在本份回调 | 498 | 覆盖：本 wine-tail 切片的 OEM 轮询没扫到 |
| 其中 close=0 | 231 | leftover 式空成交，OEM 本来就不会发 |
| 代码在别的业务秒有回调 | 5799 | 2704 内部秒 vs OEM datetime 错位 |
| 最近回调恰好 ±1s | 3929 | 看起来像「差一秒」 |
| 最近 2–5s / 6–30s / 1–5m | 1620 / 249 / 1 | 轮询节拍，不是解码丢记录 |

从未入回调的前缀以基金为主（513/751/588/512），不是普通沪市 A 股缺表。

对 6297 条再看 ±1s 邻秒是否「干净唯一」：

| ±1s 形态 | 条数 |
|---|---|
| 邻秒完全没有回调 | 2368（含 498 从未出现） |
| 只在一侧有、且该秒唯一 | 3672 |
| 其中只有价=0 leftover | **2172** |
| 其中只有成交价 | 817 |
| 同一邻秒 leftover+成交并存 | 683 |
| 两侧都有或多样 | 257 |

3929 条「最近 1s」里，3672+257=3929。若产品把 join 放宽到 ±1s，2172 条会配上 leftover（价 0），683 条仍要靠最大 sequence。这与第 23–24 节同秒 leftover×成交是同一类错态，只是错到了相邻秒。

产品规则仍然是：**同代码 + 精确业务秒**，同秒多候选取最大 sequence。不要 ±1s，不要全局 ±250ms/±5s。这 6297 不能拿去指导层 A 位流。

脚本：`scripts/coldstart/classify-unmatched-no-same-second.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/unmatched-no-same-second.json`

## 30. 2026-09-04 19:15 9184 同秒配上后：昨收已闭合，竞价 live 价额仍是错会话状态

同一 wine-tail 抽出，精确业务秒 + 最大 sequence 配上 **9184** 条。配方 v2 投影，昨收用早盘 0104：

| 子集 | n | last_close | price | volume | 额 rel&lt;1e-3 |
|---|---|---|---|---|---|
| leftover（回调价=0） | 4145 | **4145** | 1349 | 1703 | 2010 |
| live（回调价≠0） | 5039 | **5039** | **46** | 21 | 3 |

昨收 **9184/9184** 与第 28 节一致，不依赖成交字段。live 价只有 46/5039（0.91%）。拆开这 5039：

- 内部 close=0、OEM 已有成交价：**1975**（同秒 2704 leftover vs OEM 成交，和第 23 节方向相反）
- 内部 close≠0 仍对不上：3018；其中 `|close|>1e6` 的乱值 145
- 相对误差：`≥50%` 1210，`<50%` 1020，`<5%` 430，`<1%` 358

样本 600150 内部 6.47、回调 34.5；600619 内部 0、回调 13.75。这不是缩放公式，是中途 pcap 的 311B 与 Wine OEM **不是同一份公共状态**。夜里同会话 dump 的 5166/5166 不被这条否证。不要用竞价 wine-tail 去改配方或位流。

主会话 18:58 已在 `official_5188_runtime.rs` 从登录 0104 `opaque_tail[11..15]` 按 `(market, symbol_index)` 播种 `OemState::new(previous_close)`，缺种子时 `unwrap_or(0)` 并计数 `decoder_missing_previous_close_seeds`。本会话只读该文件。日切/重连回归与「无种子则 not-ready」仍归他们。生产发布仍关闭。

脚本：`scripts/coldstart/score-joined-auction-fields.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/joined-auction-fields.json`

## 31. 2026-09-04 19:20 OemState 稀疏合并救不了竞价 wine-tail

按捕获顺序做与 `Official5188OemState.merge` 同形的零值保留，昨收仍用早盘 0104，然后再精确业务秒 + 最大 sequence 配上同一 9184 条。

live 5039：

| | incoming 单条 | 合并后 |
|---|---|---|
| price | 46 | **65**（+19） |
| volume | 21 | 22 |
| 额 rel&lt;1e-3 | 3 | 3 |
| last_close | 5039 | 5039 |

1975 条 incoming close=0 且 OEM 已有成交：合并后 1157 条带上了本抽出里更早的 close，但其中只有 **19** 条价命中（恰好等于 live 价 46→65）。818 条在本抽出里从未有过非零 close。样品 688361 合并后 5.80、回调 325.65——填上的是错会话残留，不是 Wine 当前态。

leftover 4145：incoming 价命中 1349→合并后 **1126**。223 条把更早成交填进了 Wine 仍发布价=0 的 leftover 秒。leftover 回调是明确的 0，不是稀疏洞；产品投影不要拿 leftover-only 秒当 OemState 对拍目标。同秒有成交时取最大 sequence（第 23–24 节）。

结论：OemState 合并是对的机器，但这份中途 wine-tail 没有同会话初始 dump，合并不能当配方或位流的损失函数。夜里 5166/5166 仍是层 C 真值。主会话 19:03 runtime 增加了按 `(market, symbol_index)` 播种的单测；本会话只读。

脚本：`scripts/coldstart/replay-oemstate-then-join.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/oemstate-merge-then-join.json`

## 32. 2026-09-04 19:25 昨收种子跨会话只能按代码，按索引只有 31.9%

离线 `last-close-seed-map.json` 的键是 `market+code`（32,997，invalid/duplicate=0）。主会话 shadow runtime 从**当次登录** 0104 按 `(market, symbol_index)` 播种，这在同会话内合法。负对照：把 09:15 登录表按索引灌进 09:24 wine-tail（索引对齐的是 09-03 表）：

| 键 | 9184 条同秒昨收 |
|---|---|
| 当天 0104 `(market, code)` | **9184/9184** |
| 当天 0104 `(market, index)` 跨会话 | **2929/9184（31.9%）** |

09-03 与 09:15 共有代码里，索引未动 3515、动了 **29086**（稳定率 10.8%）。样品：抽出 index 23830 在 09-03 是 600228，09:15 同索引是 600199。

产品规则：

- 跨会话、离线 seed map、重连后的新 0104：按 **代码** 持有昨收整数。
- 同一次登录的 0104→2704：runtime 可以用索引，但重连必须丢掉旧索引表，从新 0104 重建。
- 不要把 `last-close-seed-map.json` 按索引灌进另一次会话。mustfix 正文写的 `(market, code)` 仍是跨生命周期键；当前 runtime 的索引键不能替代它。

本会话不改 `official_5188.rs` 或 runtime。

脚本：`scripts/coldstart/score-last-close-key-index-vs-code.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/last-close-key-index-vs-code.json`

## 33. 2026-09-04 19:30 成交字段不是 09:25 全局清零，而是标的首次成交前保持 leftover

配方 v2 写过「日切后、09:25 前 OEM 成交字段为 0」。同一 9184 条同秒配上：

| | n |
|---|---|
| 09:24（全部 leftover） | 3845 |
| 09:25 及以后 | 5339 |
| 其中 live | **5039（全部 ≥09:25）** |
| 其中 leftover | 300 = 175 仍等本标的首次成交 + 125 本份回调从未成交 |

live_before_0925 = **0**。本夹具上全局 09:25 清零碰巧不会抹掉这 5039 条成交，但 09:25 一过会把 300 条 Wine 仍为 0 的 leftover 当成「开盘可发」。leftover 4145 条全部是「早于该代码首次非零回调」或「本份从未成交」，没有一条 leftover 出现在自己的首次成交之后。

产品规则：昨收从登录 0104 起就可发。成交价/量/额保持 0，直到**该代码**首次 live 回调（或同会话 2704 首次非零 close）。不要用墙上 09:25 当全局开关。这与第 23 节 leftover×成交、第 31 节 leftover-only 秒不当 OemState 目标一致。

脚本：`scripts/coldstart/classify-preopen-vs-first-print.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/preopen-vs-first-print.json`

## 34. 2026-09-04 19:35 leftover OEM 秒里 2704 已有非零 close：首次成交门是公共态，不是位流非零

4145 条 leftover（回调价=0）的 incoming `+0x10`：

| | n |
|---|---|
| 2704 close=0（与 leftover 0=0） | 1349 |
| **2704 close≠0，OEM 仍为 0** | **2796** |
| 合并后 close≠0 | 3019 |
| 合并后 close=0 | 1126 |

样品 600228 incoming 4755、回调价 0。这 2796 条若以「2704 首次非零」当开盘门，会在 Wine 仍发 leftover 0 时把残留/未发布成交写出去。第 33 节的首次成交门必须是 **公共 OEM live**（该代码首次非零回调，或同会话 dump 已过 leftover），不是内部 close≠0。leftover 回调仍不是 OemState 对拍目标（第 31 节）。live 侧 incoming close=0 仍是 1975，与第 30 节一致。

本会话不改 `official_5188.rs`。主会话 mustfix 已承认 shadow 在有表时播种 0104，剩余是日切/登录生命周期。

脚本：`scripts/coldstart/classify-leftover-2704-close.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/leftover-2704-close.json`

## 35. 2026-09-04 19:40 leftover 非零 +0x10 靠近 live `0x12b`，不是当天 0104 昨收

2796 条 leftover OEM + incoming close≠0：

| 对照 | 命中 |
|---|---|
| 整数/f32 等于当天 0104 last | **49** |
| f32 等于同记录 live `0x12b` | 120 |
| 相对 0104 `<5%`（含精确） | 964 |
| 相对 `0x12b` `<5%`（含精确） | **2401（86%）** |
| `\|close\|>1e6` | 75 |

样品 600228：内部 47.55，`0x12b` 48.26，0104/回调昨收 **10.28**。leftover 期间 OEM 已把昨收换成 0104 结算并清零成交价；2704 的 `+0x10` 仍是隔夜 `0x12b` 一类残留，不是今日竞价。更不能拿它当首次成交门。

脚本：`scripts/coldstart/classify-leftover-nz-close-source.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/leftover-nz-close-source.json`

## 36. 2026-09-04 19:45 leftover→live 时 wine-tail 的 +0x10 几乎不离开 `0x12b`

3023 只代码在本抽出里既有 leftover 同秒又有 live 同秒。按「leftover 非零且靠近 0x12b → live 靠近 OEM 且不再靠近 0x12b」计 **transition 只有 17**。952 只在 OEM 已 live 后 +0x10 仍靠近 0x12b；1912 只 live 既不靠近 OEM 也不靠近 0x12b。全部 5039 条 live 里，+0x10 靠近 0x12b 2055、靠近 OEM 834、f32 等于 OEM 仍是 46。

样品 605003 leftover/live 都是 6.36=0x12b，OEM live 已是 23.70。这份 wine-tail 观察不到「close 槽从隔夜残留切到今日成交」。首次成交门不能从 2704 离开 0x12b 推断，仍是 OEM live。同会话 dump 仍是层 C 真值。

脚本：`scripts/coldstart/classify-leftover-to-live-close.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/leftover-to-live-close.json`

## 37. 2026-09-04 19:50 「+0x10 ≈ 0x12b ⇒ leftover」会误压 40.8% 的 live OEM

在精确秒 + 最大 sequence 的 9,184 条同秒配对上，把「非零 +0x10 相对 live `0x12b` <5%」当成 leftover 门：

| 子集 | n | 会被压成 leftover | 会保留 |
|---|---|---|---|
| leftover（OEM 价=0） | 4,145 | 2,401（57.9%） | 1,744 |
| live（OEM 价≠0） | 5,039 | **2,055（40.8%）误伤** | 2,984 |

样品 600150：内部 6.47 ≈ `0x12b` 6.40，OEM live 已是 **34.50**。leftover 侧也只抓住第 35 节那 2,401 条残留，剩下 1,744 条 leftover（含 close=0 与远离 0x12b）仍漏检。

**不能**用 2704 close 靠近隔夜 `0x12b` 检测 leftover 或首次成交。门仍是公共 OEM live。这条启发式不得接入 join / OemState / 投影。

脚本：`scripts/coldstart/score-close-eq-12b-as-leftover.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/close-eq-12b-as-leftover.json`

## 38. 2026-09-04 19:50 只读：crate `to_public_quote` 19:31 已落地 STAR lots，配方还剩 datetime / 昨收源

主会话在 **19:28–19:31** 改了 `official_5188.rs`（本会话只读，未写）。相对夜里配方 v2（5,166/5,166）：

| 规则 | 配方 v2 | crate 19:31 |
|---|---|---|
| 科创书盘 / 累计 volume | half-up `(v+50)//100`，v>0 且 lots=0 则 1 | **已落地**（`.max(1)`，含 `with_public_state`） |
| 非科创有价无量 | 档量 1 | **已落地** |
| 银行家 `(f32/100).round()` | 已否证 | **已撤掉** |
| 收盘秒 | 过 15:00:00 则发布 15:00:00 | 仍是原始 `timestamp()`，源码无 15:00 取整 |
| 次日昨收 | OemState 从当天 0104 +11 注入 | `to_public_quote` 仍读 `0x12b`；runtime 有种子时注入，缺种子 `unwrap_or(0)` |

19:28 瞬间缺 1 手地板、累计 volume 仍是股数；19:31 已补上，与单测 `12349→123`、`1 股→1 手` 一致。本会话不编译。缺口 3 剩余是投影 datetime 取整，以及昨收/无种子 not-ready（更靠近缺口 2 接线）。

## 39. 2026-09-04 19:55 解码后的 2704 字段不能预测 OEM leftover vs live

精确秒 + 最大 sequence 的 9,184 条上，用内部字段预测「OEM 价≠0」：

| 规则（预测 live） | 精度 | live 召回 | leftover 特异度 | 误伤 |
|---|---|---|---|---|
| close≠0 | 0.523 | 0.608 | 0.325 | 2,796 leftover 非零；1,975 live 仍为 0 |
| volume≠0 | 0.494 | 0.474 | 0.410 | 接近抛硬币 |
| amount≠0 | 0.445 | 0.339 | 0.485 | 更差 |
| bid1≠0 | 0.472 | 0.413 | 0.438 | 书盘也分不开 |
| mask≠0x18 | 0.536 | 0.885 | 0.066 | 时间戳-only 在两边都有 |
| close 不靠近 0x12b | 0.719 | 0.200 | 0.905 | 召回太低（第 37 节的对偶） |
| **墙上 ≥09:25** | **0.944** | **1.000** | **0.928** | **FP=300**：第 33 节已否证的全局门 |

`mask & 0x38` 在 leftover 与 live 上覆盖同一组 {0,8,16,24,32,40,48,56}。样品 600619：2704 成交字段全 0，OEM 已是 13.75。

产品不能从解码后的 close/volume/amount/open/书盘/mask 推断首次成交。墙上 09:25 是唯一好看的数，但会把 300 条 leftover 当成已开盘。门仍是公共 OEM live；shadow 在无同会话 OEM 时必须保持 leftover（昨收可发，成交字段 0）。

脚本：`scripts/coldstart/score-2704-live-predictors.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/2704-live-predictors.json`

## 40. 2026-09-04 19:39 对拍 example 的 leftover×live 特例在本夹具上等于最大 sequence

主会话 19:36 改了 `official_5188_callback_parity.rs`（本会话只读）：`business-ts` 在「恰好 2 条、0 价 leftover + 非零成交、昨收 bits 相同」时取非零价的最大 sequence，否则仍在同秒内墙钟 nearest。

精确秒 9,184 条的候选形态：

| 形态 | n |
|---|---|
| 同秒唯一 | 8,598 |
| leftover×live、昨收相同、size=2 | 586 |
| leftover×leftover / live×live / n>2 | **0** |

墙钟 nearest 会选出 4,435 条 leftover（含 586 里的 290 条误选）。19:36 特例与「同秒一律最大 sequence」都选出 **4,145** 条 leftover，二者 **0 次分歧**；特例相对 nearest 改了那 290 条。本夹具上特例已经盖住全部冲突，fallback nearest 只打在唯一候选上，与 max-seq 相同。

产品仍建议直接「同秒 + 同昨收 → 最大 sequence」，不要保留 shape 门：夜里 0 组冲突，本夹具也没有第二形态，但 shape==2 leftover×live 不是协议常量。本会话不改 example。

只读补充：`Official5188OemState::project` 在调用 `to_public_quote` 前把 `previous_close` 写进 `0x12b`。缺口 3 的昨收路径是 seed → OemState，不是改 `to_public_quote` 去读 0104。缺种子仍 `unwrap_or(0)`。datetime 过 15:00:00 取整仍未进 crate。

脚本：`scripts/coldstart/score-join-1936-special-case.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/join-1936-special-case.json`

## 41. 2026-09-04 19:42 过期 index 播种不会走 `unwrap_or(0)`，会静默种错昨收

09:24 wine-tail 抽出 **没有** 同会话 `0104.code-table.json`（0 张表）。2704 里 **6162** 个唯一 `(market, index)`。若把 09:15 登录 0104 按 runtime 现用的 index 键灌进去：

| 形态 | 唯一 index | 含义 |
|---|---|---|
| 09:15 表里没有该 index | **0** | 不会触发 `missing_previous_close_seeds` / `unwrap_or(0)` |
| 有 index，代码相同 | 1,572 | 碰巧还能用 |
| 有 index，代码不同 | **4,590（74.5%）** | 种进另一只代码的昨收 |
| 其中 last_i32 碰巧相同 | **0** | 错码全是错昨收 |

样品 index 23830 一类：抽出侧 600228，09:15 同索引是 600199。计数器会显示种子齐全。`Official5188OemState::project` 只在 crate 单测里出现，runtime 只 `merge`，不投影。缺表的 wine-tail 与「过期全表」是两种失败：前者全 0 且可计数；后者静默错昨收。重连必须丢弃旧 `oem_states`/`previous_close_seeds`，从**当次** 0104 重建；跨会话 map 用 `(市场,代码)`。不要把 `decoder_missing_previous_close_seeds==0` 当成昨收正确。

脚本：`scripts/coldstart/score-stale-index-seed-poison.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/stale-index-seed-poison.json`

## 42. 2026-09-04 19:45 同会话 index 播种 5166/5166；crate datetime 取整后精确 5166/5166

01:19 夜里同会话（0104 表 + probe 311B + seq21+ OEM）是第 41 节过期 index 的正对照：

| 键 | 相对 OEM last_close | 代码是否同一 index |
|---|---|---|
| 当次 0104 `(market, index)` | **5,166/5,166** | **5,166/5,166 同码，0 错码，0 缺失** |
| 当次 0104 `(market, code)` | 5,166/5,166 | — |
| 311B `0x12b`（用 0104 精度） | 5,166/5,166 | 与 0104 +11 整数相同 5,166 |

runtime 按 index 播种 **只在当次登录表上合法**。隔夜 dump 上 0104 +11 就是 OEM 昨收，也等于 `0x12b`。次日必须换成当天登录 0104，不能沿用昨夜 `0x12b`。

datetime（crate 仍发原始 `timestamp()`）：

| 口径 | n / 5,166 |
|---|---|
| 原始精确相等 | 4,341 |
| 原始 ±1s | 5,127 |
| 本地时间 >15:00:00 需要取整 | **825** |
| 取整后精确相等 | **5,166** |
| 取整误伤（OEM 不是 15:00:00） | **0** |

825 = 第 27 节 786 条 15:00:01 + 39 条 >1s 的 SZ399。全局「过 15:00:00 落到 15:00:00」在本夹具无假阳性。本会话不改 `official_5188.rs`。

脚本：`scripts/coldstart/score-night-same-session-index-and-datetime.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/night-same-session-index-datetime.json`

## 43. 2026-09-04 19:49 runtime 去掉 `new(0)` 只覆盖无表；过期全表仍会合并 15505 条

主会话 19:48 `official_5188_runtime.rs`（本会话只读）：`merge_oem_records` 在 `(market, index)` 没有种子时 `continue`，不再 `Official5188OemState::new(0)`。wine-tail 0 张 0104 时，15,505 条解码记录会全部 not-ready，`oem_state_updates=0`，`missing_previous_close_seeds` 按**条**累加。

把 09:15 全表当过期种子灌进同一抽出（19:48 之后）：

| 路径 | not-ready skip | 会 merge | 其中错码 |
|---|---|---|---|
| 无表（本抽出实际） | **15,505** | 0 | — |
| 过期 09:15 全表按 index | **0** | **15,505** | **12,437（80.2%）** |

错码条数高于第 41 节唯一 index 的 74.5%，因为错码符号出现更勤。`decoder_missing_previous_close_seeds==0` 在过期全表上仍然成立。重连必须丢掉旧 seed map，不能只靠 not-ready。

本抽出内部时间全在 09 点，**0 条**需要 15:00:00 取整；crate 收盘秒规则不会误伤竞价窗口。`decoder_oem_state_updates == decoder_decoded_records` 在无表路径上不再成立。本会话不改 runtime。

脚本：`scripts/coldstart/score-not-ready-vs-stale-full.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/not-ready-vs-stale-full.json`

## 44. 2026-09-04 19:51 crate datetime 应改 u32 本身，不要只特判 SZ399

`Official5188PublicQuote.timestamp` 是 unix 秒。Wine OEM datetime 是 Asia/Shanghai `YYYY-MM-DD HH:MM:SS`。夜里 5,166 条：

| 口径 | 与 OEM 字符串精确相等 |
|---|---|
| 原始 u32 直接格式化 | 4,341（84.0%） |
| 本地时间 >15:00:00 则把 u32 改成当日 15:00:00 再格式化 | **5,166（100%）** |
| u32 被改写 | **825** |

825 的前缀不只是 399（39），还有 688（293）、603（250）、000（47）、51x/56x 基金、605、588。第 27 节 >1s 的 39 条全是 SZ399；±1s 门把另外 786 条 15:00:01 藏住了。**不要只对 399 取整。** 竞价抽出 0 条需要这条规则。

落地：在 `to_public_quote` 里改 u32，不要改 formatter。回调 datetime 按 UTC+8 解析（对拍 example 已如此）。本会话不改 `official_5188.rs`。

只读补充：`netzip_service.rs` 仍断言 `decoder_oem_state_updates == decoder_decoded_records`（「每条完整解码都更新 OEM」）。19:48 无表 not-ready 后这条不成立，聚合测试夹具还把两边都设成 150。缺口 6 指标不要再用这条守恒。

脚本：`scripts/coldstart/score-datetime-string-floor.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/datetime-string-floor.json`

## 45. 2026-09-04 20:01 产品投影必须带整行 0104，不能只缓存昨收 i32

runtime 19:48 只存 `(market, index) → previous_close i32`，从不调用 `OemState::project`。crate 事实（只读）：

- `merge()` 拷 OHLC/书盘/时间，**不拷 0x12b**
- `project(code, name, price_scale)` 才把 `previous_close` 写进 0x12b 再 `to_public_quote`
- `to_public_quote` 要求六位数字代码，且 `price_scale` 来自同会话 0104 `price_scale_hint`（`10 ** opaque_tail[0]`）
- `resolver.update_from_decoded` 整记录覆盖，含 0x12b
- `seed_code_tables` 对已有 index **跳过**，重连若不丢 resolver 就不会换新表

09:15 全表 32,997 行：`opaque_tail[0]` 为 2/3/1 的分别是 6,663 / 25,616 / 718。scale=100 只占 20%。`to_public_quote` 六位门挡掉 742 行（期权/期货代码）；竞价与夜里 OEM 代码 **0 条**非六位，名称与 0104 **精确 6,183/6,183**。

| 昨收发布路径 | 竞价精确秒 9,184 | 竞价唯一 OEM 6,183 | 夜里 seq≥21 唯一 6,189 |
|---|---|---|---|
| `OemState.project` + 当天 0104 行（i32 + scale + name） | **9,184** | **6,183** | **6,189** |
| 硬编码 scale=100 | 8,626（93.9%） | 5,329（86.2%） | 5,332 |
| `to_public_quote(oem_states.record)`（不 project） | **0** | **0** | — |
| `to_public_quote(resolver.record)`（2704 覆盖 0x12b） | 106 | — | — |
| 反事实：merge 把非零 0x12b 当 OHLC 拷 | 106 | — | — |

硬编码 100 在竞价漏掉的 854 只全部是 `places=3`（scale=1000）：ETF 159/51x、B 股 900、转债 113/123。样品 512000：0104/1000=0.526 对上 OEM，`/100` 变成 5.26。样品 600228：resolver/sticky 0x12b=48.26，OEM 昨收已是 0104 的 10.28。

不要：给 `merge` 补拷 0x12b；对 merged record 或 resolver 直接 `to_public_quote`；把 scale 写成 100；重连只换 `previous_close` i32。要：从**当次** 0104 行取 code/name/`price_scale_hint`/昨收 i32，调用 `project()`。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-project-needs-0104-row.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/project-needs-0104-row.json`

## 46. 2026-09-04 20:03 crate `public_timestamp` 已按第 44 节落地；OEM 对拍要用它而不是线芯 u32

主会话 20:02 只读：`Official5188InternalRecord::public_timestamp()` 用 UTC+8 当天 15:00:00 做 `timestamp > close` 钳位，`timestamp()` 仍是线芯。`to_public_quote` 发 `public_timestamp()`。单测 `1788505201→1788505200`、`1788505199` 保持。不是 399 特判。

本会话用同一整数算法对夜里 probe∩seq≥21 OEM 5,166 条：

| 口径 | 与 OEM datetime 精确 / unix 相等 |
|---|---|
| 线芯 `timestamp()` | 4,341（84.0%） |
| crate `public_timestamp()` | **5,166（100%）** |
| 与第 44 节 `datetime.replace(15:00:00)` 分歧 | **0** |
| u32 被改写 | **825**（399 仅 39；688/603/基金等同第 44 节） |

竞价 wine-tail 15,481 条有元数据的内部时全是 09 点，**0 条**会被钳。对拍 example 仍用 `value.index.timestamp` 配回调 datetime：竞价无影响；若拿夜里收盘 dump 做 business-ts join，会因 15:00:01 vs 15:00:00 丢掉 825。

产品规则：位流 / `OemState.merge` 继续用线芯 `timestamp()`；**发布和与 Wine datetime 对拍用 `public_timestamp()`**。不要改写线芯 u32。缺口 3 的 15:00 投影规则在 crate 侧已闭合；缺口 2 的 runtime 仍未调用 `project()`。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-crate-public-timestamp.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/crate-public-timestamp.json`

## 47. 2026-09-04 20:06 不要用 2704 自带的 0xe5 代码去 join；name 根本不在 311B 里

`from_code_table_metadata` 把 0104 的六位代码写进 `0xe5`、`amount_mode` 写进 `0x11f`、整段 `opaque_tail` 写进 `0x120`。`resolver.update_from_decoded` 整记录覆盖。`OemState.merge` 不拷这些字段；`to_public_quote` 的 code/name/scale 必须由调用方从 0104 行传入。名称只在 0104 GBK 字段里，311B 没有 name。

同会话夜里 probe 311B（Wine 全局表快照）**保留**当次 0104 元数据：代码 / amount_mode / 小数位 / 昨收 / 整段 tail 全是 **5,171/5,171**。

中途 wine-tail 最后一帧 2704（6,162 个 index，对照 09-03 表）：

| 字段 | 与 09-03 同 index 一致 |
|---|---|
| 线芯 `0xe5` 六位代码 | **1,841（29.9%）**；错 4,318；空 3 |
| `opaque_tail[0]` 小数位 | 6,159（99.95%） |
| `0x12b` 昨收 i32 | 50（0.8%） |

精确秒 join 9,184 条（仍用 09-03 index→代码，OEM 代码 9,184/9,184）：

| 代码来源 | 与 OEM 代码相等 |
|---|---|
| 09-03 `(market, index)` | **9,184** |
| 2704 `0xe5` | **3,197（34.8%）** |
| 09:15 同 index | 2,924 |

样品 index 23830：线芯写 600206，09-03/OEM 是 600228，09:15 同 index 是 600199。`0xe5` 是第三张过期表，不是 join 键。STAR 手数看的是 `to_public_quote` 的 code 参数前缀 688/689，不是 `amount_mode`（夜里 616 只 688/689 的 amount_mode 全是 1，但 1 不是科创专属）。

产品：code/name/scale 只从**当次登录 0104 行**取。不要从 merged `OemState.record`、resolver 覆盖后的 2704、或 `0xe5` 猜代码。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-resolver-overwrite-wipes-0104.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/resolver-overwrite-wipes-0104.json`

## 48. 2026-09-04 20:10 runtime 两步键仍从同一张表来；过期全表昨收还是 2,929/9,184

主会话 20:09 crate 增加 `OemState::from_code_table_metadata` / `project_from_code_table_metadata`（用 0104 行的 code/name/`price_scale_hint`）。20:10 runtime（本会话只读）把种子改成：

1. `symbol_codes`: `(market, index) → code`
2. `previous_close_seeds`: `(market, code) → 整行 Official5188CodeTableRecord`

两张 map 都从**同一批** `code_tables` 插入，后写覆盖。`merge_oem_records` 仍只 `OemState::new(previous_close).merge`，**从不** `project_from_code_table_metadata`。

同一次 09:15 登录的 301 张表：index 冲突 0、同码昨收冲突 0、32,997 行。单次登录内部 last-wins 安全。

把这张 09:15 表按 20:10 两步灌进 09:24 wine-tail（索引对齐的是 09-03）：

| 路径 | 精确秒 9,184 昨收 |
|---|---|
| 20:10 两步（过期 09:15 一张表） | **2,929（31.9%）** — 与第 32 节 index 键相同 |
| 两步映射代码 == 真代码 | 2,924 |
| 缺 index / missing 计数 | **0** |
| 09-03 身份 + 09:15 按 `(market, code)` 取昨收 | **9,184** |
| 映射代码与真代码 STAR 前缀交叉 | **21**（688↔605，手数规则会反） |

样品仍是 index 23830：真 600228 OEM 10.28，09:15 同 index 映射 600199 昨收 7.31。守恒 `decoded == updates + missing` 在过期全表上仍是 15,505 = 15,505 + 0。

产品：`(market, code)` 种子是对的，但 **index→code 必须来自解码那次登录的 0104**；不能把昨收表的 index 映射套到另一场 2704 上。crate 投影入口已齐，runtime 还没调用。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2010-two-step-seed.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2010-two-step-seed.json`

## 49. 2026-09-04 20:14 一旦用过期 index 映射去 project，发出去的是错代码不是只错昨收

主会话 20:11 crate：`OemState::from_code_table_metadata(market, row)` 用 0104 行填 311B（含 `0xe5` 代码和 tail/`0x12b`）。20:14 runtime（只读）创建状态时已调用它，随后仍只 `merge`，**仍不** `project_from_code_table_metadata`。`merge` 依旧不拷 `0xe5`/`0x12b`，所以记录里的昨收不再是 0，而是映射行的 0104 昨收。第 45 节「不 project 则 last_close=0」只适用于 `OemState::new` 空记录。

若现在就用当前两步映射去 project（过期 09:15 表 × 09:24 wine-tail）：

| 发出去的字段 vs OEM | 精确秒 9,184 |
|---|---|
| 代码 | **2,924 对 / 6,260 错（68.2%）** |
| 名称 | 2,924 对（与代码同时对） |
| 昨收 | 2,929（5 条错代码碰巧昨收相同） |
| STAR 前缀 688↔605 | 21 |
| 唯一 index 错 ticker | **3,499**；对的 1,569 |
| 09-03 身份 + 当天 0104 按代码 | 代码/名称/昨收 **9,184** |

样品 index 23830：应发 600228 返利科技 10.28，会发成 600199 金种子酒 7.31。打开 project 而不能把 index→code 绑在**解码那次** 0104 上，会把错票发到行情总线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2014-metadata-seed.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2014-metadata-seed.json`

## 50. 2026-09-04 20:18 `0xe5==symbol_codes[index]` 可当同会话门，不能当 join，也替不了 reseed

20:18 runtime（只读）新增私有 `reseed_code_tables`：整颗 decoder `= Self::new(新表)`，单测证明旧 `(SH,600000)` 种子被清掉。`ShadowReader` **没有**对外 reseed；线上仍靠拆掉 reader。仍不读 `0xe5`，仍不 `project()`。

`0xe5` 不当 join 键（第 47 节）。本刀只问：六位 `0xe5 ≠` 当前表 `symbol_codes[index]` 则 not-ready。

| 夹具 | 门拒绝 | 误伤（本该对的代码） | 放行后仍错 ticker |
|---|---|---|---|
| 夜里同会话 probe 5,171 | **0** | **0** | **0** |
| 过期 09:15 × wine-tail 精确秒 9,184 | **6,267（68.2%）** | 47 | **40（转债 110/113 邻近码）** |

拒绝里 6,220 条本来就是错 ticker（含样品 23830：线芯 600206 ≠ 09:15 600199 ≠ 真 600228）。放行 2,917 里 2,877 碰巧对、40 条线芯与过期表一致但仍不是 09-03 真码。同会话夜门干净；过期表上它能拦住大部分，**不能**代替「解码会话绑定的 0104 + reseed」。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-0xe5-seed-map-gate.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/0xe5-seed-map-gate.json`

## 51. 2026-09-04 20:23 `project().is_err` 是 schema 门，不是会话门；quote 仍被丢掉

主会话 20:23 runtime（本会话只读）在 `merge_oem_records` 里两次调用 `project_from_code_table_metadata`：

1. 对 `from_code_table_metadata` 得到的 **seed_state**（0104 形 311B，金额通常为 0）
2. `or_insert` + `merge(2704)` 之后再 project 一次

两次都是 `is_err` 则 `missing++` / `continue`。返回的 `Official5188PublicQuote` **丢弃**。`Official5188ShadowSnapshot` 仍只有计数和 frame sink，没有 quotes。

`to_public_quote` 失败条件是：scale 非正、市场不是 SH/SZ/BJ、代码不是六位 ASCII 数字、金额 < 0。过期但合法的 A 股 0104 行会全部通过。Compile `202434` 写的「same-session 0104 才能 merge」在夹具上不成立：过期 09:15 表照样 construct+project Ok。

| 夹具 | project schema Ok | 其中错 ticker | 备注 |
|---|---|---|---|
| 夜里同会话 probe 5,171 | **5,169** | **0** | 缺 index 2；schema 失败 0 |
| 过期 09:15 × wine-tail 精确秒 9,184 | **8,952（97.5%）** | **6,098（68.1% of Ok）** | 3,414 个唯一 index |

schema 失败 **232** 条，**全部**是 2704 负金额，不是错码：70 条映射代码其实是对的（误伤），162 条本就是错 ticker。这 232 条会先通过 seed_state 的 project，再 `merge` 进 `oem_states`，然后 post-merge project 失败 —— **不回滚**。守恒仍可把它们记进 missing，但脏状态留着。

样品仍是 index 23830：过期映射 600199 金种子酒，真码 600228 返利科技。六位数字 + 合法 scale，20:23 门放行。

产品：`project()` 可以挡住期权/期货非六位码和负金额，**挡不住跨会话错 ticker**。index→code 仍必须来自**解码那次** 0104。snapshot 仍未接线，opaque-evidence-only 未变。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-project-gate.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-project-gate.json`

## 52. 2026-09-04 20:23 负金额 merge 进 `oem_states` 后不回滚；后续 0 额/只改时间会被 leftover 负额卡住

第 51 节在精确秒 join 集上看到 232 条 post-merge `project` 失败。本刀按 **runtime 真实顺序** 回放整份 wine-tail 15,505 条解码记录（过期 09:15 全表，missing_index=0），只跟踪 `OemState.merge` 对 `0x1c` 金额的拷贝：非全 0 且 `mask&0x38 != 0x18` 才覆盖；`project` 见负额则 missing++，**不撤销这次 copy**。

| 口径 | wine-tail 15,505 | 夜里同会话 5,171 |
|---|---|---|
| updates / missing | 14,960 / 545 | **5,171 / 0** |
| 守恒 `decoded == updates + missing` | 成立 | 成立 |
| 首次插入已是负额 | 116（116 个 index） | 0 |
| 后来把本已 Ok 的状态染负 | 182 | 0 |
| 后续仍卡在负额 | 247（162 个 index） | 0 |
| 其中入站金额为 0 或只改时间 | **158** | 0 |
| 后来正金额治好并 updates++ | **29（23 个 index）** | 0 |
| 曾经脏过的唯一 index | 298 | 0 |

样品：index 23875 映射 600261，先写入 −263,043,729,113,119，随后一条 `incoming_amount=0` mask=206 仍 missing，因为 merge 不会用 0 覆盖 leftover 负额。index 25010 后来写入正金额 4,040,269，门打开并计入 updates。

夜里冷启动 dump 金额皆非负，这条路径为 0。竞价 wine-tail 的巨额负数是中途接入/错会话内部残留，**不要拿 545 去改位流**。守恒成立只说明计数配对，不说明 `oem_states` 干净。

产品：若要把 quote 接到 snapshot，post-merge `project` 失败必须回滚该次 merge（或丢掉该 index 的 state）。否则 (1) 后续合法的 0 额/时间增量会被 leftover 负额误杀；(2) 正金额「治好」后会带着错会话字段继续 updates。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-dirty-merge-persist.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-dirty-merge-persist.json`

## 53. 2026-09-04 20:23 正金额「治好」会留下脏盘口；回滚恰好救回第 52 节那 158 条

第 52 节的 29 次 later-update 含治好之后的后续成功合并。本刀把 dirty→Ok 的**第一次**打开门单独算：wine-tail **23** 次 / 23 个 index。其中 **12** 次 sparse merge 没覆盖脏记录里的非零 OHLC/盘口：治好后金额已正，但 bid/ask 价量或 open/high/low/close 仍是负额那一帧的 leftover。样品 index 25185 映射 603861：治好金额 194,790,854，十档价量全还在。

若 post-merge `project` 失败则回滚该次 merge（新插入丢掉，已有状态复原）：

| 口径 | 当前 persist | 回滚 |
|---|---|---|
| updates | 14,960 | **15,118（+158）** |
| missing | 545 | **387（−158）** |
| oem_states | 6,186 | 6,147 |
| 治好次数 | 23 | **0** |
| 守恒 | 成立 | 成立 |

+158 正好等于第 52 节「入站金额为 0 或只改时间却被 leftover 负额卡住」的 158。剩下 387 是**入站本身**合并后金额仍为负，回滚也必须拒绝；不要拿去改位流。夜里同会话 persist≡回滚，5,171/5,171，Δ=0。

产品：接到 snapshot 之前，失败必须回滚。正金额治好不是补丁，12/23 会把脏盘口和错会话代码一起发出去。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-rollback-vs-poison.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-rollback-vs-poison.json`

## 54. 2026-09-04 20:23 若现在把 persist `oem_states` 接到 snapshot，流末会发出自洽的错票

`Official5188ShadowSnapshot` **仍然没有** PublicQuote。本刀只问：按当前 persist 回放把每个 index 的最后一条 projectable 状态（金额≥0）发出去，会是什么。

wine-tail × 过期 09:15 两步映射，对照 09-03 真代码 + 最后一条 OEM 回调：

| 口径 | 数量 |
|---|---|
| oem_states | 6,186 |
| 会发出去（金额≥0） | **5,911（95.6%）** |
| 流末仍负额、发不出 | **275** |
| 有 09-03 身份且错 ticker | **4,377（74.0% of 可发）** |
| 代码对 | 1,510 |
| 昨收 == 真代码的 OEM last | 1,509 / 5,669 有 OEM |
| 昨收 == **映射代码**的 OEM last | **5,884（99.5% of 可发）** |

样品仍是 index 23830：会发 600199 金种子酒 7.31；真码 600228 OEM 昨收 10.28；600199 自己的 OEM 昨收就是 7.31。错票看起来完全自洽，用 last_close 对拍抓不住。

夜里同会话 0104 × seq≥21 OEM：5,171/5,171 可发，错 ticker **0**，昨收 5,166/5,166（5 条无 seq≥21 OEM）。同会话身份是对的；**不能**因此打开跨会话/重连后的 snapshot。

产品：接到 snapshot 之前必须 (1) index→code 来自解码那次 0104；(2) post-merge project 失败回滚。否则会发出 4,377 只自洽错票。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-snapshot-would-publish.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-snapshot-would-publish.json`

## 55. 2026-09-04 20:23 两张 map 必须拆开：解码会话 index→code + 当天 0104 按代码；错 ticker 从 4,377 降到 0

runtime 仍从**同一张** `code_tables` 填 `symbol_codes` 和 `previous_close_seeds`。本刀只改回放配方，不改 owned 文件：

1. `(market, index) → code` 来自解码那次 0104（wine-tail 用 09-03）
2. `(market, code) → 昨收/scale/name` 来自当天登录 0104（09:15 全表）

wine-tail 6,186 个 index：缺解码会话代码 24、缺当天按代码种子 1、两步都有 **6,161**。STAR 前缀交叉 **0**。

流末 persist 可发 vs 最后一条 OEM：

| 口径 | 同一张过期表（第 54 节） | 拆开两步 |
|---|---|---|
| 可发 | 5,911 | 5,886（−25 = 24+1） |
| 错 ticker | **4,377** | **0** |
| 昨收 == 真代码 OEM | 1,509 / 5,669 | **5,669 / 5,669** |
| 流末仍负额 | 275 | **275（不变）** |

身份对了，负额 leftover 还在。拆 map 是 connect/reseed 生命周期，不是位流。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-correct-two-map-snapshot.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-correct-two-map-snapshot.json`

## 56. 2026-09-04 20:23 拆 map + 失败回滚：流末 0 错票、0 负额残留；多发出 236 条上次 Ok 状态

第 55 节拆开两张 map 后身份已对，但 persist 流末仍有 275 条负额发不出。叠上第 53 节的 post-merge project 失败回滚：

| 口径 | 只拆 map（persist） | 拆 map + 回滚 |
|---|---|---|
| oem_states | 6,161 | 6,122 |
| 可发 / 流末负额 | 5,886 / **275** | **6,122 / 0** |
| 错 ticker | 0 | **0** |
| 昨收 == 真代码 OEM | 5,669 / 5,669 | **5,898 / 5,898** |
| 275 负额去向 | 留在 snapshot | 236 回到上次 Ok；**39 从未 Ok，保持 not-ready** |

样品 index 22906（513360）：persist 金额 −100,525,162，回滚后留下上次正金额 12,963,405。39 个只见过负额 2704 的 index 不进 snapshot，这是对的。

这是接线配方，不是位流：解码会话 index→code、当天 0104 按代码、失败回滚。runtime 20:23 仍是同一张表 + 不回滚。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-two-map-plus-rollback.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-two-map-plus-rollback.json`

## 57. 2026-09-04 20:23 回滚救回的是上次 schema-Ok，不是当前 OEM leftover/live

第 56 节多发出的 236 条「上次 Ok」昨收对（229/229 有 OEM），但公共成交态不对。对照最后一条 OEM：

| 口径 | 236 recovered |
|---|---|
| 有 OEM / 昨收相等 | 229 / **229** |
| OEM 已是 live（price≠0） | **223（97.4%）** |
| OEM live 而 311B 额=0 且 close=0 | **76** |
| OEM live 而 close==OEM price | **1** |
| OEM live 而 311B 非零但价不对 | **146** |
| OEM 仍是 leftover | 6 |

样品：512720 OEM 已 1.184，回滚状态额/收全 0；513360 OEM 0.479，回滚 close 0.915 额 12,963,405；600188 OEM 21.79，回滚 close 2.19 额 1.02×10¹⁶。`project` 只要求金额≥0，**上次 Ok ≠ 可发布的公共态**。首次成交门仍是 OEM live，不是 schema 门。竞价 wine-tail 这 146 条价不对**不要**拿去改位流。

`ShadowDecoder::new` 仍从同一份 `code_tables` 填两张 map，`start_with_code_tables` 没有拆表参数。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-recovered-last-ok-vs-oem.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-recovered-last-ok-vs-oem.json`

## 58. 2026-09-04 20:23 persist-clean 5,886 昨收全对，竞价 live 价几乎全不对

第 57 节只看了回滚多出来的 236 条。persist 本来就会发的 5,886 条（金额≥0、身份已拆对）对照最后一条 OEM：

| 口径 | persist-clean 5,886 | 拆 map+回滚全量 6,122 |
|---|---|---|
| 有 OEM / 昨收相等 | 5,669 / **5,669** | 5,898 / 5,898 |
| OEM 已 live | 5,421（95.6%） | 5,644 |
| close == OEM price | **65（1.2% of live）** | 66 |
| OEM live 而额=0且close=0 | 729 | 805 |
| OEM live 而价不对 | **4,627** | 4,773 |

样品 index 23830 现在代码已是 600228：OEM 竞价 10.40，311B close 48.27（夜里 leftover，第 35 节）。第 30 节精确秒 live 价 46/5,039；流末每个 index 一条仍是 **65** 量级。昨收身份已闭合，**不能**把 persist close 当竞价成交价发出去。不要拿 4,627 去改位流。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-persist-clean-vs-oem-live.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-persist-clean-vs-oem-live.json`

## 59. 2026-09-04 20:57 同会话夜里 dump 的 persist close 就是 OEM price；竞价 65 是 leftover/live 错会话

第 58 节竞价 persist-clean close==OEM live 只有 65/5,421。同一套 persist 回放打到 01:19 同会话 dump（index→code 与昨收都来自当夜 0104）：

| 口径 | 夜里 dump | 竞价 persist-clean |
|---|---|---|
| 可发 / 有 OEM | 5,171 / 5,166 | 5,886 / 5,669 |
| 昨收 | 5,166 / **5,166** | 5,669 / 5,669 |
| OEM live | 5,166（100%） | 5,421 |
| close == OEM price | **5,166 / 5,166（100%）** | **65 / 5,421（1.2%）** |
| 负额 | 0 | 275 |
| SH / SZ close 命中 | 1,865/1,865 · 3,301/3,301 | — |

样品 603059 24.68、603060 5.53、603061 330.26 全对。层 C 正对照：dump 时刻的 311B persist close **就是**公共 OEM。竞价 wine-tail 流末 4,627 条价不对，是 leftover vs 竞价 live，不是投影或位流缺陷。不要拿竞价 65 当「persist close 可以当成交价发出去」。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-night-dump-close-vs-oem.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/runtime-2023-night-dump-close-vs-oem.json`

## 60. 2026-09-04 21:02 竞价 65 条 close==OEM：52 条第一帧已经对上，不是本抽出「打出成交」

不要把后一帧 close=0 当成价格动了：稀疏 merge 留着第一帧非零 close。65 条（深 60 / 沪 5）按 wine-tail **第一帧**分：

| 桶 | n | 含义 |
|---|---|---|
| 第一帧已经 = OEM，且 = 0104 昨收 | **9** | 竞价价碰巧等于昨收 |
| 第一帧已经 = OEM，≠ 0104 昨收 | **43** | 抽出开始时已经停在 live 价 |
| 第一帧非零，后来改到 OEM | **8** | 抽出内小跳动（如 7.29→7.18） |
| 第一帧 close=0，后来出现 OEM 价 | **5** | 本抽出里真正从 0 打出的 close |

52/65（80%）第一帧就已经等于 OEM。从 0 打出的只有 **5/5,421**。样品 000025：0→15.51；000415：第一帧已是 4.51=昨收。这 65 条不能当「persist close 可当成交价发布」。层 C 正对照仍是第 59 节夜里 5,166/5,166。不要拿这 8 条小跳动去改位流。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-auction-65-coincidence.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-auction-65-coincidence.json`

## 61. 2026-09-04 21:06 从 0 打出 close 的 5 条也组不成公共 OEM：all_six=0

persist-clean 且 OEM 已 live 的 5,421 条，对照最后一条 OEM 的 close/open/volume/额/买一/昨收。6 项全对是 **0**。

| 桶 | n | close | open | vol | 额 | 买一 | 昨收 | all_six |
|---|---|---|---|---|---|---|---|---|
| 第一帧 0→OEM close | **5** | 5 | 3 | 3 | 3 | **0** | 5 | **0** |
| 第一帧已是 live≠昨收 | 43 | 43 | 7 | 5 | 0 | 17 | 43 | 0 |
| 第一帧=昨收=OEM | 9 | 9 | 0 | 0 | 0 | 2 | 9 | 0 |
| 抽出内小跳动 | 8 | 8 | 4 | 0 | 0 | 1 | 8 | 0 |
| close 不对 | 4,627 | 0 | 32 | 14 | 0 | 97 | 4,627 | 0 |
| OEM live 却全 0 | 729 | 0 | 0 | 0 | 0 | 3 | 729 | 0 |

样品 000025：价/开/量/额都对，买一却是 leftover −2.39e6（OEM 15.50）。002696：只有 close 对，量/额仍是 leftover（845 万手 vs OEM 3,780）。沪 live 2,382 条只命中 close 5 条，深 3,039 命中 60。close 对上不等于可以发 persist 311B。买一垃圾是 leftover 盘口，不要当位流。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-first-print-fields.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-first-print-fields.json`

## 62. 2026-09-04 21:10 同一套 persist 投影：夜里 dump all_six 5,166/5,166；竞价买一垃圾不是 0x68 写错

第 61 节竞价 OEM-live persist all_six=0，从 0 打出的 5 条买一 0/5。同一 `project_state`（买一 `+0x68`）打到 01:19 同会话 dump seq≥21：

| 字段 | 夜里 dump | 竞价 first-print 5 条 |
|---|---|---|
| n | 5,166 | 5 |
| close / open / volume / 额 / 买一 / 昨收 | **5,166 全中** | 5 / 3 / 3 / 3 / **0** / 5 |
| all_six | **5,166/5,166** | **0** |
| 负额 | 0 | — |

买一偏移在同会话 dump 上是对的。竞价 000025 买一 −2.39e6 是 leftover 盘口，不是 getter。不要拿竞价买一去改位流或 0x68。crate 现路径是 `crates/netzip-fullpull/src/official_5188.rs`（mtime 仍 20:11）。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-night-dump-all-six.py`  
产物：`diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/runtime-2023-night-dump-all-six.json`

## 63. 2026-09-04 21:13 从 0 打出 close 的买一垃圾来自入站 2704，不是 merge 丢掉好盘口

5 条 first-print 各 2 帧。稀疏 merge 只在入站 4 字节非零时拷盘口。**0 条**是「live 入站买一=OEM、persist 却错了」。

| 代码 | 第一帧 | live 入站 close / 买一 | persist 买一 | OEM 买一 |
|---|---|---|---|---|
| 000025 | mask 219 仅时间，买一 0 | 15.51 / **−2.39e6** | −2.39e6 | 15.50 |
| 000823 | close 0 买一 0 | 15.10 / **−1.66e6** | −1.66e6 | 15.10 |
| 002703 | close 0 买一 0 | 18.32 / **−1.85e6** | −1.85e6 | 18.31 |
| 002696 | close 0 买一 0 | 6.43 / **6.42** | 6.42 | 6.43 |
| 003816 | close 0 买一 0 | 4.39 / **0**（槽仍全 0） | 0 | 4.38 |

4 条 live 入站买一已经不等于 OEM（3 条线芯 leftover 大负数，1 条差 1 分）；merge 原样拷进去。003816 的成交帧 mask=56 没带买一，persist 保持 0。不是 0x68 getter，也不是 merge 丢好盘口。wine-tail 第一笔成交 311B 仍不是公共 OEM 盘口。不要拿这 3 条大负数去改位流。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-first-print-incoming-bid1.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-first-print-incoming-bid1.json`

## 64. 2026-09-04 21:16 精确秒 join：同秒 OEM 买一已经是公共盘口，2704 仍是 leftover

第 63 节对照的是流末 last OEM。5 条 first-print 成交帧全部配上 **同一业务秒** `09:25:00`（各 1 个候选，取最大 sequence）：close **5/5**，买一 **0/5**，all_six **0**。同秒 OEM 买一与 last OEM 买一相同（000025 15.50、000823 15.10、002703 18.31）。2704 买一仍是 −2.39e6 / −1.66e6 / −1.85e6 / 6.42 / 0。不是拿错了后面一条回调。003816 同秒 OEM 已有开/量/额/买一，入站 close-only。wine-tail 第一笔成交 311B 仍不是公共 OEM。不要拿这 3 条大负数去改位流。夜里 dump all_six 仍是 5,166/5,166。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-first-print-same-second.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-first-print-same-second.json`

## 65. 2026-09-04 21:19 精确秒 live join 全集：close 对上也不等于买一是 OEM

第 64 节 n=5。wine-tail 精确秒 + 最大 sequence 共 9,184 条，其中 OEM live 5,039。leftover 盘口门：`abs(i32 +0x68) > 1e6`。

| 口径 | n | leftover i32 | 买一=0 | 买一=OEM | 其它不对 |
|---|---|---|---|---|---|
| OEM live | 5,039 | **719（14.3%）** | 2,957 | **19（0.38%）** | 1,344 |
| 其中 close=OEM | **46** | **8（17%）** | 15 | **6（13%）** | 17 |
| 其中 close≠OEM | 4,993 | 711 | 2,942 | 13 | 1,327 |

close 对上的 46 条里只有 6 条买一也对。样品 000019：close=OEM 6.92，买一 −1.05e7。夜里 dump 同一 getter 买一 5,166/5,166。wine-tail 2704 盘口几乎不是公共 OEM。不要拿 719 条大整数去改位流。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-joined-live-bid1-leftover.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-joined-live-bid1-leftover.json`

## 66. 2026-09-04 21:22 精确秒 live：close+买一对上的行 all_six 仍是 0

第 65 节 leftover 门后 close∧买一=6。本节对 OEM live 5,039 条做六项对拍（昨收用当天 0104）。same_f32 含 0=0，所以 close∧买一变成 **11**（多出来的是双方买一为 0）。

| 子集 | n | close | open | vol | 额 | 买一 | 昨收 | all_six |
|---|---|---|---|---|---|---|---|---|
| OEM live 全量 | 5,039 | 46 | 38 | 21 | **3** | 79 | 5,039 | **0** |
| close=OEM | 46 | 46 | 13 | 8 | 3 | 11 | 46 | **0** |
| 买一=OEM | 79 | 11 | 19 | 14 | 0 | 79 | 79 | **0** |
| close∧买一 | **11** | 11 | 6 | 5 | **0** | 11 | 11 | **0** |

样品 002342：close/买一/昨收对，量 354,128 vs OEM 1,543，额 leftover。即使价和买一对上，wine-tail 2704 仍不是公共成交量/额。不要拿量差去改位流。夜里 dump all_six 仍是 5,166/5,166。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-joined-live-all-six.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-joined-live-all-six.json`

## 67. 2026-09-04 21:24 3 条额命中就是 leftover 买一的 first-print：只缺盘口

第 66 节 live 额命中 3 条，all_six 仍是 0。这 3 条正是第 63/64 节成交帧买一 leftover 的 000025 / 000823 / 002703（mask 全是 193）：

| 代码 | close | open | vol | 额 | 昨收 | 买一 |
|---|---|---|---|---|---|---|
| 000025 | 15.51 | 15.51 | 1,545 | 2,396,295 | 15.58 | **−2.39e6** vs OEM 15.50 |
| 000823 | 15.10 | 15.10 | 592 | 893,920 | 14.98 | **−1.66e6** vs OEM 15.10 |
| 002703 | 18.32 | 18.32 | 24,988 | 45,778,016 | 18.16 | **−1.85e6** vs OEM 18.31 |

五字段已对，缺的只有 leftover 盘口。wine-tail 最接近公共 OEM 的 3 条仍然不能发。不要拿这 3 条大整数去改位流。夜里 dump 买一 5,166/5,166。snapshot 仍未接线。本会话不改 owned 文件。

脚本：`scripts/coldstart/score-runtime-2023-joined-live-amount-hits.py`  
产物：`diagnostics/20260904-live-rust/auction-0924/runtime-2023-joined-live-amount-hits.json`

## 9. 证据索引

- 夜里实验计划与 H4/H5：`docs/codex/tasks/cold-start-parity-plan-2026-09-04.md`
- 经验短条目：`docs/EXPERIENCE.md`（2026-09-04 01:20 与本条）
- 权威账本：`docs/fullpull-replication-authority.md`（同日 cold-start 节）
- 盘中失败分类：`docs/forensics/official-5188-volume-diagnostics-20260904.md`
- 执行队列：`docs/codex/tasks/official-5188-open-gaps-20260904.md`
- zcode 官方样本（ACK/P6/3f04）：`windows_debug/zcode-handoff-20260904.md`

来源会话：本 Cursor 线程（冷启动内存对照与本拆分）；Codex
`01a05a9a-92c9-7813-9649-70d7483eaf0e` 持有解码器文件与 12:03 队列。
本文不移交那些文件的所有权。
