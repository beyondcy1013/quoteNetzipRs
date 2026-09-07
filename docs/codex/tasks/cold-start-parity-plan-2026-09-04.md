# 官方 5188 全推：冷启动逐步核对计划（2026-09-04）

状态：计划（2026-09-03 23:45 拟定）。目标：用正式账号（`NETZIP_TDX_ACCOUNT`，凭据只从
`/etc/default/netzip-rs` 读取，不写入任何文档/日志/命令行）让 `quoteNetzipRs`
的官方 5188 链路从冷启动起每一步与 `quoteNetzipWine` 一致，最终回调字段
（`OEM_REPORT`：code/name/OHLC/volume/amount/五档/昨收/时间戳）与 Wine 同代码同时间戳对齐。

## 1. 当前判断：哪里已一致，哪里还没有

| 阶段 | Rust 现状 | 与 Wine 对照 |
|---|---|---|
| 7100/6100 登录、L1.ini、选 5188 | 已实现并部署 | 已按抓包逐字段核对 |
| 3610/3110/3210 交错初始化、0104 四表 | 已实现 | 已逐字节核对 |
| `2d10 x3` 回执、`2a10` 订阅分区 | P1–P5 逐字节一致；P6/P7 今晚改成多槽一连接一分区（`只接收股票代码表.csv ∩ 0104`） | **P6/P7 未在真实会话中核对** |
| 连接数 | Wine 10 条 5188 + 1 条备用站；Rust 今晚起按非空分区开 N 条 | **未核对** |
| `2704` 结构解析 | 199/199 帧结构解析成功 | 结构一致 |
| `2704` 数值（基线） | 冷启动：0104 元数据 + fresh 空槽解初始转储，夜间内存字段已对齐；中途接入仍缺历史 | 夜间内存 OHLC/量额/昨收/买一卖一已对齐；**盘中 OEM_REPORT 未对** |
| `3e04` 大块帧 | 只统计、不解 | **语义未知** |
| amount 符号/换算 | 负值被拒绝发布 | 基线正确时 netzip_win 回放已匹配 |

结论：传输层与初始化已复刻；冷启动 `2704` 的基线规则已改为「空槽 + 会话内初始转储」
（H4 拒绝，H5 确认）。**剩下的发布门是盘中同代码同时间戳 `OEM_REPORT` 字段对齐**，
以及连接数/P6 段序仍 `needs-verification`。
盘中回调数字与位流失败不要并成一层，见
`docs/forensics/official-5188-layer-split-20260904.md`。

## 2. 今晚的新证据与突破口

1. `quoteNetzipWine/数据/实时.dat` 最后写入时间是 **9月1日 05:09**；Wine 于 9月3日 13:54
   冷启动后，14:15 回调正确。→ Wine 本会话基线**不是**从磁盘持久文件恢复的，
   只能是服务器在会话内下发。
2. 现有盘中抓包（09:37、14:15）都是 Wine **已连接之后**中途开始的。今夜 01:19
   已补一份 Wine 冷启动同会话 pcap + 内存 311 字节记录（`wine-night-coldstart-20260904T011938`），
   解码器字段对齐；**仍缺**交易时段从第 0 字节到首批 `OEM_REPORT` 的配对。
   → 明天 09:10 的核心实验是回调字段，不是再证明空槽规则。
3. Wine 09:15–09:24 的 `quote_batch` 输出 price=0.0，09:25 竞价后才有首个真实价格。
   → Wine 对没有基线的增量帧就是按零基线解（输出 0），并非有额外知识；
   基线要么来自 09:25 起的绝对帧，要么来自连接后服务器补发的快照。

4. 复查 `diagnostics/20260902-live-pair/fixed-accept-2000`（Wine 9/2 20:00 夜间 5188 冷启动，
   含 pcap 与回调）得到今晚最重要的几条事实：
   - Wine 客户端在每条连接上只发 `3610×3 / 2d10×3 / 2a10`（首连接多一个 `0710`），
     `2a10` 之后**再无任何客户端帧**。→ H3（Wine 多发了请求帧）排除。
   - 服务器在每条连接初始化后主动推约 890 KB，连接间等量、与分区无关。已辨认：
     `1504` = 文件推送（头 128 字节是文件名：`.//update//split.pwr` 除权、`.//update//bkcode_shszbj.dat` 板块），
     `2804` = 市场分类表（上证指数/上证A股…，即 `OEM_MARKETINFO`），`3f04` = 每股文本（H股比价），
     `1b04` = 按市场带日期戳（20260902）的索引表，`3e04` = 每 6 s 一块 5120 字节的慢速后台文件同步。
     这些都不是行情基线。→ H2（`3e04` 是快照）排除。
   - `2a10` 之后每条连接收到的 `2704` 记录数**正好等于该连接订阅分区大小**
     （1024/1024/1022/1021/1021/58），即服务器对订阅的应答就是「每只订阅代码一条初始记录」，
     但记录全部 `uses_baseline=true`，且值流 `mask&1` 置位。严格解码器对此报
     `requires missing baseline`；冷启动应走 fresh-fallback（H5，已用今夜内存对照确认）。
   - 同一时段 Wine 发出 6215 条「股票数据」回调，`gateway_forwarder.received_quotes=12374` 全部 POST 成功。
     → Wine 仅凭「0104 表 + 这批初始记录」就产出了完整行情，参照不可能是本会话上一条记录。
   - 0104 `opaque_tail` 不是不透明的：上证指数一行尾部三个 u32 = 397989/437788/358190
     = 昨收 3979.89、昨收×1.1、昨收×0.9（涨跌停）。现有 `from_code_table_metadata` 已把它放到
     内部记录 `0x120..0x137`，`last_close` 取 `0x12b`，位置吻合。

因此当前最强假设改为：

- **H4 0104 种子即基线（已拒绝，2026-09-04 01:20）**：把 0104 元数据插入
  `resolve_baseline` 后，与 Wine 内存 311 字节记录对不上。0104 尾部昨收/涨跌停
  仍是元数据，不是上一份业务记录。
- **H5 空槽 + 会话内初始转储（已确认，同一次实验）**：Wine 冷启动后全局表只有
  0104 种子；服务器在 `2a10` 之后主动推「每只订阅代码一条」的 `2704`，
  `mask&1` 且客户端无业务基线时走 **fresh/zero 槽**（与
  `decode_official_5188_values_with_fresh_fallback` 一致）。今夜 Wine
  supervisor 重启 01:19 的同会话 pcap + `/proc` dump 上，mode 0 与内存
  ts/OHLC/量额/昨收/买一卖一字段一致；mode 1（H4）为负对照。
  `实时.dat` 仍停在 9/1，排除磁盘恢复。
- H1 绝对帧假设（保留，开盘窗口才有意义）：09:25 竞价后每代码首条
  `uses_baseline=false` 记录为基线。判别：明早 09:25 窗口覆盖代码数。
  今夜无连续竞价，不能用它否定 H5。

## 3. 明日时间表（2026-09-04，周五交易日）

前置：多槽 connect 已在源码里；shadow 解码改为 fresh-fallback。今夜 01:19
已对 Wine 做过一次冷启动内存对照（H4 拒绝 / H5 确认），**不要再夜间重启 Wine**。
另一 Codex 终端（`quoteNetzipRs_18`）已预约 09:20 运行 `scripts/open-auction-baseline-capture.sh`
（Wine 侧 09:24–09:35 pcap + 回调），**不要重复起 tcpdump**，同一个 pcap 会包含 Rust 的 5188 流。
Rust 侧正式 `login`+`connect` 仍按 09:08/09:10，不要提前开 5188。

| 时间 | 动作 | 核对项 / 产物 |
|---|---|---|
| 09:05 | 健康检查：两侧服务、Wine `vendor_config.effective.login_primary=true`、Rust `/health`；启动全时段 pcap（`tcp port 5188 or 7100 or 6100`，覆盖 Rust 与 Wine） | `diagnostics/20260904-cold-start/full.pcap` |
| 09:08 | Rust `POST /api/auth/login`（凭据来自环境） | L1 服务器列表与所选 5188 端点 == Wine 当前远端（今晚为 58.16.134.228） |
| 09:10 | **Wine 冷重启**（`systemctl restart quoteNetzipWine-wine-supervisor`，此时无行情，影响最小）；同时 Rust `POST /api/fullpull/official-5188/connect` + `shadow/start` | 连接数、每连接 `2a10` 分区逐字节 diff（含 P6/P7）、`0104` 四表 diff、`2d10 x3` diff |
| 09:10–09:25 | 两侧持续接收 | 按连接统计 `3e04` / `2704(abs)` / `2704(delta)` / `0d04` / `5404` 帧数与字节数，Rust vs Wine 是否同型同量（H3 若不一致，先比客户端发送帧） |
| 09:25–09:35 | 集合竞价定价窗口（另一终端脚本同时抓 Wine 回调） | 每代码首个 `uses_baseline=false` 记录时间；Rust 解码值 vs Wine 回调同代码同时间戳（H1） |
| 10:30 | **盘中冷启动配对**：Wine 重启一次 + Rust disconnect/connect（需你确认，Wine 中断约 1 分钟） | 连接后 60 s 内服务器补发内容：绝对记录覆盖数、`3e04` 总字节；这是判定 H1/H2 的决定性样本 |
| 10:35–11:00 | 离线回放 | `official_5188_extract` 抽 Rust 流 → `official_5188_callback_parity` 对 Wine 回调；同时对 `3e04` 拼接体做 H2 试解 |
| 下午 | 按判定结果实现基线来源（绝对帧建基线 / `3e04` 解码 / 补发请求帧），部署后 14:00 再做一次盘中冷启动配对复核 | 不匹配率 ≤ 1%，且 amount 无负值 |

## 4. 通过标准（逐级门）

1. 连接级：Rust 连接数、每连接 `2a10` 载荷、`0104`、`2d10` 与 Wine 同会话逐字节一致。
2. 帧级：`2a10` 后 60 s 内服务器帧类型/数量/字节量与 Wine 同量级（差异 < 5%）。
3. 基线级：全部订阅代码在会话内都获得一次可用基线（绝对记录或快照），并记录来源。
4. 值级：对齐 Wine 回调（code, timestamp）后，OHLC/volume/amount/五档/昨收全部字段不匹配率 ≤ 1%，无负 amount。
5. 只有第 4 级通过，才解除 `to_public_quote` 的发布闸门并接入生产发布。

## 5. 风险与回退

- Wine 冷重启会中断 `选股` 上游约 1 分钟：09:10 那次无行情、无影响；10:30 那次需你同意，否则改到 11:30 午休或 15:05 收盘后（收盘后仍能验证补发快照，但无增量）。
- Rust `connect` 使用正式账号与 Wine 同时在线；今日 Wine 已 10 条 5188，Rust 再开 5–7 条，若服务器限制并发连接，会在连接级门直接暴露。
- 夜间（23:30 起）Wine 只回 `连接 网际风.exe 成功` 提示、无行情帧，夜间不做冷启动实验。
- `official_5188_extract` 目前只在给定基线时能出正确值；所有对比先以 Wine 回调为真值。

## 6. 凭据处理

正式账号只由 `/etc/default/netzip-rs` 提供给服务进程（`NETZIP_TDX_ACCOUNT` / `NETZIP_TDX_PASSWORD`）。
计划、脚本、诊断产物中不出现密码；抓包中 7100 登录段用 `scripts/sanitize_7100_capture.py` 脱敏后再入库。

## 2026-09-04 01:50 evidence refresh

- Night cold-start fixture
  `diagnostics/20260904-cold-start/wine-night-coldstart-20260904T011938/`
  has pcap SHA-256
  `f70e2e4111b3b9dd8df7d041b1f3d17b6b2e217f7c767484a862a11f1b06eee5`.
- The 408x multi-slot release is installed and healthy, but its research lane
  is not authenticated until the planned 09:08 login/connect. It correctly
  reports seven-slot `pending-production-wiring`, `opaque-evidence-only`.
- Rerun of the saved mode-0 replay confirms 877 captured frames with 38
  initial-dump 2704 frames decoded, zero failures. Comparison against the
  t+150s Wine memory snapshot is 5,171/5,171 matched records with all tracked
  fields (timestamp, OHLC, volume, amount, last close, bid1, ask1) equal.
  Evidence file: `parity-vs-memory-t150-rerun.txt`.
- The temporary 0104-as-baseline negative control remains rejected; the
  deployed runtime shadow decoder uses the confirmed fresh-fallback path.
- Morning run remains on schedule: login/connect at 09:08/09:10, capture the
  09:25 absolute window, then run same-code/same-time callback parity before
  any publication gate changes.
