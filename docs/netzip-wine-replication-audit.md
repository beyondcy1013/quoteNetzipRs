# quoteNetzipWine 功能复核与数据补充证据链

Updated: 2026-09-01

## 结论

### 2026-09-01 正本清源复核

Wine 的全推行为已经按真实源码和抓包重新确认：`Start(callback)` 建立 DLL 回调运行时，
随后执行认证/备用登录/初始化；正式登录控制连接（已观察到 6100 和 7100 两种端口角色）
返回服务器列表，厂商进程随后建立长期
`5188` 数据连接。午后样本中 5188 客户端每条连接仅发送 6--8 个初始化帧，服务端持续
发送数千个 `0x2704` 增量帧，并伴随 `0x0d04/0x5404/0x2104/0x3e04` 批次。Rust 当前
生产进程的 53 条 7709 工作表扫描连接不属于该链路，仍是补数据/过渡发布。

Rust 已新增 `netzip-fullpull::official_5188`，先把这个真实边界固化为可测试的帧重组与
方向分类层。5188 内层对象压缩、证券字段映射、完整初始化和回调等价性仍未完成；在这些
证据门槛通过前，Wine 继续作为官方全推实现，Rust 7709 服务不得改名宣称替代。

`quoteNetzipWine` 不能被描述为已经由当前 Rust 服务“完全复刻”。现有证据确认 Windows
主程序至少承担三个不同职责：正式账号认证后的非 7709 全推、通过 7709 完成的补数据、
启动参考资料同步。此前 `quoteNetzipRs` 被称作“实时推送”的生产路径实际是 7709 全市场
扫描，属于过渡发布，不是 Wine 官方全推。

本次新增 `/home/codes/stock/crates/netzip-supplement`，并由 `quoteNetzipRs` 的
`POST /api/supplement/kline` 对外提供日线、5分钟线和1分钟线补数；代码表、除权、财务与财务 V6
文件参考对象也已分别通过 OEM 接口闭环；`实时.dat` 与 FIN V8 的完整 `OEM_REPORT` loader 也已
接入。抓包 `object_01..15` 的数据外壳和控制包也已逐字节闭环；动态回调时序、失败分支和盘中质量
仍是独立的未完成事项，不能用现有补数实现冒充完成。

## 功能边界

| 功能 | Wine 证据 | Rust 状态 | 结论 |
|---|---|---|---|
| 官方全推 | 正式账号、认证后 5188 数据连接、`OEM_REPORT` 盘中回调 | `netzip-fullpull` 目标包；当前仅有认证和取证模块 | 尚未完成；7709/0547 发布不能作为替代证据 |
| 7709 过渡发布 | 不属于当前 Wine 官方全推主链 | 旧 0547 worker 与 quoteGateway `netzipRust7709` 发布链 | 可运行，但语义上属于补数/兼容路径 |
| 补日线 | 168/168 Wine 完整 296 字节 answer | supplement `daily -> category 4` | SH600000 三根服务映射逐字节一致 |
| 补5分钟 | 168/168 Wine 完整 296 字节 answer | supplement `five_minute -> category 0` | SH600000 三根服务映射逐字节一致 |
| 补1分钟 | 168/168 Wine 完整 296 字节 answer | supplement `one_minute -> category 7` | SH600000 三根服务映射逐字节一致 |
| 代码表同步 | 两个完整初始化对象，2965/3366 条 | JSON join 与 OEM 二进制接口 | 完整抓包 fixture 及当前动态 HTTP 均已闭环 |
| 除权同步 | 完整对象 59,600 个 200 字节单元 | PWR V8 解析与 `/api/supplement/split/oem` | 完整抓包 fixture 及当前动态 HTTP 均已闭环 |
| 财务同步 | 完整 OEM 财务 5993 条及 FIN 文件 | FIN V8 解析与 `/api/supplement/finance/oem` | 完整抓包 fixture 及当前动态 HTTP 均已闭环 |
| 财务 V6 文件 | 完整 `object#5` 与当前 6173 条文件 | V6 校验与 `/api/supplement/file/oem` | 旧对象及当前 HTTP payload 均已逐字节闭环 |
| 本地实时快照 | `数据/实时.dat` 固定槽、getter 与 `object#9/#10` | JSON 解析器与 `/api/supplement/realtime/oem` | 6211 条完整 OEM loader 已闭环；竞价及 volume `+1` 分支缺动态样本 |
| 本地 2000 兼容 | `Stock.dll -> 127.0.0.1:2000 -> 网际风.exe` | 仅分析/对位工具 | 尚未成为原生替代入口 |

## 逐项证据

1. 官方上层契约
   - `docs/netzip_api_bin/NetzipAPI/股票接口调用规范.txt` 给出日线、分钟线等 `Ask` 语义。
   - `docs/netzip_api_bin/NetzipAPI/StockC#/Test.cs` 按 200 字节头和 32 字节 K 线记录解析。
2. Windows 实机行为
   - `windows_debug/tmp_netzip_probe_20260329/probe_run_20260329_v3.txt` 中，1分钟线和日线
     均返回 `296 = 200 + 3 * 32`，时间与价格可解释。
   - `windows_debug/findings.md` 记录安装说明中的“补日线/补5分钟”入口。
   - `windows_debug/kline_full_probe_20260901` 使用隔离的 `168/168` 配置保存三类同步 Ask
     完整 answer；三份均为 296 字节，而非从日志字段重建。
3. 当前生产运行配置
   - `/home/codes/third_party/quoteNetzipWine/用户/配置文件.ini` 当前根数为日线 1000、
     5分钟 3000、1分钟 2000。较早 Windows 现场曾记录日线 5000，说明配置可变；新 crate
     采用当前运行副本的 1000 作为默认值，同时允许请求显式覆盖。
4. 初始化大对象抓包
   - `windows_debug/tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_init_full_2000_chunks/outer_object_catalog/summary.txt`
     完整列出上海/深圳代码表、除权、财务、文件和实时对象顺序及长度。
   - `.../frida_ws2_trace_20260330_v15_object3_split_full/parsed_split_full/summary.txt`
     确认除权对象完整。
   - `.../frida_ws2_trace_20260330_v16_object4_finance_full/parsed_finance/summary.txt`
     确认 5993 条财务记录完整。
5. 远端协议对位
   - `windows_debug/tdx7709_vs_rusthq_20260330.md` 确认 K 线使用 `0x052d`，并将 5分钟、
     日线、1分钟分别映射到 0、4、7 类别。
   - 通用 TDX 算法复用 `/home/codes/crates/rustHq`：`TdxPacket` 负责 `0x052d` K 线与 F10
     两类请求构造，`parser::parse_single_kline_body` 负责 K 线日期、价格 varint、成交量和金额
     解码，公共 F10 parser 负责栏目与正文解码。当前 7709 兼容代码仍物理位于
     `netzip-fullpull/src/tdx7709.rs`；其目标所有者是 `netzip-supplement`，迁移期间补数 crate
     先做业务编排且不复制算法。

## 新增契约

- 输入周期：`daily`、`five_minute`、`one_minute`。
- 默认根数：1000、3000、2000。
- 一次 7709 会话完成同一请求，按最多 800 根分页。
- 跨页按 `datetime` 去重并按时间升序返回。
- 默认请求间隔 10ms，对应 Wine 的 `补数据间隔=10`。
- 每个证券/周期独立返回 `page_count / exhausted / complete / error`。
- 任何失败保留在响应中；`continue_on_error=false` 时明确标记 `aborted=true`。
- 对外来源固定为 `netzip-rust-7709-052d-supplement.v1`，历史数据不进入实时推送身份。
- `POST /api/supplement/kline/oem` 把同一补数结果编码为官方 packed ABI：200 字节头加每根
  32 字节 `OEM_KLINE`；日线时间归一到中国零点，分钟时间原样保留，成交量按同一 7709
  代码表的 `volume_unit` 换算并四舍五入。三周期 SH600000 服务映射均与完整 Wine answer 逐字节一致。
- `POST /api/supplement/split/oem` 读取动态 PWR V8，按 Wine 接收清单交集和顺序输出 200 字节
  `OEM_SPLIT_HEAD / OEM_SPLIT` 单元；旧完整对象 fixture 与当前 HTTP 验收分别覆盖静态字节一致和动态集合。
- `POST /api/supplement/finance/oem` 读取动态 FIN V8，按 Wine 接收清单交集和顺序输出 350 字节
  `OEM_FINANCE`；三个日期、48 个指标和完整旧对象均有字节级 fixture。
- `POST /api/supplement/file/oem` 校验动态 FIN V6 的 magic、166 字节记录与完整长度，再以
  `OEM_DATA_HEAD + 原始文件字节` 输出；完整旧 `object#5` fixture 和当前文件 HTTP `cmp=0` 均已覆盖。
- `POST /api/supplement/realtime` 校验动态 `实时.dat` 偏移 4 的布局标记 `250303`、195216 字节头、5206 字节槽、
  活动计数、代码格式与唯一性，并输出磁盘原始 getter JSON；内部成交额压缩码和 OEM 派生值保持分离。
- `POST /api/supplement/realtime/oem` 连接同一 `实时.dat` 与 FIN V8；原始 JSON 保留全部 6211 个
  已占用槽，OEM 实时 callback 则按 Wine 行为排除 24 个 `time=0` 槽并编码 6187 条
  `OEM_DATA_HEAD + OEM_REPORT[]`。金额三模式、分类、价格比例、override、五档、inVol、
  `change / weiBi / liangBi` 均按 Wine getter 的 `f32` 运算顺序映射。

## OEM 二进制证据边界

- 旧 2026-03-29 探针只打印字段，原文保留于
  `docs/forensics/oem-kline-wine-probe-20260329.txt`，不再承担完整字节等值证明。
- 2026-09-01 增强探针使用隔离的测试账号 `168/168`，已保存日线、5分钟线和1分钟线三份
  `SH600000` 完整 296 字节 answer。codec 在相同字段输入下逐字节一致；服务测试再从 7709
  原始时间和成交量出发，经 HTTP handler 使用的同一入口逐字节一致。
- fixture 哈希、隔离配置、字段摘要和有效测试计数见
  `docs/forensics/oem-kline-wine-full-buffer-validation-20260901.txt`。
- 该证据只闭环已捕获的单证券三根结果；大页数、全证券成交单位、动态 callback/失败分支、并发负载和
  开盘全市场质量仍需单独验收，不能据此宣称完整 Wine 替代。

## 2026-09-01 rustHq 通用算法复用边界

- 当前物理布局中，`/home/codes/stock/crates/netzip-fullpull` 以本地路径依赖
  `/home/codes/crates/rustHq`（crate 名 `dllhqarrow-rs`）。K 线、F10 栏目、F10 正文请求构造和
  响应解析不再维护第二份算法。
- 旧共享 crate 遗留的 7709 bootstrap、TCP 会话、`client10/server16` frame、代码表
  原始记录、`0x0547` 交付和 Wine/OEM 映射当前仍在 `netzip-fullpull`。这些模块需要在不复制
  `rustHq` 公共 parser 的前提下迁往 `netzip-supplement`；当前所在目录不代表长期所有权。
- 三项等价性合同分别比较三个请求包、K 线全部业务字段及 F10 栏目/正文；webClx 请求
  `060614-18d0ccb5d58be4a8` 使用完整测试名实际各运行 1 条并通过。生产切换后请求
  `061121-18d0ccb5d58be4a9` 运行 `netzip-fullpull` 全部 50 条单元测试，50/50 通过。
- 该重构减少重复实现，不扩大 Wine 一致性结论。抓包逐字节相等仍由下文 OEM 完整对象 fixture
  和 168/168 K 线 answer 验证；动态 callback、异常路径、大页分页与开盘全市场质量仍是独立证据门槛。

## 尚需开盘验收

- 全市场实时推送的延迟、缺码和重连质量仍以已有 replacement evidence gate 为准。
- 5分钟和1分钟大页数补数应在真实服务上校验返回上限、分页连续性和请求节流。
- 补数与全推并发时的上游连接配额、服务 CPU/内存和响应体大小需要负载验收。
- `object#6..8/#11..15` 控制包、数据外壳、`object#9/#10` codec 和
  `实时.dat -> OEM_REPORT` loader 均已覆盖；动态初始化回调时序和失败分支仍需现场验证。当前
  `实时.dat` 没有 `jingJia=true` 或 Wine volume `+1` 特例记录，这两条分支仍缺动态 fixture；
  全市场实时链仍需开盘质量验收。

## 2026-09-01 初始化外壳与控制包闭环

- `object_01/02/03/04/09/10` 的 264 字节外壳共享同一骨架，仅 8 个长度字段变化；参数化编码后
  六个完整对象均可逐字节 round-trip。`object_05` 的 132 字节文件外壳也已按自身长度公式完整
  round-trip，但目前只有一份该变体抓包，证据强度低于主数据外壳。
- `object_06/07/08/11` 是同一控制消息族，标题和正文各以 UTF-16LE 序列化两次；参数化编码器对
  欢迎、认证错误、备用服务器和初始化完成四包逐字节一致。`object_13/14/15` 分别与
  `object_06/07/08` 完全相同。
- `object_12` 是请求/应答/模块/包名称/应答编号/初始化完成的复合控制包。当前只有一份权威样本，
  因此按固定语义模板保留，不臆造未获证据支持的动态字段。
- 完整对象目录、字段偏移、SHA 和 RED/GREEN 请求见
  `docs/forensics/wine-initialization-outer-objects-rust-validation-20260901.txt`。这里证明的是抓包格式，
  不替代动态 callback 顺序、异常路径或开盘质量验收。
- `probe_run_20260329_v3.txt` 的显式初始化调用进一步确认 callback 主序列为接口标题、沪/深代码表、
  除权、财务、文件、实时、空实时、初始化完成。认证服务器和备用服务器提示属于此前登录流程的
  条件消息，不能因为 catalog 中保存了其包型就硬编码到每次初始化中。

## 2026-09-01 实时 OEM 派生闭环

- 当前权威程序 `/home/codes/third_party/quoteNetzipWine/网际风.exe` 的 SHA-256 为
  `de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29`；当前 exporter 位于
  `0x4a7890`，金额 getter 位于 `0x40acc0`，派生字段分派位于 `0x441070`。
- 当前 6211 条记录的价格比例为 100 共 5335 条、1000 共 876 条；金额模式为 mode 0 共 158 条、
  mode 1 共 86 条、mode 2 共 5967 条。SH600000 金额的 `f32` 位模式为 `0x4ddb011a`。
- `change` 按证券分类选择 FIN `metrics[34]` 或 `metrics[35]`；同一次抓包的 `object_09`、
  `object_04` 和当前分类表可连接 5958 条，Rust 与 Wine 的结果 `5958/5958` 位级一致。其余
  321 条缺同次财务对象、19 条缺当前分类，未伪造进该对比。SH600000 的 change 位模式为
  `0x3e1bcd6c`；财务缺失或分母为零时为 0。
- 原始 realtime loader 保留全部 6211 个已占用槽；动态 OEM callback 输出长度
  `200 + 6187 * 500 = 3093700`，头部 count 为 6187，排除的正好是 24 个 `time=0` 槽。
  split/finance 初始化对象仍使用全部 6211 个已占用槽作为证券集合，并从 `实时.dat` 取名称，
  不能复用 OEM realtime 的 `time!=0` 过滤条件。
  `/api/supplement/realtime/oem` 已进入 stable 与 linux-native capability 清单。实现与证据日志见
  `docs/forensics/wine-realtime-dat-rust-validation-20260901.txt`。

## 2026-09-01 动态初始化逐字节闭环

- 隔离探针使用测试账号 `168/168`，完整保存 split `12620000`、finance `2101950`、file
  `1024926`、realtime `3093700` 和 empty realtime `200` 字节 callback。对应 SHA-256 分别为
  `54f5fc44713fbd956dd3e6a9c01e2fcbcb114843ce634a986c4fc5da42ac7ef3`、
  `8c4f864b8680756aad0d3dd1c15a2ebb9bb624da94ba9b4f311dfd1f55e2dfe0`、
  `ee8d385973d01d8db5ae9288666af36c36f4f43edb1dfd772f6afdb58cf28fc2`、
  `813f527259b06e41de1deb0a86528e0a76ca1f82c24c529dc43fe22e48400cca` 和
  `220a3bcf18331fe83db235036294f416476344b89cce8e98b2de0e65e6253bc5`。
- Wine `0x40ac30` 的 mode-0 高位标记分支不是 7709 packed-number：它清除最高位，将余值按
  `u64` 左移 16 位，再舍入为 `f32`。当前 `SH113624` 原值 `0x8000b09c` 对应
  `0x4f309c00`，`SZ123118` 原值 `0x80012077` 对应 `0x4f903b80`；共享测试固定这两个位级样本。
- webClx 请求 `070747-18d0ccb5d58be4c1` 证明五类动态 callback 与 Rust 编码逐字节一致。
  请求 `072942-18d0ccb5d58be4cc` 在 Rust `1.100.0-nightly` 及升级后的依赖上完成共享 crate、
  workspace、动态 fixture 和 release 回归；release SHA-256 为
  `4b72c41edb781aec9a1427d08e6cfa7fae724af87062ea25adb03d8c5f0a6470`。
  增量请求 `075108-18d0ccb5d58be4ce` 又单独执行了 `SZ123118` 高位标记金额位级样本，
  并完成 `netzip-supplement` 格式检查和 13 项全量测试。
- 两份动态代码表 callback 已保存，但仍缺同会话独立 7709 输入；不得通过解码 Wine 对象再编码
  自身来制造循环证明。开盘质量、失败路径、大页分页与并发负载也仍是独立验收项。

## 2026-09-01 盘后验收

- 现有 `netzip_linux` 诊断二进制直连 `120.195.71.160:7709`，对 `SH600000` 分别请求
  `1d / 5m / 1m` 三根，类别 `4 / 0 / 7` 均返回一个响应帧和三根可解析记录。
- 新 release `quoteNetzipRs` 的 `POST /api/supplement/kline` 在一次请求中执行三周期，
  `page_size=2` 强制每周期走两页，最终 `completed_items=3`、`failed_items=0`、
  `total_bars=9`，每项均为 `complete=true`。
- 三周期最后一根均为 `2026-08-31` 收盘数据；日线最后时间 `15:00:00`，5分钟最后时间
  `15:00:00`，1分钟覆盖 `14:58/14:59/15:00`。
- `page_size=801` 返回 HTTP 400，并明确报告合法范围 `1..800`；边界检查没有下沉成
  连接失败或服务器异常。
- 本次是盘后历史补数验收。它证明 052d 三周期、分页拼接和 HTTP 契约，不替代盘中实时
  推送及补数/全推并发负载验收。

## 2026-09-01 OEM K 线及代码表名称闭环

- 修复前的 7709 代码表现场响应被解析为 `000001d` 和 `999999d`。逐字段核对 29 字节记录后
  确认第 6..8 字节为成交单位，现场值 `0x64 0x00` 即 100；旧解析误把低字节字符 `d`
  拼到了六位代码尾部，导致 `SH600000` 无法命中名称。
- 当前仍位于 `netzip-fullpull`、目标迁往 `netzip-supplement` 的 7709 兼容实现固定从第
  0..6 字节读取证券代码，并用“代码 `600000` + 成交单位 100”
  的记录夹具覆盖该边界。修复后的真实同步共返回 50000 条，首尾代码恢复为 `000001` 和
  `999999`；第 6..8 字节同时作为 `volume_unit` 保留并进入同步 JSON/CSV，为后续映射
  `OEM_STKINFO.hand` 提供原始字段。`POST /api/supplement/kline` 不再需要调用方补名称即可
  返回 `浦发银行`。
- 新 release 在隔离端口接收不带 `name` 的 OEM 请求，日线、5分钟线、1分钟线各返回 HTTP
  200、`application/octet-stream` 和 296 字节。三者头部均为 `label=SH600000`、
  `name=浦发银行`、`len=96`、`count=3`、`flag=0`、`power=0`、`oemVer=0`；askId
  分别按请求保留为 6、7、8，每根记录的 `temp` 均为 0。
- 完整解析值和本次 Rust 响应 SHA-256 见
  `docs/forensics/oem-kline-rust-validation-20260901.txt`。这些散列只固定本次 Rust 运行结果，
  不能替代尚未取得的 Wine 原厂完整 answer 缓冲区。
- `tuwenca-codec` 已增加 200 字节 `OEM_MARKETINFO` 和 250 字节 `OEM_STKINFO` 编码器；
  `SZ000001` 原始 Wine 记录在把 UTF-16 首个 NUL 后的未初始化槽尾规范化为零后，250 字节
  fixture 对比完全一致。原始槽尾的 `0x65/0x6b` 等残留不作为协议常量复刻。
- 全量 fixture 已进一步覆盖 `object_01.bin` 和 `object_02.bin` 的完整内层对象：从抓包 CSV 重建
  SH 2965 条和 SZ 3366 条 snapshot 后，分别与 741650 字节、841900 字节 Wine 对象比较。只将每个
  UTF-16 固定槽首个 NUL 后的未初始化内存归零，其余 1583550 字节全部一致。该复核同时确认
  `OEM_DATA_HEAD.value[0]` 不是保留零值，而是 `OEM_MARKETINFO.date` 对应的中国本地零点 Unix 时间戳；
  20260327 对应 1774540800。
- 动态代码表集合以原 Wine 当前 vendor 接收清单 `用户/只接收股票代码表.csv` 为观测来源，不直接使用 7709 的
  50000 条全量集合。`POST /api/supplement/code-table` 已负责解析清单并按市场限定代码连接
  7709 元数据。2026-09-01 现场结果为 6954 个请求、5887 个命中、1067 个缺失；缺失项以退市证券和
  到期转债为主，均显式报告，未被静默删除或跨市场误连。

> 该文件在 2026-09-02 运行目录中虽以后缀 `.csv` 存在，实际内容是 vendor 的 UTF-16LE TAB 工作表（含配置头与启用标记），不应按普通 CSV 行号解释；
> 5188 `2a10` 的 wire index 必须先映射同一连接 `0104` 代码表。当前 6203/6203 命中仅是集合归属
> 证据，尚未证明第六/第七分区的排序和 slot 指派规则。
- `POST /api/supplement/code-table/oem` 已接入同一连接结果，并要求显式选择 SH 或 SZ；市场日期来自
  同一 7709 会话的最新日线，不使用系统日期。SH/SZ block、市场编号、指数/大盘标志、交易时间及
  涨跌停比例均按 2026-03-27 全量抓包映射；`*ST国华` 仍按抓包编码为 `*STGH` 和正负 10%，未套用
  泛化的 ST 5% 规则。通用拼音库与 Wine 在 137 个多音字证券名称上存在差异，现以
  `(symbol,name,pinyin)` 抓包表精确覆盖，名称变化时回退通用算法；全量测试要求 SH/SZ 抓包中的
  6331 条拼音全部一致。新 release 的隔离 HTTP 验收返回 SH 2579 条和 SZ 3308 条，合计正好等于
  JSON join 的 5887 个命中；长度分别满足 `200+200+count*250`，市场日期和头部零点时间戳均为
  20260831，旧抓包同名重叠记录的拼音差异为 0。响应散列、代表记录和回归请求见
  `docs/forensics/oem-code-table-rust-validation-20260901.txt`。这同时闭环了“2026-03-27 抓包全包
  逐字节复刻”和“当前动态集合现场 HTTP”，但不代表除权、财务、文件对象已经复刻。
