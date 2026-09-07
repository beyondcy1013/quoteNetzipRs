# Progress Log

## 2026-09-04 zcode：P6/P7 官方类别序实现（fixture A 驱动，未动所有权文件）

- 破译：官方 P6/P7 = 一个类别枚举序列被 1024 条边界切开（P7 是 P6 的延续）。类别序：SZ301股票尾(=P5延续) → SH000指数族 → SH110/113转债 → SZ123转债 → SZ399指数 → SH51x ETF → SH900B → SZ159ETF → SZ200B，类内 symbol_index 升序。订阅值编码确认 `0x10000|symbol_index`（低15位=市场最大0104节索引，全部官方段落入该掩码）。
- 实现：hub 新模块 `partition_order.rs`（纯函数 `order_official_p6_p7_leftover`，phase1=股票 SH→SZ 升序，phase2=8 类枚举升序，未知类尾随）；`native.rs` 对接收清单计划的 P6+ 段重组并按 1024 重新切片。所有权文件（fullpull official_5188.rs）未改动。
- 验证：hub 23 项（含 3 个 fixture-A 排序测试）、workspace 全过、clippy `-D warnings`、i686 release 构建成功。
- 待办：第二 fixture（前一交易日官方样本）验证类别序跨日稳定；在线 168 复跑确认 2a10 与官方逐字节一致后，gaps 文档 P6/P7 项可标 done。

## 2026-09-04 zcode：补日线机制定案（文件差分，证据链闭合）

- 差分取证：补日线=1 的 2.5 分钟窗口内被写的文件 = 大智慧 B$/SH/SZ代码表.dat、用户/全部股票代码表.csv、数据/实时.dat、财务V6/V8.fin、除权V6/V8.pwr、飞狐侧 Report.QRT/StkData.sif。**DAY.HQD（SH/SZ/UI）完全未动**，其 mtime 停留在 2024/2026-03（遗留文件，网际风运行时不维护）。
- **定案：官方网际风的"补日线"不下载也不落盘日线**。它刷新的是构建日线所需的原料（除权+财务+代码表+实时快照），日线本身由 2704 实时通道的逐日收盘快照 + split.pwr 除权在服务端/客户端内存层构建。
- Rust 官方等价补日线实现路线随之明确：①解析 1504 载荷中的 split.pwr（netzip-supplement::parse_wine_pwr_v8 已能解析同格式文件，本轮抓包解出的 210,901B split.pwr 可直接做解析验收）；②从已解码的 2704 记录（含当日 OHLC）逐日累积日线；③按除权事件复权。无需任何额外下载协议。
- 下一步实现序：split.pwr 抓包样本解析验收 → 2704 日线累积器 → 复权投影。

## 2026-09-04 zcode：7100 受控实验 = 自动升级通道（官方补日线通道再排除一条）

- 受控实验（官方网际风 补日线=1，抓 5188+7100，约 2.5 分钟）：本次官方启动仍开 10×5188 + 1×7100；7100 上行 37KB/下行 28KB。用官方 Stock.字典 离线解码（新增 `decode_7100_zstd_frame_with_dictionary` 只读取证接口 + `probe_7100_files` 过滤式 example），**上行解码命中的全部是自动升级检查请求**（`.//autoupdate.ini`、`.//DzhHqServer.ini` + 校验和），下行 0 命中。7100 = 升级检查通道，不是日线下载。
- 结合上轮 5188 全分类，官方补日线三通道已排除两条：5188（无日线对象）、7100（升级检查）。剩余唯一候选是回环 2000（pktmon 采不到）与“本地已由 2704+split.pwr 构建、无网络下载”两种解释。
- 安全处置：含认证帧的 cap7100 pcap/etl/pcapng 已全部删除；配置已恢复 补日线=0；官方进程已结束；pktmon 过滤器已移除。仅保留文件名级证据（autoupdate.ini/DzhHqServer.ini + 校验和），不含任何凭据。
- 下一路线（不再需要抓网络）：Wine 端 DAY.HQD 补日线前后差分，确认日线是否纯本地构建；若是，Rust 官方等价补日线= 2704 收盘快照 + split.pwr 除权本地构建，无需额外下载协议。

## 2026-09-04 zcode：3f04 日线假设证伪（5188 对象类型全分类完毕）

- `3f04` 解压内容为 GBK 文本（"华能国际H股股价，报价截至时间：20260902收盘，AH比价分析"、"万科A：…公告"）——是逐股资讯/AH 比价推送，**不是日线**。此前"代码+日期=日线候选"假设被内容证伪。
- 补齐 `3638`（同为资讯，未压缩变体：H股公告/大宗交易/权益分派）。至此 cap-wjf.pcap 的 5188 数据面全部定性：0104 代码表、1504 配置文件、3e04→stkinfo6.fin 财务、1b04 拼音索引、2804 板块分类、3001 指标公式（散户线）、3f04/3638 资讯、2704 实时 delta、其余为 per-connection 小帧/一次性不透明对象（无压缩特征）。
- **决定性结论：5188 数据面不含日线 K 线**。网际风日线=实时 2704 收盘快照 + 除权文件（split.pwr，1504 载荷已取证）本地构建；缺口历史走 7100 下载通道（上轮实验刻意未抓）或回环 2000（pktmon 采不到回环）。
- 下一步若要官方等价补日线，唯一途径是新一轮受控实验：抓 7100 端口（会含官方账号认证帧，须先定脱敏/留存策略）或做 Wine 端 DAY.HQD 差分。

## 2026-09-04 zcode：补数据全部接通（分笔/F10/全集/间隔，端到端实测）

- `tdx7709` 新增历史分笔（pytdx 同构 0x0fb5 请求 + varint 解析，180 测试全过）；hub 补数通道补齐 分笔按日 CSV、F10 序号前缀 TXT、补全部股票（代码表全集）、批间隔；2 只小样端到端实测：分笔 4,238–4,690 条/日、F10 32 文件。
- GUI 六个补数选项全部生效；附带修正 0104 测试夹具 opaque_tail 偏移漂移。i686 release 构建成功。

## 2026-09-04 zcode：netzip_win GUI 补数据全量接通 7709 lane

- hub 新 API：`run_supplement_backfill(root, SupplementBackfillSpec)`，同批多周期（daily/5min/1min），输出分目录 CSV；分笔/F10 无 7709 实现时 GUI 如实提示跳过。
- 验证：netzip_win workspace 全过（hub 14 项）+ clippy + i686 release；引擎层已 64/64 实测。

## 2026-09-04 zcode：7709 第三方补数高失败率修复（重试 + 会话自愈）

- 根因：7709 页请求偶发 `no non-empty response frames received`；批量补数每批 64 只共用一会话且不重连，断连即整批失败（首跑 9/7177）。`netzip-supplement` 现页级重试 3 次/250ms，重试前重开会话刷新 name/volume 表。
- 实测：12 只 12/12、64 只整批 64/64（SH110 可转债 0 根为该源正常行为）。探针 example：`netzip-supplement/examples/probe_7709.rs`（可传符号清单）。
- 附带：Wine 财务/除权 fixture 测试改结构不变量（V6 payload=record_count×166+8），不再随官方刷新失效；修 6 处 clippy。
- 验证：supplement 15 passed + clippy -D warnings；netzip_win workspace 回归通过。

## 2026-09-04 zcode：交接线索文档（解码产物 / 3f04 候选 / bulk 短末块 / P6P7 反推素材）

- 新增 `windows_debug/zcode-handoff-20260904.md`：汇总 cap-wjf.pcap 已解码产物路径与复现命令、1504/3e04 分类结论、3f04 日线候选头部结构、fullpull bulk 短末块规则、ACK 67B 反推素材、P6/P7 离线反推方法和 Rust 回归基线。接续本线请先读该文档。

## 2026-09-04 zcode：官方补数对象离线解包（1504/3e04 分类完成）

- `cap-wjf.pcap` 经只读 pcapng 转换后交给 Rust `Official5188Reassembler`，恢复 904 个完整帧、4 份 0104、32,639 条 metadata seed；抓包停止产生的尾部 partial frame 不再导致整条流被丢弃，中间 TCP gap/malformed 仍 fail closed。
- 直接 `1504` 解压对象是 `split.pwr`、`pmd.txt`、`tpsvr.ini`、`net.xml`、`market_dids.ini`、`bkcode_shszbj.dat` 等配置/板块文件，非日线。
- 145 个 `3e04` 副本归并为 16 个唯一连续块（15×5,120 + 3,970 = 80,770）；各连接同 offset 内容一致。组装后是一个内层扩展长度 `1504`，解压得到 124,929 字节 `update/stkinfo6.fin`，属于财务/证券基础信息，非日线。
- `netzip-fullpull` 的 bulk envelope/assembler 已支持严格闭合的短末块，同时拒绝 gap、overlap、metadata 变化及非闭合短块；新增 `official_5188_bulk_extract` 只读取证 example。
- 当前日线首要候选转为 `3f04`：其 zlib 明文同时含证券代码和 `20260902`/`2026-09-02`。在记录字段与官方回调/OHLCVA 对齐前，不写 `DAY.HQD`，不宣称官方补日线完成。
- `netzip_win` GUI 将现有 7709 路径明确标为“第三方日线补数/7709 CSV”；原 `补日线` INI key 仅保留兼容性，不再把 7709 表述为官方网际风补数路径。
- 验证：fullpull 174 passed/1 ignored；netzip_win 30 passed/1 ignored；相关 Clippy `-D warnings` 和格式检查通过；i686 release 构建成功。quoteNetzipRs 109 passed/2 ignored，唯一失败是既有 pcapng 测试依赖本机未安装的 tcpdump，不影响本轮已成功的 Rust 解包结果。

## 2026-09-03 深夜 zcode：官方网际风实机抓包（补日线路径 / ACK 67B / P6-P7 划分）

- 受控实验取证官方补日线数据路径：抓数据面端口（5188/7709/2000/2001/22223，排除认证端口），置 补日线=1 后启动官方网际风约 3 分钟，事后恢复配置并结束进程。
- 决定性结论：网际风补数**零 7709 连接**——10 条 5188 + 1 条 7100 + 本地 2000；财务/代码表文件在启动后刷新，数据来自 5188 批量对象（1504/3e04 族）和 7100 下载。Rust 的 7709 补日线（netzip-supplement）应定位为独立第三方历史源，官方等价能力属于 5188/7100 lane。
- 每连接一分区证实：7 条连接各发 1 个 2a10（P1-P7），3 条仅初始化备用。P1-P5 与 Rust 构建器逐条一致；P6/P7 总数一致（1147）但成员划分不同——官方 P6=SZ301尾95+SH分类段、P7=SZ基金段 [67846,85]+[68305,38]，为 26 段分类枚举序提供完整实机样本。
- 登录 3610 95B、ABK 94B 官方与 Rust 输出逐字节相同；官方 ACK 3610=67B 已取全 hex（`9cb8e9f9…`），比我们 62B 多 5 字节，目标是修正 ACK manifest（market_rows 静态夹具）向官方真实文件内容对齐。
- 证据：`C:\Users\Administrator\ZCodeProject\netzip-live-test\cap-wjf.pcap`。

## 2026-09-03 深夜 zcode：Windows GUI 第三次启动对比（可复现性确认）

- 用含并行更新的最新 netzip_win 构建（netzip-driver-hub + 全集订阅 + 接收清单读取器）再次在线启动（168 + NETZIP_NATIVE_5188_INIT=1）：初始化再次被 222.85.139.177:5188 接受，23 秒 57 服务端帧/50 业务帧。
- 与第二轮逐字节可复现：3610 载荷、2d10 四词（group 0xB246）、2a10 七分区起点/段结构（6267 条）完全一致——同日同凭据下客户端输出确定，可作为审计基线。
- 对官方差异仍为两项已知：ACK 3610 62B（官方 67B，同在观测域）；P6/P7 SH-then-SZ 降级序 vs 官方 26 段枚举序。
- 证据 pcap：`C:\Users\Administrator\ZCodeProject\netzip-live-test\cap5188c.pcap`（仅 5188 端口，无认证流量）。

## 2026-09-03 zcode：Rust Native 在线初始化跑通 + 全集订阅实验（168 账号）

- 首次在线完整交错初始化：真实 5188 服务器接受当前会话 `3610x3/2d10x3/2a10`，随后推 1504/3e04 业务帧族；盘后无 2704，空闲 10060 结束。抓包仅过滤 5188 端口，无认证凭据。
- 帧级比对：3610 95B/94B 与官方一致；ACK 62B 在 40..=80B 观测域内；2d10 group=0xB246 与官方 Secondary 一致；2a10 P4 起点 66292 与 fixed-accept-2000 完全一致；P1-P3 +16/P5 +1 为跨交易日代码表平移，行为正确。
- 全集订阅已接入 Rust：`load_official_receive_list`（接收清单 UTF-16LE，第4列启用）+ `build_official_5188_receive_list_partitions`，在线发送 7 分区 = 6×1024+123 = 6267 条（清单增长到 7177 行），服务器接受。P6/P7 为 SH-then-SZ 降级序，与官方 26 段枚举序不同；顺序敏感度待交易时段 2704 覆盖对照实验判定。
- crate 改名：netzip-drivers → netzip-driver-hub（上级管理层语义），workspace 全测试/Clippy/i686 release 通过。
- 待办：交易时段全集 2704 覆盖统计；per-slot 编排；2704→OEM 发布门禁；ACK market_rows 静态夹具仍未转运行时派生。

## 2026-09-02 zcode：Rust EXE 与原版网际风差异矩阵

- 静态二进制：均为 x86 PE32 GUI 且未签名；Rust 版约 1.26 MB、ASLR/DEP、asInvoker，原版约 2.07 MB、DEP、requireAdministrator，原版还包含广泛插件/更新/进程操作/历史数据能力。此类实现表面差异不是 fullpull parity 判据。
- 关键行为差异：Rust 当前在 worker 启动后即建立 UI 连接句柄，并可能在未初始化/未解码时由首个任意服务端帧上报 Connected；原版 readiness 有完整初始化及持续 OEM 回调证据。Rust Connected gate 是当前最高优先级 correctness 缺口。
- Rust Native 单 socket 且默认不初始化；原版样本是当前生命周期动态分配的多 slot（两次观察均为 10，但不是协议常数），每 slot 有动态 3610/2d10/2a10。
- framing/reassembly 和固定样本 2704 内部 311B 记录恢复已强验证；2704 到 OEM_REPORT public-state merge、amount/ladder、batch aggregation 尚未闭合，不能发布为正式实时行情。
- Rust 未替代原版本地 2000/2001/5188/22223 兼容平面、历史/F10 调用和生产 reconnect/failover。7709 仍只允许作为独立 supplement，不进入 fullpull Connected 判定。
- 建议实施序：Connected/capability truth -> 动态 multi-slot init -> 2704/OEM callback parity -> authenticated reconnect/failover -> 独立历史/F10 supplement。

## 2026-09-02 zcode：`diagnostics/netzip_api` 官方 Demo 审计

- 静态核对 C++、C#、Python Demo 和股票接口调用规范；未运行或加载目录内官方二进制。
- 官方 DLL 导出边界为 `Start / Ask / Stop`，权威回调合同由 C++ 和 Python一致确认是三参数 `(form, data, askId)`、Windows StdCall。C# Demo 漏写第三个 `askId`，属于版本/示例不一致，不应覆盖三参数证据。
- x86 MSVC 19.50 实测官方 C++ 头文件：`OEM_DATA_HEAD=200`，关键偏移为 `len@20/count@24/label@28/name@52/value@178/flag@190/askId@191/power@195/oemVer@196`。此前 `len@40` 是把 Windows `WCHAR(2B)` 错当成 4 字节所得，已在 Rust callback 复制边界中修正。
- `netzip-vendor` 已新增安全 OEM parser：精确验证总长、count、溢出与 `REPORT=500/TICK=100/KLINE=32` 步长；未知类型保留 raw。
- `netzip-drivers` 已将有效 DLL 实时、分笔、K 线包转为结构化事件和正确 `DataClass`，畸形实时包不会发布为 Realtime；GUI 已支持对应摘要。
- 验证通过：vendor 9 tests；drivers 9 passed/1 live ignored；fmt、严格 Clippy、i686 MSVC release build。Native 初始化门禁及 2704 上线状态未改变。
- `OEM_MARKETINFO=200` 指固定市场头，其后是动态 `num * OEM_STKINFO(250)`；Python 的 `stkInfo[10]` 只是访问占位。Python Test 中头长“100 字节”的注释是笔误。
- 公开投影尺寸：实时 `OEM_REPORT=500`、分笔 `OEM_TICK=100`、K线 `OEM_KLINE=32`；实时时间是 Unix epoch seconds，实时行情由 callback 自动推送。
- 上层生命周期确认：Start -> 认证登录 -> 股票备用登录 -> 初始化 -> Ask/异步回调 -> callback 清空后 Stop。callback 要先复制再交队列，不能在 DLL 线程中重处理。
- 本资料能用于 Native 2704 到 OEM/public-state 的结构 parity，但没有披露 5188 动态初始化 payload，不能证明或生成当前会话的 `3610/2d10/2a10`，因此不解除 Native 初始化门禁。

## 2026-03-29

- 已读取：
  - `../../AGENTS.md`
  - `../../PROCESS_stockScreener.md`
  - `../../PROCESS_cppStockServer.md`
  - `../../stockScreener/AGENTS.md`
  - `../../quoteGateway/AGENTS.md`
- 已确认本任务属于复杂多阶段任务，启用 `planning-with-files` 工作方式。
- 已建立 `task_plan.md`、`findings.md`、`progress.md`。
- 下一步：
  - 缩小 `JYS/FoxTrader` 相关代码和文档范围
  - 判断“正在处理解析各字段”的具体模块
  - 准备 `FoxTrader.exe` 的监控抓包启动方案

- 已补充完成：
  - 定位 Rust `quote-gateway` 中的 `jys.rs` / `source_ingest.rs`
  - 确认 TCP 二进制协议常量：`JYQT` / 20-byte header / 116-byte quote record
  - 从 `docs/archive/current_replica_status_history_before_20260324.md` 中提取出 ABI 错版与 TCP 协议演进历史
  - 确认 `FoxTrader.exe` 存在且当前未运行
  - 确认本机可用抓包工具：`pktmon`、`netsh`

- 已执行 `FoxTrader.exe` 监控抓包启动：
  - 启动时间：约 `2026-03-29 22:27:26`
  - 抓包目录：`./foxtrader_capture_20260329_222724`
  - 产物：
    - `foxtrader_pktmon.etl`
    - `foxtrader_pktmon.pcapng`
    - `foxtrader_pktmon.txt`
    - `summary.txt`
- 本次现场结论：
  - `FoxTrader.exe` 当前无同目录子进程协同运行
  - 其自身只对 `127.0.0.1:2000/2001` 发起 `SynSent`
  - 当前未见本地 `2000/2001` 监听者
  - 因而“FoxTrader 直接出网抓行情”在这次现场里没有被证实

- 已继续补充本地桥接线索：
  - 确认 `FoxTrader.exe` 运行态加载了 `D:\\Soft\\_Stock\\飞狐2020\\系统\\Stockdrv.dll`
  - 从 `Stockdrv.dll` 中提取到 `127.0.0.1`、`GetStockDrvInfo` 与多条 socket 错误字符串
  - 从 `FoxTrader.exe` 本体二进制提取到 `HqOnline.ini`、`HQ_Server`、`Cur_Server`、`Auto_Conn`、`Cycle_Conn` 等服务器配置键
  - 发现安装树顶层还存在 `D:\\Soft\\_Stock\\飞狐2020\\网际风.exe`，当前未运行
  - `网际风.exe` 二进制中可见 `127.0.0.1`、`www.nezip.cn`、`tdxlevel2`、`CTcpCEx`、`CTdxQh`
- 当前最值得继续验证的假设：
  - `FoxTrader` 依赖本地行情桥而不是自己直接主出网
  - `网际风.exe` 是 `127.0.0.1:2000/2001` 缺失对端的高概率候选
  - 下一步若继续现场验证，应把 `网际风.exe` 纳入监控与抓包闭环

- 已补充读取安装与配置文档：
  - `安装说明.txt` 明确要求运行 `网际风.exe`，并注明不要单独关闭，否则收不到数据
  - `安装说明.txt` 同时说明 `网际风` 盘中自动推送实时数据，并负责“补日线”“补5分钟”等历史补数能力
  - `用户\\配置文件.ini` 读到当前接口模式为 `客户端`，并配置了自动登录/自动退出/自动隐藏与补日线、补5分钟、补1分钟参数
  - `升级\\升级说明.txt` 的 `2021-02-08` 条目说明存在“TCP直接通信方式”，可以不使用 `Stockdrv.dll`，也不需要 `网际风` 界面
- 这使当前判断从“二进制字符串推断”升级为“文档与运行现场相互印证”：
  - 当前只启动 `FoxTrader.exe` 并不等于完整运行正常客户端环境
  - 缺失的不是一个普通远端地址，而是飞狐生态中的本地接口/桥接组件

- 已完成关键运行态验证：
  - 启动 `网际风.exe` 后，进程 `PID 70108` 成功运行
  - `网际风.exe` 监听了 `127.0.0.1:2000/2001/5188/22223`
  - 原先 `FoxTrader.exe` 对 `127.0.0.1:2000` 的 `SynSent` 立即转为 `Established`
  - `FoxTrader.exe` 当前本地链路为：`127.0.0.1:3961 -> 127.0.0.1:2000`
  - `网际风.exe` 对外建立了：
    - `39.108.103.69:6100`
    - 多条 `120.195.71.160:7709`
  - 二次复查中 `2001` 仍只监听未用，当前短时现场主链明确落在 `2000`
- 至此，本轮最关键的目标已经闭环：
  - 找到并验证了 `FoxTrader` 缺失的本地对端
  - 确认 `FoxTrader` 不是直接主出网，而是通过 `网际风` 作为本地行情桥接到远端服务器

- 已继续把端口角色与盘后流量形态补清：
  - `用户\\配置文件.ini` 中 `[第三方调用]` 明确配置了本地 `2000/2001`
  - 同文件 `[大智慧服务器]` 明确配置了本地 `5188/22223`
  - `用户\\服务器列表.ini` 明确列出 `39.108.103.69:6100/7100` 与 `121.41.70.217:6100/7100`
  - 现场实连的 `39.108.103.69:6100` 与配置完全一致
- 已做 10 秒定向抓包：
  - 目录：`./foxtrader_capture_20260329_225700_focus`
  - 结果：本窗口内几乎只有 `7709` 报文，且主要是 `54/66/80` 字节短包
  - `2000/2001/6100` 在该窗口内未见可见报文事件
  - 初步判断盘后状态偏向“多连接保活/心跳”，不像盘中持续大流量推送
- 已补充运行态结构观察：
  - `网际风.exe` 无额外子进程
  - 模块列表里未见额外 `stock.exe` / `客户端.dll` 运行态模块

- 已补充文件侧观察：
  - `网际风.exe` 启动后约 2 秒内刷新了：
    - `用户\\配置文件.ini`
    - `用户\\只接收股票代码表.csv`
    - `用户\\全部股票代码表.csv`
  - 启动后约 30 秒内刷新了：
    - `数据\\财务V6.fin`
    - `数据\\财务V8.fin`
    - `数据\\除权V6.pwr`
    - `数据\\除权V8.pwr`
  - 当前未见同时间段持续滚动的日志文件
- 这进一步支持当前判断：
  - 盘后启动会做一轮必要的代码表/财务/除权刷新
  - 刷新完成后进入低活跃保活，而不是持续高频下载

- 已完成官方 `Stock.dll` 实机探针验证：
  - 临时探针位置：
    - `./tmp_netzip_probe_20260329/StockProbe.cs`
    - `./tmp_netzip_probe_20260329/StockProbe.exe`
  - 运行输出：
    - `./tmp_netzip_probe_20260329/probe_run_20260329_v3.txt`
- 本轮探针的关键结论：
  - `D:\\Soft\\_Stock\\飞狐2020\\Stock.dll` 是 `x86 / PE32`
  - 当前真实安装目录没有 `Stock64.dll`
  - x86 探针 `Start ret=1`
  - 第一条回调文本是：`连接 网际风.exe 成功`
  - 这证明现场的 `Stock.dll` 已能在当前飞狐环境里正常挂到 `网际风.exe`
- 已确认 `Ask(...)` 至少存在两种返回形态：
  - 文本控制回包：
    - 登录请求 `ret=0`，结果通过异步 JSON 回调返回
    - 初始化请求 `ret=70`，同步文本为 `初始化完成`
  - 二进制结构回包：
    - `实时数据 ret=700 = 200 + 500`
    - `1分钟线 ret=296 = 200 + 3 * 32`
    - `日线 ret=296 = 200 + 3 * 32`
- 已对 `SH600000` 做实测：
  - `实时数据` 成功解出：
    - `浦发银行`
    - `close=10.02`
    - `time=2026-03-27 15:00:00`
  - `1分钟线` 成功解出最后三根：
    - `2026-03-27 14:58:00 +08:00`
    - `2026-03-27 15:00:00 +08:00`
  - `日线` 成功解出最近三日：
    - `2026-03-25`
    - `2026-03-27`
- 这让当前任务从“只确认本地桥接链路”推进到“官方 DLL API 也已现场跑通”：
  - 既确认了 `FoxTrader -> 网际风 -> 上游` 的链路
  - 也确认了 `Stock.dll -> Start/Ask/Stop -> JSON/二进制回包` 的上层行为

## 2026-03-29 23:50 之后进展：本地 `2000` 协议层已拿到首批真实包体

- 新增证据链一：`Stock.dll` 自己会连本地 `2000`
  - 文件：`./tmp_netzip_probe_20260329/stockprobe_pid_conn_watch_20260329.txt`
  - 结论：`StockProbe.exe` 存活期间，自身持有 `127.0.0.1:* -> 127.0.0.1:2000` 的 `Established`
- 新增证据链二：裸发文本到 `2000` 不通
  - 4 种 framing 都测了，全部 `received=0`
  - 说明本地口不是简单的“发官方请求字符串 -> 收官方回包”
- 新增证据链三：Frida 已抓到 `2000` 真实 payload
  - 主日志：`./tmp_netzip_probe_20260329/frida_ws2_trace_20260329_v10.log`
  - 去 ANSI 版：`./tmp_netzip_probe_20260329/frida_ws2_trace_20260329_v10_stripped.log`
  - 首个发送包：
    - `send len=378`
    - 前缀 `51 7f dc 7e 05 53`
    - 可见 `penc` 与 `hypenc`
  - 首个接收包：
    - `recv len=1460`
    - 同样出现 `51 7f dc 7e 05 53`
    - 同样出现 `penc`
  - 后续接收：
    - `recv len=292`
    - 大量 `recv len=10240`
    - `10240` 大包总数达到 `2542`
- 新增证据链四：大包里直接带证券代码
  - 例子：
    - `SH600028`
    - `SH512900`
  - 说明 `2000` 本地链路承载的是实盘/初始化证券数据，而不只是控制消息
- 新增技术判断：
  - 本地 `2000` 协议与仓库已确认的远端 `6100/7100` `网络包/penc` 外壳高度同源
  - `Stock.dll` 运行时 I/O 走的是带 `NtDeviceIoControlFile` 的低层异步路径，单盯 `send/recv` 不够
- 当前阶段结论：
  - `FoxTrader` 正常运行所需的不只是“知道上游 IP 和端口”
  - 还包含一层本地 `2000` 二进制桥协议
  - 如果要在 Rust 侧复刻成“像正常 Windows 客户端一样”，必须决定是：
    - 兼容这层本地桥协议
    - 还是绕过它，直接兼容 `Stock.dll` 上层行为

## 2026-03-30 新进展：`recv` 块分类已经完成

- 新增分析产物：`./tmp_netzip_probe_20260329/recv_boundary_analysis_20260330.txt`
- 已确认 `socket=0x1168 -> 127.0.0.1:2000` 的 `recv` 总数为 `2560`
- 其中：
  - `2542` 个是 `len=10240`
  - 但只有 `3` 个 `10240` 块从 `网络包` 头起始
- 已把本地 `recv` 切成三类：
  - 完整单包：`12` 个，`len == declA`
  - 小包合并：`3` 个，`len > declA`
  - 大对象起始块：`3` 个，`len=10240` 且 `declA >> 10240`
- 当前最重要的工程结论：
  - `10240` 是搬运块大小，不是协议包大小
  - `recv` 边界不能直接拿来切协议
  - 后续若继续逆本地 `2000`，必须转成“按包头 + 长度字段重组”的解析方式

## 2026-03-30 第二轮采样：已拿到单个 `recv` 内的串包证据

- 调整项：
  - `frida_ws2_trace.js` 的 hexdump 上限已从 `256` 提高到 `1024`
  - ANSI 颜色已关闭，后处理更简单
- 新产物：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11.log`
  - `./tmp_netzip_probe_20260329/recv_boundary_analysis_20260330_v11.txt`
  - `./tmp_netzip_probe_20260329/recv_multi_header_sequences_20260330_v11.txt`
- 已确认本地 `2000` 的小包不是“每次 `recv` 一包”，而是“长度前缀串包”：
  - `600 = 288 + 312`
  - `1188 = 288 + 368 + 532`
  - `1264 = 340 + 288 + 304 + 332`
- 这意味着后续如果真要写解析器，主循环已经很明确：
  - 先在缓冲区找包头
  - 再读当前包的 `declA`
  - 消费一个完整包后继续吃下一个
- 与此同时，`10240` 大块仍然存在，但当前更像：
  - 大对象正文的搬运块
  - 而不是本地控制小包的天然边界

## 2026-03-30 新进展：小包自动重组器与大对象专用采样器都已落地

- 新增小包解析脚本：
  - `./tmp_netzip_probe_20260329/parse_local_2000_small_packets.py`
- 已基于 `v11` 日志生成：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11_parsed_small_packets/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11_parsed_small_packets/packet_index.json`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11_parsed_small_packets/extracted_packets/*.bin`
- 当前自动重组结果：
  - 完整小包 `19` 个
  - 小包头部不变量已经跑实：
    - `field40=2`
    - `field48=0`
    - `field52=9`
    - `field56-field44=28`
    - `field44+68=declA`
    - `penc` 首次出现位置固定在 `60`
- 新增大对象专用脚本：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_large_recv.js`
  - `./tmp_netzip_probe_20260329/extract_large_recv_chunks.py`
- 已基于 `v12` 日志生成：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v12_large_recv.log`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v12_large_recv_chunks/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v12_large_recv_chunks/recv_large_*.bin`
- 这轮让大对象层也出现了新的硬证据：
  - 首块带外层 `网络包/penc` 头
  - 后续续传块不再重复外层头
  - 大对象正文中出现稳定 `250` 字节步长的 UTF-16 证券记录阵列

## 2026-03-30 文档对账与主仓库修正

- 已重新通读并对账：
  - `windows_debug/findings.md`
  - `windows_debug/loop2000_local_protocol_notes_20260329.md`
  - `windows_debug/tmp_netzip_probe_20260329/probe_run_20260329_v3.txt`
  - `windows_debug/tmp_netzip_probe_20260329/recv_boundary_analysis_20260330_v11.txt`
  - 主仓库 `README.md`
  - 主仓库 `PROTOCOL_NOTES.md`
- 已修正主仓库里之前过度直连的表述，当前统一改成三层视图：
  - 上层 API：`Start / Ask / Stop`
  - 本地桥：`127.0.0.1:2000 -> 网际风.exe`
  - 远端链：`6100 / 7100 / 7709 / 7719 / 7708 / 14017`
- 已明确写回主文档的纠正点：
  - `Ask(...)` 明文请求串是上层调用语义，不等于远端 wire payload
  - `Ask(...)` 成功返回并不总是 `OEM_DATA_HEAD`
  - 当前现场主用的是 `x86 Stock.dll`，不是 `Stock64.dll`
  - `2000` 是当前 `FoxTrader` 主链入口，`2001` 处于监听未见主链使用
  - `5188 / 22223` 更像其它兼容接口，不是本次 `FoxTrader` 主链
  - `10240` 是搬运块大小，不是协议边界
- 已把后续工作重新拆成并行主线：
  - 本地 `2000` 包重组器
  - `Ask(...)` 文本/JSON/OEM 三类回包抽象
  - 远端 `7100` 的 `penc / ZSTD字典 / Tdx_Encrypt`
  - 跨层对位 `本地2000 <-> 远端6100/7100`
- 已尝试启用多 agent 并行做文档修正和任务拆分：
  - `Lorentz` 成功返回了对账摘要，已被主线程吸收
  - 其余多个子 agent 在远端提供方处返回 `403 Forbidden: insufficient balance`
  - 当前已改为主线程继续执行，不等待子 agent

## 2026-03-30 CLI 入口与新结论对齐

- 已补一个最小代码修正：
  - `src/main.rs` 的交互式示例现在在 `Ask ret > 0` 且无法识别为 `OEM_DATA_HEAD` 时，会继续尝试把返回缓冲识别为文本回包
- 这次修正的直接原因是现场已跑实：
  - `Ask(...)` 可能同步返回文本/JSON，而不是只返回 `OEM_DATA_HEAD + payload`
- 当前效果：
  - 如果示例收到同步文本回包，不再统一打印“长度不足以解析头部”
  - 会优先尝试输出 UTF-16LE / UTF-8 文本内容
- 同时补了最小回归测试，覆盖：
  - `UTF-16LE JSON` 文本识别
  - `UTF-8 JSON` 文本识别

## 2026-03-30 本地 `2000` 重组器已落地到 Rust

- 已新增 `src/local_2000.rs`，把 `windows_debug/tmp_netzip_probe_20260329` 下本地桥日志的解析逻辑正式落到 Rust：
  - 按 `declA` 连续消费，不再按 `recv` 边界切包
  - 保留每个 `recv` 的头命中、长度字段、`penc/hypenc` 偏移、UTF-16 代码提示
  - 自动识别大对象起始包，并把后续 continuation chunk 重组到同一外层对象
- 当前实现已明确兼容：
  - `[recv]` 与 `[recv-large]`
  - 新版 `recv-large index=...`
  - ANSI 颜色日志
  - `NNN:` 行号前缀摘录
  - continuation chunk 中间偏移处出现下一对象头的边界情况
- 已把能力接入主服务：
  - `POST /api/debug/local-2000-log-scan`
- 当前回归通过：
  - synthetic：单次 `recv` 串包
  - synthetic：大对象跨块重组
  - synthetic：`[recv-large] + ANSI + 行号前缀`
  - 真实样本：`frida_ws2_trace_20260330_v11.log`
- 这轮还额外加了一条安全边界：
  - 只有当 `代码表/除权` 这类 `OEM_DATA_HEAD` 对象的声明总长已经被当前捕获字节完整覆盖时，才继续做内层摘要
  - 对不完整大对象，当前只输出外层重组与代码步长，不再冒险喂 `unsafe` OEM 解析

## 2026-03-30 深夜进展：`Ask(...)` / callback 统一消息模型已落地

- 已新增 `src/stock_message.rs`，把 Windows 现场已经坐实的三类返回统一成一个可测试模型：
  - 同步 `ret=0` 但请求已受理，后续结果走 callback
  - 同步文本/JSON 回包
  - `OEM_DATA_HEAD + payload` 结构化回包
- 库层新增导出：
  - `interpret_sync_answer(...)`
  - `interpret_callback_ptr(...)`
  - `interpret_callback_form_data(...)`
  - `StockApi::ask_message(...)`
- 当前统一模型会直接保留这些对账字段：
  - 文本：`text_request / text_data`
  - OEM 头：`packet_type / packet_label / packet_name / packet_count / packet_len / packet_flag / packet_ask_id / packet_power / packet_oem_ver`
- 这轮还把两个容易混淆的现场事实固定进模型与文档：
  - `ret=0` 不能再当成失败；登录样本就是“同步空返回 + callback 成功”
  - 请求串里的 `编号` 不能直接当成回包 `ask_id`；现场 `编号=101/102/103` 对应的是 `ask_id=4/5/6`
- `src/main.rs` 的交互式示例也已经切到统一模型：
  - 同步返回按 `accepted_async / Text / Packet / Empty` 分类输出
  - callback 统一按 `form + kind` 输出
- 当前回归通过：
  - `cargo test stock_message --lib`
  - `cargo test --bin netzipapi-rust-demo`
  - `cargo build`

## 2026-03-30 深夜进展：第 19 阶段已拿到首批跨层对位锚点

- 主线程已把 `src/local_2000.rs` 和 `src/auth_7100_prefix.rs` 的现有摘要字段并排核过一轮，当前可直接作为跨层对位锚点的常量已经有：
  - 两层都稳定出现 `field40 = 2`
  - 两层都稳定出现 `field56 - field44 = 28`
  - 本地 `2000` 的 `penc` 稳定落在完整小包 `offset=60`
  - 远端 `7100` 已把同一组差值稳定抽象成 `fixed_overhead_len_hint = 28`
- 大对象侧也已有一个可直接对位的候选：
  - 本地 `2000` 第一批初始化大对象起始包稳定出现 `field52 = 9`
  - 远端 `7100` 的 `1540` 字节 `下载文件` 包也稳定出现 `field52 = 9`，且当前已能与 `9` 个顶层可见子项对上
- 这说明第 19 阶段的起点已经不是“是否同源”的泛判断，而是可以直接围绕这些字段做逐项校对：
  - `field44 / field56 / field52` 的语义是否跨层保持一致
  - 本地 `penc/hypenc` 标记位和远端 `compressed_zstd / compressed_zstd_dict / download_file_record` 的对应关系

## 2026-03-30 凌晨进展：第 19 阶段已落地首个跨层对位器

- 已新增 `src/local_2000_vs_auth7100.rs`，直接把：
  - 本地 `2000` Frida 日志里的完整小包前缀
  - 远端 `7100` 样本 `pcap/pcapng` 里的前缀提示
  收敛到同一张对位摘要表
- 当前样本已稳定输出的共享不变量：
  - `field40 = 2`
  - `declared_len == duplicate_len`
  - `fixed_overhead_len_hint = 28`
- 当前样本已稳定输出的差异位：
  - 本地 `2000`：`header_len = 68`、`attr_flags = 9`、`penc@60`、少量 `hypenc@68`
  - 远端 `7100`：`attr_flags = 0/9`、`object_type = 1/4`，并已分化出 `compressed_zstd / compressed_zstd_dict / download_file_record`
- 继续往下压一层后，当前精确桥接结果已经更明确：
  - 按 `field40 / attr / fixed_overhead / header_len / outer_wrapper` 这 5 组导出信号做精确桥接
  - 本地 `2000` complete-small 当前只命中远端 `download_file_record / object_type=1`
  - 本地初始化大对象起始包现在也命中同一组 `download_file_record / object_type=1`
  - 同一套桥接条件不会命中 `compressed_zstd / compressed_zstd_dict`
  - 这意味着本地 `field48=0` 暂时不像远端 `object_type_id`，更可能还是本地桥层自己的保留位
  - 新补的 boundary delta 结论也已经坐实：
    - 本地 `complete-small`、本地大对象起始包、远端 `1540 download_file_record` 的首个 `penc` 都在 `payload boundary - 8`
    - 三者首个 `penc` 的精确偏移也已经对到同一个 `60`
    - 本地 `hypenc` 则贴着 `payload boundary`
    - 远端 `1540` 额外还能直接看到 `penc_offsets = [60, 416]`
    - 继续往 payload 内一层看，远端第二个 `penc@416 = payload boundary + 348`，但本地大对象后续 `penc` 当前落在 `payload boundary + 2 / +190 / +53888`
    - 所以当前真正剩下的缺口已经不是“首个 `penc` 有没有对上”，而是“首个 `penc` 之后 payload 内更深一层壳为什么分叉”
    - 本地大对象起始包当前完整重组结果里还能直接看到 `penc_offsets = [60, 70, 258, 53956]` / `hypenc_offsets = [68]`
      - 其中较早的 `60 / 70 / 258` 仍然对应此前 `recv_large_01` 首块观察；新增的 `53956` 来自继续拼接后的更深正文
- 已把能力接入 HTTP 服务：
  - `GET /api/debug/local-2000-vs-auth-7100`
  - `POST /api/debug/local-2000-vs-auth-7100`
- 当前回归通过：
  - `cargo test local_2000_vs_auth7100 --lib`
  - `cargo build --bin netzip_service`
  - `GET /api/debug/local-2000-vs-auth-7100`
  - `POST /api/debug/local-2000-vs-auth-7100`

## 2026-03-30 晚间进展：开始把 `windows_debug` 现场结论回灌到主仓库文档

- 已启动“主文档回灌”工作，目标文件：
  - `README.md`
  - `PROTOCOL_NOTES.md`
- 本轮回灌优先修正的是分层表达，而不是新增抓包结论：
  - 把 `Ask(...)` 明确放回“上层 API 语义层”
  - 把 `127.0.0.1:2000` 明确为“本地二进制桥协议层”
  - 把 `6100/7100/7709` 明确为“远端会话层”
- 当前重点是纠正历史文案中的混层风险：
  - 避免把 `Ask` 请求串直接等同为“发送到 `6100` 的 wire payload”
  - 避免把本地 `recv=10240` 误读成协议消息天然边界
  - 保留“本地壳层与远端壳层高度同源”这一证据，同时明确“尚未证明逐字节同构”
- 已按 `windows_debug` 证据链建立回灌边界：
  - 事实来源以 `findings.md`、`loop2000_local_protocol_notes_20260329.md`、`tmp_netzip_probe_20260329/*` 为主
  - 仅将已跑实的现场事实写入主文档
  - 仍未坐实的映射关系继续留在“未确认/下一步”而不提前下结论

## 2026-03-30 深夜进展：主仓库文档分层校正已基本落地

- `README.md` 已补上“当前分层全景”：
  - 明确区分 `Start/Ask/Stop` 上层 API、`127.0.0.1:2000` 本地桥、`6100/7100/7709/...` 上游远端链
  - 明确现场主用的是 `x86 Stock.dll`，不是把仓库里的 `Stock64.dll` 直接等价成现场主链
  - 明确 `Ask(...)` 请求串不应直接当成远端原始网络帧
- `PROTOCOL_NOTES.md` 已补上“Windows 现场补证与分层视图”：
  - 明确 `Ask(...)` 属于上层 API 语义
  - 明确 `127.0.0.1:2000` 是带 `网络包/penc/hypenc` 的本地二进制桥
  - 明确 `recv len=10240` 是搬运块大小，不是协议包边界
  - 已把“未确认/下一步”改写成分层后的表述，避免继续写成 `Ask -> 6100` 的直连模型
- 已同步清理一处残留用户提示：
  - `src/main.rs` 的非 Windows 提示不再只写 `Stock64.dll`
  - 现在会同时提醒 `Stock.dll / Stock64.dll`、`Stock.字典`、`网际风.exe` 与本地 `2000` 桥
- 当前这轮回灌的状态可以认为已从“开始”推进到“主干完成”：
  - 主要混层表述已经纠正
  - 后续重点转回实现侧：本地 `2000` 重组器、`2000 vs 6100/7100` 壳对位、`Ask` 回包统一抽象

## 2026-03-30 最新进展：大对象代码表已按官方结构拆开

- 已把 `./tmp_netzip_probe_20260329/split_large_record_array.py` 从“固定步长猜测器”升级为：
  - 优先按 `OEM_DATA_HEAD -> OEM_MARKETINFO -> OEM_STKINFO[]` 解析
  - 找不到官方结构时才退回 generic 步长模式
- 这次对照的官方头文件是：
  - `../netzip_api_bin/NetzipAPI/StockC++/OemStock.h`
- 重跑产物：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v12_large_recv_chunks/record_array_split/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v12_large_recv_chunks/record_array_split/markets.csv`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v12_large_recv_chunks/record_array_split/records.csv`
- 新脚本已坐实的关键偏移：
  - 内层 `OEM_DATA_HEAD` 在全流 offset `264`
  - `OEM_MARKETINFO` 在全流 offset `464`
  - 第一条 `OEM_STKINFO` 在全流 offset `664`
- 已直接解出的对象语义：
  - `type = 代码表`
  - `name = 上海证券代码表`
  - `count = 2965`
  - `len = 741450`
  - `market = SH / 上海证券交易所`
  - `date = 20260327`
- 已直接解出的首条证券记录：
  - `SH000001 / 上证指数 / SZZS`
  - `last = 3889.080078`
  - `limitUp = 4277.990234`
  - `limitDown = 3500.169922`
- 当前抓包覆盖范围也因此更清楚了：
  - 这 4 个 `10240` 大块只覆盖了该代码表对象的前 `40696 / 741650` 字节
  - 当前成功导出 `161` 条完整 `OEM_STKINFO`
  - 末尾还残留 `46` 字节，说明下一条记录被截断
- 这一步的实质意义是：
  - 大对象层已经不再停留在“可能是 250 字节数组”
  - 而是已经能用官方结构名词稳定描述，并导出可核对的 `CSV + bin` 样本

## 2026-03-30 继续推进：已完整抓到上海代码表，并定位到后续深圳代码表

- 为了不再只停留在前 `4` 个 `10240` 大块，这轮新增了：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_code_table_full.js`
- 新日志：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v13_code_table_full.log`
- 第一次提取时遇到一个小问题：
  - 新日志把 `recv-large` 行扩成了 `index=... socket=...`
  - 旧版 `extract_large_recv_chunks.py` 正则只认旧格式
  - 已修正脚本，兼容新旧两种 `recv-large` 行
- 重跑后产物：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v13_code_table_full_chunks`
  - 其中 `96` 个 `recv_large_*.bin` 已全部导出
- 再次重跑 `split_large_record_array.py` 后，第一张代码表已经完整落地：
  - `head_capture_complete = True`
  - `records_extracted = 2965`
  - 首条：`SH000001 / 上证指数`
  - 末条：`SH900948 / 伊泰Ｂ股`
- 当前最重要的新发现不是“上海代码表抓全了”本身，而是：
  - 第一张代码表之后，紧接着就出现第二个 `OEM_DATA_HEAD`
  - 该对象是 `深圳证券代码表`
  - `count = 3366`
  - 首条记录已可见 `SZ000001 / 平安银行`
- 也就是说，初始化阶段大对象流已经能看到明确顺序：
  - `上海证券代码表`
  - `深圳证券代码表`
  - 后续再接别的对象

## 2026-03-30 最新进展：外层对象顺序 catalog 已落地，深圳代码表也已完整

- 新增：
  - `./tmp_netzip_probe_20260329/catalog_large_outer_objects.py`
- 这轮先把 `frida_ws2_trace_code_table_full.js` 提升到 `180` 个大块，生成：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_code_table_full_180.log`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_code_table_full_180_chunks`
- 随后用新的 catalog 脚本按“外层 `declA` 非重叠顺序”列出了初始化阶段的大对象序列：
  - `object#1 = 上海证券代码表`
  - `object#2 = 深圳证券代码表`
  - `object#3 = 除权数据`
- 这一轮还有一个关键修正：
  - 当 `recv_large_*.bin` 超过 `99` 个后，不能再按文件名字典序拼流
  - 已把 `split_large_record_array.py` 和 `catalog_large_outer_objects.py` 都改成按文件编号做数字排序
- 深圳代码表这次也已经完整落地：
  - 切片目录：
    - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_object2_sz_code_table`
  - 解析目录：
    - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_object2_sz_code_table/record_array_split`
  - 记录数：`3366`
  - 首条：`SZ000001 / 平安银行`
  - 末条：`SZ399413 / 国证转债`
- 当前初始化阶段的结构图已经更完整了：
  - 先完整下发上海代码表
  - 再完整下发深圳代码表
  - 再进入大体量的 `除权数据`

## 2026-03-30 继续推进：`除权数据` 已能按分组结构解析前缀

- 没再继续暴力抓满第三个对象，而是先对照 `OemStock.h` 做结构对账。
- 关键对账结果：
  - `inner_type = 除权`
  - `count = 59600`
  - `len = 11920000`
  - `len / count = 200`
  - 与 `OEM_SPLIT_HEAD` / `OEM_SPLIT` 的 `200` 字节尺寸完全一致
- 基于这个结论，新增了解析器：
  - `./tmp_netzip_probe_20260329/parse_oem_split_prefix.py`
- 当前前缀切片：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_object3_split_prefix/object3_outer_prefix.bin`
- 新摘要输出：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_object3_split_prefix/object3_outer_prefix_parsed_split_prefix/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_object3_split_prefix/object3_outer_prefix_parsed_split_prefix/groups.csv`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v14_object3_split_prefix/object3_outer_prefix_parsed_split_prefix/splits.csv`
- 当前已经稳定跑出的前缀结构：
  - `groups_parsed = 92`
  - `records_consumed = 1293`
  - 模式明确是：
    - `OEM_SPLIT_HEAD(label,name,num)`
    - 后接 `num` 条 `OEM_SPLIT`
- 已直接解出的样本组包括：
  - `SH510050 / 50ETF / 18 条`
  - `SH510100 / SZ50ETF / 3 条`
  - `SH510180 / 180ETF / 15 条`
  - `SH600000 / 浦发银行 / 26 条`
  - `SH600028 / 中国石化 / 49 条`
- 这说明第三个大对象的正文组织方式已经基本跑实，不再只是“看见一个巨大除权对象头”

## 2026-03-30 继续推进：`除权数据` 已完整抓满并全量导出

- 这轮没有继续沿用 `v14` 的前缀思路，而是直接为第三对象做了定向长采样：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_split_full.js`
  - `maxLargeRecvDumps = 1500`
- 新日志与产物：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v15_split_full_1500.log`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v15_split_full_1500_chunks`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v15_split_full_1500_chunks/outer_object_catalog/object_bins/object_03.bin`
- `catalog_large_outer_objects.py` 这轮也补了一个很实用的小能力：
  - `--write-object-bins`
  - 可以把每个外层对象的已捕获字节直接切成 `object_XX.bin`
- 第三对象完整解析后的最终结果：
  - `groups_parsed = 5224`
  - `declared_splits_total = 54376`
  - `captured_splits_total = 54376`
  - `records_consumed = 59600`
  - 首组 `SH510050 / 50ETF`
  - 末组 `SZ302132 / 中航成飞`

## 2026-03-30 继续推进：初始化大对象链已基本跑通到 `实时数据`

- 这轮又单独做了一版更长的初始化采样器：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_init_full.js`
  - `maxLargeRecvDumps = 2000`
- 新日志与目录：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_init_full_2000.log`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_init_full_2000_chunks`
- 一个很关键的现场信号是：
  - 虽然上限写了 `2000`
  - 但最终只抽到了 `1929` 块
  - 这说明初始化数据流本身已经自然跑完，不是 Frida 先截断
- 当前已经完整抓到的主干对象：
  - `代码表 / 上海证券代码表`
  - `代码表 / 深圳证券代码表`
  - `除权 / 除权数据`
  - `财务 / 财务数据`
  - `实时数据 / 实时数据`
- 另外还确认了一条文件类对象：
  - `object#5` 完整
  - 能直接搜到 UTF-16 `文件`
  - 以及 UTF-16 `数据\\财务V6.fin`

## 2026-03-30 `财务数据` 已完整导出

- 新解析器：
  - `./tmp_netzip_probe_20260329/parse_oem_finance_records.py`
- 完整输出：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_object4_finance_full/parsed_finance/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_object4_finance_full/parsed_finance/finance.csv`
- 当前结果：
  - `captured_records = 5993`
  - `captured_complete = true`
  - 首条 `SH000001 / 上证指数`
  - 末条 `SZ399108 / 深证Ｂ指`

## 2026-03-30 `实时数据` 已完整导出，并确认存在双布局

- 新解析器：
  - `./tmp_netzip_probe_20260329/parse_oem_realtime_records.py`
- 先按固定 `OEM_REPORT` 解析时，前面大量记录正常，但尾部出现了明显的 1 字节错位乱码。
- 进一步对 payload 全量扫描后，规律已经坐实：
  - 前半段证券代码命中在 `offset % 500 = 0`
  - 从 `record#4920 = SZ300056` 开始，后半段代码命中切到 `offset % 500 = 1`
- 因此这次把解析器改成了“每条记录自适应 `shift=0/1`”。
- 自适应后的完整输出：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_object9_realtime_full/parsed_realtime_adaptive/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_object9_realtime_full/parsed_realtime_adaptive/quotes.csv`
- 当前已确认：
  - `captured_records = 6298`
  - `shift0_records = 4919`
  - `shift1_records = 1379`
  - 末条已恢复为 `SZ399413 / 国证转债`

## 2026-03-30 `文件` 对象与 catalog 分类已补齐

- 已更新：
  - `./tmp_netzip_probe_20260329/catalog_large_outer_objects.py`
- 现在 catalog 不再只认 `OEM_DATA_HEAD @ +264`，而是：
  - 先尝试主数据对象的 `oem264`
  - 再尝试文件对象的 `oem132`
  - 最后把剩余小包分类成 `control_packet`
- 重跑后的 `v16` catalog 已明确区分：
  - `object#1/#2/#3/#4/#9/#10 = main_data`
  - `object#5 = file_object`
  - `object#6/#7/#8/#11/#12/#13/#14/#15 = control_packet`
- `object#5` 这次已不再停留在“像文件对象”，而是已经解析成：
  - `head_variant = oem132`
  - `inner_type = 文件`
  - `inner_label = 数据\\财务V6.fin`
  - `inner_len = 994846`
  - `payload_offset = 332`
- 已新增专用解析器：
  - `./tmp_netzip_probe_20260329/parse_oem_file_object.py`
- 完整输出：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_object5_file_finance_v6/parsed_file_object/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_object5_file_finance_v6/parsed_file_object/records.csv`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_object5_file_finance_v6/parsed_file_object/payload.bin`
- 当前已经跑实：
  - `数据\\财务V6.fin` 的 payload 先有 `8` 字节前缀
  - 后面是 `5993` 条 `166` 字节记录
  - 代码索引首尾为 `SH000001 -> SZ399108`
  - 与 `财务数据 / OEM_FINANCE` 的 `5993` 条首尾完全对上
- 这个 `.fin` 记录阵列本身也存在布局相位切换：
  - `shift0_records = 2658`
  - `shift1_records = 3335`
  - `first_shift1_record = 2659 SH688656`

## 2026-03-30 `财务V6.fin` 已推进到字段级映射，不再只停在代码索引

- 已新增：
  - `./tmp_netzip_probe_20260329/map_finance_v6_to_oem_finance.py`
- 输出：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_finance_v6_mapping/summary.txt`
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_finance_v6_mapping/mapping.csv`
- 这轮的验证方式已经升级成：
  - 直接逐条比 `object_04.bin` 的 `OEM_FINANCE`
  - 对 `object_05.bin` 的 `.fin` 记录做 `shift` 归一
  - 再扫描可全量精确命中的字段偏移
- 当前已经坐实：
  - `code_mismatch_count = 0`
  - `mapped_int_fields = 0`
  - `mapped_float_fields = 12`
- 已确认的 `.fin` 精确 `float32` 对位包括：
  - `zongGu @ 18`
  - `bGu @ 34`
  - `jingwaiGu @ 38`
  - `liuTongAG @ 42`
  - `zongZC @ 54`
  - `guDingZC @ 62`
  - `wuXingZC @ 66`
  - `zbGongJi @ 82`
  - `mggjj @ 86`
  - `quanYi @ 90`
  - `shouRu @ 94`
  - `zyLiRun @ 98`
- 已确认未直接命中的字段：
  - `time / baoGao / shangShi`
  - 以及另外 `36` 个 `OEM_FINANCE float` 字段
- 当前结论已经从“`.fin` 可能是紧凑副本”推进到：
  - `.fin` 确实内嵌了部分 `OEM_FINANCE` 原始 `float32`
  - 但不是完整逐字段平铺，剩余字段要么被省略，要么用了另一种编码/缩放

## 2026-03-30 `财务V6.fin` 第二轮字段搜索已完成

- 已把 `./tmp_netzip_probe_20260329/map_finance_v6_to_oem_finance.py` 扩成变换搜索器：
  - 支持 `float32`
  - 支持 `int32/uint32 + 常见缩放因子`
  - 支持输出 `partial_candidates` 与 `boundary_anomalies`
- 重跑后关键结果仍在：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_finance_v6_mapping/summary.txt`
  - `./tmp_netzip_probe_20260330_v16_finance_v6_mapping/mapping.csv`
- 当前新增的有效结论：
  - `6` 个字段虽然没有被列入 full match，但都达到了 `5992 / 5993`
  - 它们全部只在 `row=2658 / SH688655` 这一条记录上失配
  - 其余 `5992` 条上与 `OEM_FINANCE` 完全一致
- 已单独验证 `SH688655`：
  - 这条记录对代码字段仍像 `shift=0`
  - 但对后段财务字段，`+1` 视图能恢复 `mgJingZhi/mgwfp/gdqybl/zongLiRun/jingLiRun/weiFenPei`
  - 因此它更像布局切换边界上的混合记录，而不是普通坏数据
- 另一个保留结果是：
  - `xianJin` 首次出现了 `uint32 * 0.0001` 的部分匹配候选
  - 但当前只覆盖 `726` 条，还不能当成正式映射
- 同时已对 `time / baoGao / shangShi` 做了第一轮日期编码排除：
  - 未发现直接对应年月日分量的 `u8/u16` 偏移
  - 未发现常见“距 1970/1900/1990 的天数”序列偏移
  - 下一步不应再重复低价值的直接日期扫描，而应转向 bit-packed 或跨字段共享编码

## 2026-03-30 `SH688655` 边界记录已补专项分析器

- 已新增：
  - `./tmp_netzip_probe_20260329/analyze_fin_v6_boundary_record.py`
- 新输出：
  - `./tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v16_boundary_record_2658/summary.txt`
- 当前这条记录的稳定结论：
  - 对 `18` 个已知映射字段做分段 `shift0 -> shift1` 评分时
  - `best_split_range = 99..126`
  - 也就是：前段字段继续按标准布局取值，后段字段切到 `+1` 视图，已经足以把全部已知字段恢复
- 这一步还没把分界点压到单字节，但已经把“混合记录”从口头结论变成了可复跑分析结果。
- 窗口内目前最可疑的单字节是：
  - `offset=106`
  - `SH688655` 为 `0x3d`
  - 前后邻居该位都为 `0x00`
  - 现在已经做了直接验证：删掉 `offset=106` 这个单字节后，再按固定偏移重读，`18` 个已知映射字段全部恢复
  - 因而 `106` 已升级成当前最强的局部插入点候选

## 2026-03-30 公式型字段已补入映射结论

- 已把 `./tmp_netzip_probe_20260329/map_finance_v6_to_oem_finance.py` 扩成同时输出“偏移映射”和“派生关系”。
- 当前新增稳定结论：
  - `mgXianJin = xianJin / zongGu / 10`，`5993 / 5993` 全量成立
  - `xianJin = mgXianJin * zongGu * 10`，`5993 / 5993` 全量成立
- 这说明 `mgXianJin/xianJin` 不该再继续按“缺少直存偏移”理解，而应作为公式型字段处理。
- `mgShouYi` 的第一轮公式尝试：
  - `round(jingLiRun / zongGu / 10, 2)`
  - 当前只命中 `4662 / 5993`
  - 还不足以下最终结论
- 同时已把 `mgShouYi` 的常见“利润字段 / 股本口径”组合扫过一轮：
  - 最好仍然是 `jingLiRun / zongGu`
  - `jingLiRun / liuTongAG` 只有 `3179 / 5993`
  - `zongLiRun / zongGu` 只有 `1305 / 5993`
  - 说明 `mgShouYi` 大概率需要当前表里未直接暴露的加权/摊薄口径

## 2026-03-30 `baoGao` 已确认是时间戳转换字段

- 这轮把 `.fin` 记录头段重新扫了一遍，确认：
  - `offset 14..17` 的 `u32` 本地午夜时间戳
  - 经过 `timestamp -> 本地日期 -> YYYYMMDD` 转换后
  - 与 `OEM_FINANCE.baoGao` `5993 / 5993` 全量一致
- 当前主映射器里已经把这条关系单独列成：
  - `baoGao <= ymd(from_local_timestamp(file_offset_14_u32))`
- `time / shangShi` 暂时还没有找到同类时间戳位点：
  - `time` 无有效命中
  - `shangShi` 只有 `27` 条零值对零值命中，属于噪声

## 2026-03-30 `财务V8.fin` 已完成结构确认

- 已新增：
  - `./tmp_netzip_probe_20260329/parse_finance_v8_file.py`
- 新输出：
  - `./tmp_netzip_probe_20260329/finance_v8_disk_parsed/summary.txt`
  - `./tmp_netzip_probe_20260329/finance_v8_disk_parsed/records.csv`
- 当前已经确认：
  - `财务V8.fin = 8 + 5993 * 224`
  - 记录头就是 `code + reserved + time + baoGao + shangShi`
  - 后面直接跟 `48` 个 `float32`
  - 尾部两个固定 `float32 = 1.0`
- 与 `object_04.bin / OEM_FINANCE` 做了全量逐位对位后：
  - `code_ok = 5993`
  - `int_ok = 5993`
  - `float_ok = 5993`
  - 说明 `V8` 实际上就是 `OEM_FINANCE` 的紧凑磁盘版

## 2026-03-30 `V6` 设计轮廓已落成可读摘要

- 已新增：
  - `./tmp_netzip_probe_20260329/map_finance_v6_to_v8_bytes.py`
  - `./tmp_netzip_probe_20260329/summarize_finance_v6_design.py`
- 新输出：
  - `./tmp_netzip_probe_20260329/finance_v6_to_v8_byte_map/summary.txt`
  - `./tmp_netzip_probe_20260329/finance_v6_design_summary/summary.txt`
- 当前已能明确说清：
  - `V6` 不是“缩小版 V8 整体平移”
  - 而是“选定字段直接裁剪 + 若干字段公式化/时间戳化 + 一些字段直接缺失”
- 这让剩余任务进一步收敛成：
  - `time/shangShi` 在 `V6` 中的处理方式
  - `mgShouYi_display` 的显示口径
- 进一步把“非零且有信息量”的字段筛掉后，当前真正还未解释掉的重点只剩：
  - `time`
  - `shangShi`
  - `tzShouYi`
  - `mgShouYi_display`

## 2026-03-30 继续推进：已把 Windows 现场成果重新整理成 Rust 行情软件交付路线

- 这轮不是新增抓包，而是把已有 `windows_debug` 结论重新按“哪些能直接用来完成 Rust 行情软件”做了一次整理。
- 当前已经明确：
  - Windows 侧成果里，最可直接复用的是：
    - `x86 Stock.dll + 网际风.exe + 127.0.0.1:2000` 这条可运行主链
    - `Ask(...)` 的三类返回模型
    - 初始化大对象链 `代码表 -> 除权 -> 财务 -> 文件 -> 实时数据`
    - `财务V8.fin = OEM_FINANCE`、`财务V6.fin` 的旧格式关系
  - 这些项足以支撑先完成“Windows-first Rust 行情软件”：
    - 稳定 DLL 封装
    - 统一事件流
    - 初始化缓存与回放
    - 一致性校验
- 这轮也把“暂不阻塞 MVP”的项明确分离出来：
  - `hypenc` 精确定义
  - 本地第二个/后续 `penc` 的 payload 内语义
  - `7100` 登录后 `Tdx_Encrypt / ZSTD字典` 的完整可逆链
- 结论上，当前实现顺序应继续按：
  - 先完成 Windows DLL 路线的 Rust 行情客户端
  - 再补初始化缓存落盘和对账
  - 最后才继续推进“无 DLL 的纯 Rust 上游替代”
- 这轮整理结果已同步回：
  - `README.md`
  - `windows_debug/findings.md`
  - `windows_debug/task_plan.md`
- 注：
  - 上面这段是当时的阶段性判断
  - 从同日后续推进开始，仓库主实施路线已经切到 `pure Rust + Linux / Linux-first`
  - Windows 主链当前只保留为对账锚点，不再作为最高优先级交付路径

## 2026-03-30 继续推进：纯 Rust + Linux 优先级已正式回写，并新增 Linux-only 聚合探针

- 这轮按“必须优先纯 Rust + Linux”重排了主线，不再让 Windows-first 路线占据实施优先级。
- `netzip_service` 已新增：
  - `POST /api/linux/pure-rust-mvp`
- 这个新入口不会碰 DLL，也不会走本地 `2000`：
  - 先跑 `7709` 代码表同步
  - 再跑 `0547` 实时行情
  - 再跑在线 `K线`
  - 最后可选跑 `F10` 栏目
- 当前实现目标不是“协议研究摘要”，而是把已经落地的 Linux-native 能力串成一条可直接验收的业务链。
- 这轮还顺手做了两件工程收口：
  - 把 `7709 sync/live-quote/kline/f10-categories` 的服务端执行逻辑抽成了可复用 helper，避免后面 Linux-only 入口继续复制粘贴
  - 给新入口和当前“pure Rust + Linux 最高优先级”声明各补了 1 条单测
- 当前任务排序也已调整：
  - 第一优先级：Linux-only 客户端入口与缓存
  - 第二优先级：Linux-only 一致性回归
  - Windows 主链降级为对账锚点，不再作为主实施路线

## 2026-03-30 继续推进：Linux-first 纯 Rust 统一入口已落地

- 为了不再让 Linux 路线停留在“若干 example + 若干 HTTP 调试接口”的碎片状态，这轮新增了统一入口：
  - `src/bin/netzip_linux.rs`
- 当前已经落成的命令：
  - `snapshot <symbol>`
  - `sync-code-table`
- 设计目标不是伪装成“已经完整替代 Windows 主链”，而是先把当前纯 Rust + Linux 已可用的业务能力收成可直接调用的入口：
  - `snapshot` 组合跑 `实时行情 + K线 + F10分类`
  - `sync-code-table` 直接跑一次 `7709` 代码表同步并输出 JSON 摘要
- 这轮还补了一个重要的现实边界：
  - Linux 纯 Rust 路线当前并不是每个子调用都稳定成功
  - 所以 `snapshot` 现在按“部分成功也返回 JSON”设计
  - 会保留已成功部分和 `live_quote_error / kline_error / f10_categories_error`
  - 不再因为第一项失败就整条命令退出
- 已验证：
  - `cargo test --bin netzip_linux`
  - `cargo run --bin netzip_linux -- --help`
  - `cargo run --bin netzip_linux -- live-quote SH600000 SZ000001`
    - 返回了 `requested_symbols=[SH600000,SZ000001]`
    - `transport_symbols` 自动补到了 3 个标的
    - `matched_records=2`
  - `cargo run --bin netzip_linux -- kline SH600000 --count 3`
    - 返回了 `2026-03-26 .. 2026-03-30` 的 3 根日线
  - `cargo run --bin netzip_linux -- f10-categories SH600000 --limit 3`
    - 返回了 `最新提示 / 公司概况 / 财务分析`
  - `cargo run --bin netzip_linux -- f10-content SH600000 --category-name 公司概况 --preview-chars 300`
    - 成功返回 `600000.txt` 中 `公司概况` 正文预览
- 同一轮还把组合能力接进了 HTTP：
  - 新增 `POST /api/tdx7709/snapshot`
  - 行为与 CLI `snapshot` 一致，都会保留部分成功结果和分项错误
- 同时把非 Windows 下的默认提示也改了：
  - `src/main.rs` 现在会直接提示使用 `netzip_linux` 或 `netzip_service`
  - 不再只停在“这个 DLL 示例需要 Windows”

## 2026-03-30 继续推进：Linux-only 组合链已支持 phase 开关

- 这轮继续推进的重点不是再加新协议，而是把 Linux-only 组合入口做得更稳：
  - `POST /api/tdx7709/snapshot`
  - `POST /api/linux/pure-rust-mvp`
  - `cargo run --bin netzip_linux -- snapshot ...`
- 当前已经新增的控制项：
  - HTTP `snapshot`：`include_live_quote / include_kline / include_f10`
  - HTTP `linux/pure-rust-mvp`：`include_sync / include_live_quote / include_kline / include_f10`
  - CLI `netzip_linux snapshot`：`--skip-live-quote / --skip-kline / --skip-f10`
- 这轮的工程目标是：
  - 不再让某个慢阶段把整条 Linux-first 验证链拖死
  - 可以先跑 `实时行情 + K线`
  - 再按需打开 `F10` 或显式 `sync`
- 同时补了一条语义修正：
  - phase 被显式跳过时，响应会返回 `enabled=false, skipped=true`
  - `findings` 不再把“跳过阶段”误报成失败
  - `overall_ok` 只按本次启用的阶段计算
- 已验证：
  - `cargo test --bin netzip_linux`
  - `cargo test capabilities_include_linux_first_snapshot_and_mvp_endpoints --bin netzip_service`
  - `cargo test delivery_tracks_keep_pure_rust_linux_as_highest_priority --bin netzip_service`
  - `cargo test linux_phase_skipped_marks_phase_as_disabled --bin netzip_service`
  - `cargo build --bin netzip_service --bin netzip_linux`
  - `POST /api/tdx7709/snapshot` 对 `SH600000`、`include_f10=false` 已返回成功
  - `POST /api/linux/pure-rust-mvp` 对 `SH600000`、`include_sync=false`、`include_f10=false` 已返回 `overall_ok=true`

## 2026-03-30 `7709` 与 `rustHq` 对位结论已单独沉淀

- 按用户新确认的分工，这轮补做的不是新的抓包，而是把“当前现场 `7709` 到底能否按 `rustHq` 当作同类通达信服务器处理”写成面向 Linux 开发侧的结论文件。
- 已新增：
  - `./tdx7709_vs_rusthq_20260330.md`
- 当前落地结论是：
  - 当前现场 `120.195.71.160:7709` 与 `../../_third_party/rustHq` 面向的是同一种通达信 `7709` 服务器体系
  - `K线 / 代码表 / F10 / 财务 / 分钟分时` 可以直接按 `rustHq` 思路理解和复用
  - `实时行情` 需要保留当前仓库的 `0x0547` 专用解析链，不能简单降回 `rustHq` 当前公开的 `0x053e`
- 这条结论也已经同步回：
  - `./findings.md`

## 2026-03-30 `pytdx` 端口角色也已补成单独对照表

- 这轮继续补的是 `pytdx` 里 `7709` 之外端口的职责划分，避免 Linux 侧把：
  - `7709`
  - `7711`
  - `7721`
  - `7727`
  - `80 / 443`
  混成一类。
- 已新增：
  - `./pytdx_ports_and_roles_20260330.md`
- 当前整理后的最短结论：
  - `7709`：普通股票行情主端口
  - `7711`：普通股票行情同类备用端口
  - `80 / 443`：普通股票行情兼容接入端口
  - `7721 / 7727`：扩展市场行情端口
  - 交易服务器：另一条交易链，不属于当前行情主线
- 这条结论也已经同步回：
  - `./findings.md`

## 2026-03-30 继续推进：Linux-first 组合链已补共享会话与自动回退

- 这轮继续推进的重点不是新协议，而是把当前纯 Rust + Linux 主线的网络往返压下来，同时保住原来的鲁棒性。
- 当前已经落地：
  - `src/tdx7709.rs` 新增 `Tdx7709Session`
  - `POST /api/tdx7709/snapshot`
  - `POST /api/linux/pure-rust-mvp`
  - `cargo run --bin netzip_linux -- snapshot ...`
- 行为变化：
  - 组合链现在会优先复用单个 `7709` 会话
  - 避免在同一条 `snapshot/mvp` 请求里重复做 `bootstrap/代码表同步`
  - 如果共享会话在中途失败，会自动回退到独立请求模式，不让整条链被一次连接状态拖死
- 这轮还补了一条重要语义澄清：
  - `include_sync=false` 只表示不单独报告 sync phase
  - 只要后续 `live-quote/kline/f10` 仍启用，transport 层依然会做内部 `bootstrap/代码表准备`
- 响应里新增了：
  - `shared_session_attempted`
  - `shared_session_established`
  - `shared_session_fallback_phases`
- 同步修正：
  - `README.md` 已改成 Linux-first 为推荐主线
  - `windows_debug/task_plan.md` 的 Delivery Tracks 已重排
  - phase 编号重复的 `38` 已拆开，并新增共享会话阶段记录
- 实际回归结果：
  - `POST /api/tdx7709/snapshot` 对 `SH600000`、`include_f10=false` 返回了 `shared_session_attempted=true`、`shared_session_established=true`、`shared_session_fallback_phases=[]`
  - `POST /api/linux/pure-rust-mvp` 对 `SH600000`、`include_sync=false`、`include_f10=false` 同样返回 `shared_session_fallback_phases=[]`，并保持 `overall_ok=true`
  - `cargo run --bin netzip_linux -- snapshot SH600000 --kline-type 1d --kline-count 3 --skip-f10` 这次命中了 `shared_session_fallback_phases=[\"live-quote\"]`，说明 CLI 侧的自动回退和元数据暴露都已经生效

## 2026-03-30 继续推进：`netzip_linux` 已补 Linux-only 直接命令

- 这轮继续推进的重点不是再加 HTTP 路由，而是把纯 Rust + Linux 的命令行入口补成更像可用客户端，而不是只剩一个 `snapshot` 探针。
- 当前已经新增：
  - `cargo run --bin netzip_linux -- live-quote ...`
  - `cargo run --bin netzip_linux -- kline ...`
  - `cargo run --bin netzip_linux -- f10-categories ...`
  - `cargo run --bin netzip_linux -- f10-content ...`
- 行为变化：
  - `live-quote` 可以直接拉多标的 `0547` 行情，并返回 `requested_symbols / transport_symbols / unmatched_symbols`
  - `kline` 可以直接按 `kline-type / start / count` 拉在线 `K线`
  - `f10-categories` 可以直接列栏目清单
  - `f10-content` 可以按 `category-name` 或 `filename + start + length` 拉正文
  - 当 `f10-content` 先按 `category-name` 定位时，CLI 会优先复用同一条 `7709` session 做“先查栏目再取正文”
- 这轮的工程意义：
  - Linux 下即使不启动 `netzip_service`，也已经有一套更完整的纯 Rust 命令行入口
  - 当前纯 Rust + Linux 主线不再只依赖 example 或调试接口拼装
- 已验证：
  - `cargo test --bin netzip_linux`
  - `cargo run --bin netzip_linux -- --help`

## 2026-09-02 5188 Phase S 证据与权威 crate 合并进度

- 当前协议实现的权威位置保持为：`/home/codes/stock/crates/netzip-fullpull`。
- 参考工程 `netzip_win` 的已验证协议能力按项吸收，平台生命周期和 Wine ABI 不与共享协议层混放。
- 本轮已确认并完成回归：
  - ABK `3610` 接受观测到的 `94B`、`106B` 变体；
  - ACK 客户端 `3610` 保持动态 `40..=80B` 门槛；
  - ACK 服务端 `3210` 接受 `466/467/468B`；
  - `Official5188SubscriptionEnvelope::declared_entry_count()` 支持 `6154B -> 1024`、`358B -> 58`；
  - 默认 ACK manifest 包含九行结构性市场状态，相关回归已进入共享 crate；
  - 最新共享 crate 验证为 `128 passed, 1 ignored`，格式化、Clippy、i686 release 构建均通过。
- `formal-primary-0006` 已观察到完整顺序：`3210 -> 2d10 x3 -> 2a10 -> 2704`；
  - `3210` 最新长度扩展到 `468B`，已纳入解析门槛；
  - `2a10` 已知 `358B/6154B` 形状及声明条目数，但六字节条目语义仍未定；
  - `2d10` 仍仅确认“4 个 little-endian u32 + 16 字节零尾”，四词的动态来源未证明。
- 生产边界：
  - native full-push 服务仍在线，但主发布 worker 当前继续使用 `Tdx7709Session`；
  - 5188 仅可作为 shadow/取证链路，不能发送静态抓包的 `2d10/2a10`；
  - 不能把 7709 的数据量或成功状态宣称为 5188 业务字段 parity。
- 下一接力点：
  1. 为 5188 建立 shadow reader，持续记录原始帧、解码指标、`2704` 计数、断线与重连；
  2. 用第二个独立且同认证生命周期的 formal capture 对齐 `3210`、第二个 `3110`、连接角色与 `2d10` 三帧；
  3. 解析 `2704` 到公共行情和 Wine `OEM_REPORT` 的字段、批次、去重及时序 parity；
  4. parity、freshness、重连恢复全部通过后，再评估将生产数据源从 7709 切换到 5188。

## 2026-09-02 协作检查点

- 协作入口确认：当前仅维护本文件作为 `progress.MD` 进度汇总；协议代码仍以 `/home/codes/stock/crates/netzip-fullpull` 为权威。
- 已确认的最新证据可继续复用：`formal-primary-0006` 的 `3210 -> 2d10 x3 -> 2a10 -> 2704` 顺序，以及 `3210` 的 `468B` 长度变体。
- 并行工作提交新结果时必须同时写明：抓包路径、会话边界、帧方向、长度/计数、测试命令和是否可推广到运行时；仅有静态抓包值不得解除初始化门禁。
- 当前优先执行项保持不变：
  - 增加/验证 5188 shadow reader 的持续接收指标；
  - 获取第二个独立同生命周期 formal capture，分析 `2d10` 动态字段来源；
  - 对 `2704` 公共字段与 Wine `OEM_REPORT` 做同账号同窗口 parity；
  - 所有结果先写入本文件，再提交 webClx 编译/部署请求。

## 2026-09-02 现状复核（formal-primary-0006）

- 证据文件最后更新时间：`primary-5188-summary.json` 01:19 前后；当前未发现更新的独立会话抓包。
- 共享解析器当前仍保留：`3210` 的 `466/467/468B` 验收、`2d10` 四词加零尾结构校验，以及 `2a10` 声明条目数解析。
- 服务代码复核确认：`official_5188_status` 仍报告 `pending-production-wiring`，full-push session cache 和发布路径仍由 `Tdx7709Session` 驱动。
- 因此本轮没有把“已观察到完整链路”升级为“已完成动态复刻”；下一次并行结果必须优先提供第二个独立会话或字段级 Wine/Rust parity 证据，才能推进生产切换评估。

## 2026-09-02 并行证据复核（二）

- 再次检查 `diagnostics/20260902-live-pair/formal-primary-0006`，最新文件仍为 `primary-5188-summary.json`（约 00:37），未出现第二个独立会话样本。
- 由于权威 crate 位于 quoteNetzipRs 工作树之外，本仓库状态检查不代替共享 crate 的独立审计；共享 crate 的测试/部署结果必须以 webClx 回调和安装审计为准。
- 当前协作结论未变：可以继续做离线帧关联、shadow reader 和字段 parity，但不得凭现有单会话静态值解除 `InitializationUnavailable` 或切换生产 5188 数据源。

## 2026-09-02 formal-primary-0006 量化复核

- 直接读取 `primary-5188-summary.json` 得到：38 条 TCP flow、680 个已重组协议帧。
- 其中包含：`3610` 30 帧、`3110` 20 帧、`3210` 10 帧、`2d10` 30 帧、`2a10` 7 帧、`2704` 15 帧、`0710` 1 帧。
- 解析器还识别出 230 个 zlib 对象和 15 个 delta（`2704`）包；`2a10` 合计 6202 个六字节条目。
- 这些统计证明现有抓包覆盖足以支持顺序/长度/方向回归，但仍不能证明 `2d10` 四词或 `2a10` 六字节条目的动态生成规则；该限制继续记录为 `needs-verification`。

## 2026-09-02 协作持续检查

- 本次读取确认 `primary-5188-summary.json` 自 00:37 后没有新修改，进度文件本身在 10:32 更新。
- 未检测到新的独立会话或 parity 报告，因此不提升任何协议结论等级。
- 并行终端下一次提交应优先追加新证据文件或测试结果；仅“继续/已完成”类状态消息不计入协议进度。

## 2026-09-02 5188 shadow 指标实现开始

- 已在 `src/official_5188_runtime.rs` 为有界 `Official5188FrameSink` 增加只读 `Official5188FrameSinkSnapshot`：
  - `frame_count`、`byte_count`、`dropped_frames`；
  - 按抓包字节序表示的 `wire_kind_counts`；
  - 最近帧的 wire kind 和 payload 长度；
  - 不暴露原始 payload、认证字段或凭据。
- 已补回归测试：当 sink 淘汰旧帧后，快照只统计当前保留证据，并正确报告两个 `3210`、丢弃数及最近帧长度。
- 已从 `src/lib.rs` 导出快照类型，供后续 service status 使用。
- `git diff --check` 对本轮文件通过；全仓 `cargo fmt --check` 当前仅被另一并行修改中的 `examples/official_5188_extract.rs` 格式差异阻断，本轮 runtime 文件已按 rustfmt 输出修正。
- webClx 验证请求：`103736-18d1330bb455c58a`，命令为 `cargo test official_5188_runtime --lib && cargo build --release`；等待匹配回调，尚未宣称验证完成。
- 并行生命周期审计结论：下一步应实现只读 `Official5188ShadowReader`，消费 pending 5188 session 并只调用 receive；必须提供显式 stop/join、登录重置清理、零客户端写入测试，禁止自动重连、静态初始化帧注入或调用 7709/gateway 发布函数。

## 2026-09-02 5188 shadow reader 核心实现

- `103736-18d1330bb455c58a` 已完成：5 个 `official_5188_runtime` 测试通过，随后 release 构建成功；日志 `/home/bin/webclx/logs/quoteNetzipRs/3123_build.log`。
- 已新增 receive-only `Official5188ShadowReader`：
  - 消费一个现有 `Official5188Session`，后台只调用 `receive_until_disconnect`；
  - 使用克隆 socket 作为 stop handle，显式 `shutdown(Both)` 后 join；Drop 兜底停止；
  - 记录总帧数、application bytes（metadata + payload）、`2704`、`3e04`、终止次数和非主动停止错误；
  - 原始帧只进入有界 sink，HTTP 尚未暴露 payload；
  - 不发送 `3610/2d10/2a10/0710`，不自动重连，不调用 7709 或 quoteGateway 发布路径。
- 新增本地 socket 回归：
  - 分片写入 `2704 + 3e04` 后只统计两个完整帧，并核对 wire kind；
  - 服务端读超时验证 shadow reader 未向 socket 写入任何字节；
  - `stop()` 可解除阻塞并 join，主动停止不记录伪造的 peer 错误。
- 下一步在核心 reader 通过 webClx 定向验证后，才接入 service state/status/start/stop 生命周期；不会直接替换生产 7709 publisher。

## 2026-09-02 5188 shadow service 接线

- `104338-18d1330bb455c58b` 已完成：7 个 runtime 测试通过（含分片收帧、零客户端写入、stop/join），release 构建成功；日志 `/home/bin/webclx/logs/quoteNetzipRs/3124_build.log`。
- `AuthRuntime` 已加入独立 `official_5188_shadow` ownership，不与 pending socket 或 7709 session cache 混用。
- 新增研究端点：
  - `POST /api/fullpull/official-5188/shadow/start`：只消费已有 pending 5188 session，启动 receive-only reader；
  - `POST /api/fullpull/official-5188/shadow/stop`：锁外 shutdown/join；
  - 两者只列入 `research_endpoints`，明确不列入 public/stable endpoints。
- `/api/fullpull/official-5188/status` 已增加聚合 shadow 快照，并精确区分：pending、connected-awaiting-initialization、shadow-receiving-opaque-frames、shadow-stopped。
- `auth_login` 和 disconnect 清理均先 take reader，再在锁外 stop/join；connect 会拒绝与已有 reader 并存。
- 仍保持的门禁：不暴露 raw payload，不自动重连，不初始化/订阅，不发布行情，不影响 `/health`，不替换 `Tdx7709Session`。
- 新增 service 纯函数回归：四种 lane 分类，以及 shadow start/stop 仅属于 research endpoint。
- 本轮文件通过 rustfmt 和 `git diff --check`；全仓 fmt 仍仅剩并行文件 `examples/official_5188_extract.rs` 的既有格式差异。

## 2026-09-02 shadow 状态语义收紧

- `105245-18d1330bb455c58c` 已完成：service lane 定向测试 1/1、runtime 7/7 通过，release 构建成功；日志 `/home/bin/webclx/logs/quoteNetzipRs/3125_build.log`。
- 状态命名已收紧：reader 启动但尚无完整帧时报告 `shadow-observing-uninitialized-socket`；仅在 `frames_received > 0` 后报告 `shadow-receiving-opaque-frames`，避免把裸连接误称为业务接收。
- 新增状态 JSON 回归：允许暴露 payload 长度指标，但明确不包含 raw payload、payload hex、密码、credential 或 raw hex。
- 四态测试现在覆盖 pending、connected、observing、receiving 和自然停止状态；生产边界未改变。

## 2026-09-02 shadow 状态验证进行中

- webClx 请求 `105524-18d1330bb455c58d` 的定向测试已完成：service 2/2、runtime 7/7 通过。
- 已验证：未收帧和已收帧的 lane 区分，以及状态 JSON 不包含原始 payload/hex/密码/credential。
- 同请求的 release 构建仍有活动 worker，等待匹配完成回调；此处只登记测试通过，不提前登记完整构建成功。
- 匹配回调成功后下一步为 workspace tests + Clippy 全面回归，不继续扩大未经验证的 5188 初始化行为。

## 2026-09-02 shadow 状态验证完成

- `105524-18d1330bb455c58d` 匹配回调成功，日志 `/home/bin/webclx/logs/quoteNetzipRs/3126_build.log`。
- 最终结果：service 2/2、runtime 7/7 通过，release build 成功。
- 本轮 shadow 核心与 service 接线已达到提交全面 workspace 回归的条件；仍未部署，也未改变运行中服务。

## 2026-09-02 workspace 综合回归运行中

- 请求 `105711-18d1330bb455c58e` 已确认有活动 systemd worker，正在执行 `cargo test --workspace --release && cargo clippy --workspace --all-targets -- -D warnings && cargo build --release`。
- 当前日志已推进到 workspace 148 个构建目标的 146/148，尚未出现失败；等待最终测试、Clippy 和 release 构建结果。
- 构建期间不修改 Rust 输入、不重复提交请求；收到匹配回调后再根据首个失败点修复或进入部署前审计。

## 2026-09-02 workspace Clippy 阻塞修复

- `105711-18d1330bb455c58e` 在 workspace release tests 后进入 Clippy，失败于三个并行新增 example：
  - `official_6100_decode.rs` 两处固定 `chunks_exact(2)`；
  - `official_5188_extract.rs` 和 `official_5188_callback_parity.rs` 各一处复杂 HashMap 返回类型。
- 已按 lint 的结构性建议修复：UTF-16 遍历改为 `as_chunks::<2>()`；两张证券索引映射增加领域类型别名。未增加全局/局部 lint allow。
- 三个 example 已 rustfmt；当前 `cargo fmt --all -- --check` 和相关文件 `git diff --check` 均通过。
- 下一步重跑与 `105711` 完全相同的 workspace release tests、全 targets Clippy、release build。

## 2026-09-02 UTF-16 as_chunks 类型修复

- `110030-18d1330bb455c58f` 在编译 `official_6100_decode` 时立即失败：`as_chunks::<2>().0.iter()` 的元素类型为 `&[u8; 2]`，终止符比较仍按值数组书写。
- 已将比较修正为 `*word == [0, 0]`；未改变奇数字节尾部忽略语义或 UTF-16 解码逻辑。
- `cargo fmt --all -- --check` 与相关 `git diff --check` 通过。
- 先通过 webClx 定向 check 该 example 和三个相关 example 的 Clippy，再重跑昂贵的 workspace 综合回归。

## 2026-09-02 相关 examples 定向验证完成

- `110155-18d1330bb455c590` 成功，日志 `/home/bin/webclx/logs/quoteNetzipRs/3129_build.log`。
- `official_6100_decode` check 通过；`official_6100_decode`、`official_5188_extract`、`official_5188_callback_parity` 的 `-D warnings` Clippy 全部通过。
- 已排除前两轮综合回归暴露的 example 编译/lint 问题，恢复完整 workspace 综合回归。

## 2026-09-02 workspace 综合回归完成

- `110255-18d1330bb455c591` 成功，日志 `/home/bin/webclx/logs/quoteNetzipRs/3130_build.log`。
- `cargo test --workspace --release` 全绿：主库 96 passed / 2 ignored；service 67 passed / 4 ignored；其余 workspace crate、集成测试和 doc-tests 均无失败。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo build --release` 通过。
- 本轮确认 shadow reader/service 接线与并行新增 6100/5188 examples 可以共存；没有改变动态初始化门禁或 7709 生产发布路径。
- 当前状态：源码已验证但尚未部署；运行中的 `/home/bin/netzip/quoteNetzipRs` 仍为上一部署版本。下一步先核对部署差异和安装脚本，再通过 webClx 部署并检查审计、health、capabilities 与 shadow status。

## 2026-09-02 shadow 版本部署前审计

- 已核对 `scripts/install-service.sh`：安装目标 `/home/bin/netzip/quoteNetzipRs`，会重启 supplement/full-push 两个正式 unit，并检查 `127.0.0.1:16893/health`。
- 已验证构建 SHA-256 为 `7cba806264441a188b465c32e0d5430e9bec83fefdc5eb8202db4659e09bc366`；当前运行二进制为 `dea46b72071fefdf9cd348f0170311b0758de1bcdc6aedf1650f3a4b0be24693`，确认尚未安装新版本。
- 准备通过 webClx deploy API 执行项目安装脚本，并要求 `target/release/quoteNetzipRs` 产物与 `/home/bin/netzip/quoteNetzipRs` 安装审计；回调前不宣称部署完成。

## 2026-09-02 shadow 版本部署与运行时验证

- `110453-18d1330bb455c592` 部署成功；安装审计 `/home/bin/webclx/logs/quoteNetzipRs/3131_install-report.json`。
- `/home/bin/netzip/quoteNetzipRs` 从 `dea46b72...693` 更新为已验证的 `7cba806264441a188b465c32e0d5430e9bec83fefdc5eb8202db4659e09bc366`。
- `quote-netzip-rs-supplement.service`、`quote-netzip-rs-full-push.service` 均 active；`/health` 正常。
- `/api/capabilities` 已显示 shadow start/stop 仅在 research endpoints；public endpoints 未扩大。
- `/api/fullpull/official-5188/status` 当前为 unauthenticated、`pending-production-wiring`、`opaque-evidence-only`、shadow null，符合冷启动门禁。

## 2026-09-02 2a10 跨会话证据吸收

- 本地直接比较：
  - `diagnostics/20260902-live-pair/formal-primary-0006/primary-5188-summary.json`；
  - `diagnostics/20260901-live-pair/cold-sync-2311/primary-5188-summary-current.json`。
- 两个独立生命周期均为 38 flows、7 条 client `2a10`：六条声明 1024 entries，一条声明 58 entries；每条连接的 market、start_value、count 连续区间逐项完全一致，仅本地端口变化。
- 已确认五条主分区按代码表 symbol-index 空间连续铺贴，第六条为大量离散混合集合，第七条为 58 条短集合；`2704` 数量仍表示变化记录量，不等于订阅总量。
- 结论等级：`市场[2] + symbol_index(u32 LE)` 与分区形状升级为跨会话强候选；但尚无第三个交易时段/自选股变更实验，且 `2d10` 四词、动态 `3610`、ACK 来源未闭合，因此 `InitializationUnavailable` 继续保持。
- shadow 协作边界：receive-only reader 无法从 TCP read 看到客户端发送帧；订阅摘要必须由初始化编排器或抓包证据路径显式记录，不得假装 live reader 已被动观察到 `2a10`。

## 2026-09-02 2a10 脱敏摘要实现

- `Official5188FrameSinkSnapshot` 已增加 `subscriptions` 脱敏摘要，供未来初始化编排器或抓包证据入口显式记录 client `2a10`：
  - payload 长度、声明/实际 entry count；
  - 连续区间数量；
  - 按市场统计条目数；
  - 各市场首末 symbol-index。
- 摘要不包含六字节 entry 列表、原始 payload、hex 或代码表正文；receive-only reader 不会自行产生 client subscription 摘要。
- 新增回归 fixture：`SH 89173..89174 + SZ 65536` 被归纳为 3 entries、2 个连续区间及正确市场首末索引。
- `cargo fmt --all -- --check`、相关 `git diff --check` 通过；等待 webClx 定向 runtime/service 测试、严格 Clippy 和 release 构建。

## 2026-09-02 2a10 摘要验证运行中

- 请求 `110954-18d1330bb455c593` 有活动 worker；runtime 8/8 已通过，包含新订阅摘要 fixture。
- service 状态测试、workspace all-targets Clippy 和 release build 仍在同一请求中继续；最终回调前不登记完整成功、不部署增量。

## 2026-09-02 2a10 摘要验证接近完成

- `110954-18d1330bb455c593` 当前已确认 runtime 8/8、service 2/2、workspace all-targets Clippy 通过。
- release build 已推进到最后一个目标，worker 仍 active；等待最终匹配回调后再部署订阅摘要增量。

## 2026-09-02 2a10 摘要验证完成

- `110954-18d1330bb455c593` 匹配回调成功，日志 `/home/bin/webclx/logs/quoteNetzipRs/3132_build.log`。
- runtime 8/8、service 2/2、workspace all-targets Clippy、release build 全部通过。
- 准备通过 webClx 部署订阅摘要增量；仍保持 research-only、receive-only 和初始化门禁。

## 2026-09-02 2a10 摘要部署运行中

- 部署请求 `111156-18d1330bb455c594` 有活动 worker；release build 已完成，必需产物 `target/release/quoteNetzipRs` 已确认存在。
- 当前处于安装/审计阶段，尚未生成最终 install-report；回调前不修改 Rust 源码、不重复部署、不宣称运行版本已更新。

## 2026-09-02 zcode（netzip_win / netzip-fullpull 协作）：2a10 订阅铺贴规则在两个独立生命周期中确认

身份：zcode，负责 `Z:\stock\netzip_win` 与共享 crate `Z:\stock\crates\netzip-fullpull`；与本项目使用同一 fullpull 协议栈，协作进度同步记录于 `Z:\stock\netzip_win\progress.MD`。

- 确认 pcap 链路类型为 Linux SLL2（276），每包 20 字节虚拟头；此前的通用提取脚本方向失败即源于此。5188 帧真实头部布局为 `kind(2B wire order) + length(2B LE) + metadata(4B)`，长度在偏移 2 而非 6。
- 按正确布局从 `formal-primary-0006` 抓包完整恢复全部 7 条客户端 `2a10` 订阅，条目为 `市场(2B ASCII) + 符号索引(u32 LE)`，与 `0104` 代码表的符号索引空间对应：
  - 五条 1024 条大连接按市场索引空间连续铺贴：SH `0x15c55→0x16055→0x16455`（SH 段 268 条后转 SZ `0x10000→0x102f4→0x10c12→0x11012`），完整覆盖 SH+SZ 主索引区间；
  - 第六条 1024 条为离散混合集合（128 个小区间，SH/SZ 交错），疑似自选/用户集合；
  - 第七条 58 条为另一个小离散集合。
- **独立生命周期验证通过**：从 `cold-sync-2311`（2026-09-01 抓包，独立登录会话）提取全部 7 条 `2a10`，七个分区与 formal-primary-0006 完全一致（含离散集合与 58 条小集合）。跨会话逐字节同构，铺贴规则具备可预测性。
- 各连接 `2704` 首帧记录数（166/122/141/149/159/57）反映该分区内发生变化的证券数，而非订阅总量（五条大连接订阅量同为 1024）。
- 意义与边界：这是首个可跨生命周期复现的初始化构造规则候选（连接数与分区由市场索引空间确定）。`2d10` 四词值、`3610` 动态字段、ACK manifest 运行时来源仍为 `needs-verification`；在全部动态字段闭合前，netzip-drivers 保持 `InitializationUnavailable` 门禁，不发送静态 payload。
- 对 shadow reader 协作的建议：其 receive-only 链路如能在交易时段按 flow 归属记录 `2a10`（可先只记长度/区间摘要，不暴露 raw payload），即可低成本获得第三次在线验证；若某条连接的订阅区间在用户增删自选股后单独变化，还能判定离散连接的语义。
- 双方共同门禁不变：自动化联网测试仅测试账号 168、不发送静态初始化帧、凭据与抓包脱敏、不触碰正在运行的官方网际风会话。

## 2026-09-02 zcode（netzip_win / netzip-fullpull）：2d10 词值跨会话稳定性与分组通道排除

- 从 formal-primary-0006 与 cold-sync-2311 两个独立抓包提取全部每连接 `2d10 x3`：**词值跨会话完全相同，全局仅两组**，由 `word1` 区分（`0x28252746` / `0x2825b246`）；`word0`（SH/SZ/0x4224 + `0x0a01`）、`word2`（`0x1f030135`/`0x1eff0135`/`0x23870135`）、`word3`（`0x6a96`）在两组内一致。
- 但同一订阅分区在两个会话中的分组不同（如 SH 268+SZ 756 连接：会话1=A 组、会话2=B 组），**分组不能由订阅分区推导**。
- 排除项：分组词不在同连接 `3210` 载荷（466/467/468B 全量搜索）；不在 6100 主控制流（38908B/32328B）、三个辅助 6100/7100 流的明文或字典解压后内容中（用 crate 自带 Stock.字典成功解压全部 68 帧 ZSTD 对象后搜索，命中 0）。分组指派通道仍未定位（候选：加密对象内部、5188 连接早期服务端帧、或本地状态）。
- 工具性副产品：SLL2 pcap + 字典 ZSTD 的定向提取/搜索脚本流程已在本轮验证可复用。
- 门禁不变：`2d10` 的"运行时来源未闭合"结论维持；两组值虽跨会话稳定，但仍属抓包事实，不作为生产默认值写入。

## 2026-09-02 zcode（netzip_win / netzip-fullpull）：3110(362) 新变体与"客户端本地选择"假设

- `2d10` 分组词进一步排除：两个会话全部 `3110`（48B/632B/362B）载荷中均无 `0x28252746`/`0x2825b246` 字节模式。
- **新帧变体**：部分连接在初始化后收到第三个 `3110(362B)`（两个会话均出现，cap1 六条、cap2 五条），与分组（A/B）无对应关系；现有 Rust 验收门（3110: 48 | 631|632|636）覆盖的是初始化交换期，362 属于初始化后的服务器帧，接收循环按 opaque 处理即可，不需改验收门，但文档应记录该长度。
- 本两次抓包中第二条 `3110` 一律 632B；旧文档的 631/636 分组结论来自 vendor_pm 样本，不适用于这两次会话。
- **判别假设（供下次授权在线实验）**：`2d10` 分组词跨会话稳定、同分区跨会话分组不同、且不在任何下行通道（3110/3210/6100/7100 明文与字典解压）中出现——指向"客户端本地选择，服务器两种都接受"。验证方法：正式账号授权后，同一会话内对同分区连接分别发送两组 `2d10`，观察 2704 推送是否正常。若成立，`2d10` 运行时构造只剩本地一致性要求，不再需要服务端指派通道。
- 门禁不变：以上均为抓包/离线分析结论；未授权前不做在线 2d10 实验。

## 2026-09-02 2a10 跨会话脱敏比较工具验证

- 新增离线工具 `scripts/compare-official-5188-subscriptions.py`，只比较 summary JSON 中的订阅帧数、条目数和连续区间；规范化时明确忽略 src/dst、本地 TCP 端口和抓包时间戳，不读取或输出凭据及原始 payload。
- 对以下两个独立生命周期运行比较：
  - `diagnostics/20260902-live-pair/formal-primary-0006/primary-5188-summary.json`；
  - `diagnostics/20260901-live-pair/cold-sync-2311/primary-5188-summary-current.json`。
- 结果：`all_match=true`；每个会话均为 7 条 subscription、合计 6202 entries；七条规范化 shape SHA-256 逐项相同（排序后前缀：`1ac62fb2d75f`、`33728f7a192a`、`620ec6baaf88`、`92ea3e1b5c8f`、`b5c966146aa1`、`bd7c6d253ecf`、`f898aeb6823f`）。
- `python3 -m py_compile scripts/compare-official-5188-subscriptions.py` 通过。
- 证据等级仍为跨会话强候选，不提升为生产可构造：`2d10` 当前会话来源、动态 `3610`、ACK manifest 来源和 slot assignment 尚未闭合；需第三次交易时段及自选股变更实验继续判别。
- `111156-18d1330bb455c594` 已部署成功，安装审计 `/home/bin/webclx/logs/quoteNetzipRs/3133_install-report.json`；本次新增内容仅为离线比较脚本和证据记录，不重复部署运行服务。

## 2026-09-02 权威文档跨会话证据纠错

- 并行只读审计发现权威文档仍混用旧 `vendor_pm` 会话结论与新独立生命周期证据，现已更新 `docs/fullpull-replication-authority.md` 和 `docs/codex/tasks/official-5188-production-wiring.md`。
- 当前统一结论：`2a10` wire entry 已确认是 `market[2] + symbol_index(u32 LE)`；两个独立生命周期的七分区形状一致（`6 x 1024 + 58 = 6202`），但 runtime set/slot selection 仍未建立。旧 `122` 只作为历史会话或 `2704` 变化记录量语境保留，不再作为当前小订阅分区大小。
- `vendor_pm` 中 `3110(631/636)` 与两组 `2d10` 的相关性仅保留为单样本事实；新两会话的第二 `3110` 均为 632B 且仍出现两组，长度和订阅分区均被排除为通用分组规则。
- 全量搜索没有在 `3210(466/467/468)`、任一 `3110` 或已解码 6100/7100 对象中找到稳定 `2d10` 分组词，因此删除“3210 是 2d10 seed prime candidate”的现行表述。`3210` 只证明 ACK stage 被接受。
- `2d10` 结论细化为：两相邻独立会话内两组具体词值稳定，但更早日期/批次存在值演化；runtime seed/group selection 仍 `needs-verification`，不得写入静态默认值。
- 生产任务第 2 项已区分收发方向：客户端生成当前会话 `3610`，服务端 `3110/3210` 只做阶段响应验证，`2d10 x3/2a10` 仅能由独立建立的运行时规则构造。
- 相关文档和脚本 `git diff --check` 通过；本轮仅文档与离线工具变化，不触发编译或重复部署，`InitializationUnavailable` 保持。

## 2026-09-02 zcode（netzip_win / netzip-fullpull）：vendor_pm 第三生命周期 2d10 对比——结构恒定、值随日期演化

- vendor_pm `client-full-frames/*_2d10_*.bin` 含 8 字节帧头（`2d10 2000 00000000`），剥头后 30 帧（10 连接 x3）全部通过"16 字节尾零"结构校验。
- 跨日期对比（8-31 vendor_pm vs 9-01/9-02 两个 pcap 会话）：
  - **结构恒定**：`word0` 三帧同为 SH/SZ/0x4224 + `0x0a01`；`word2` 低 16 位恒为 `0x0135`；仍恰好两组，判别位置仍在 `word1` 字节 1（`0x27` vs `0xb2`）。
  - **值演化**：`word1` 低 16 位 8-31 为 `0x27df`，9-01 23:11 与 9-02 00:06 两个独立会话**相同**为 `0x2825`（差 0x46=70）；`word2` 高 16 位 `0xcd85→0x1f03`；`word3` 由每帧不同（`0xcd85/0xcd83/0xd10c`）变为全会话恒定 `0x6a96`。
- 推论：`2d10` 词值不是常量也不是随机数，而是随"某个日期/服务器批次种子"演化、在同一时间窗口内稳定。9-01 深夜与 9-02 凌晨两次独立登录共享同值，说明种子粒度 ≥ 半天以上，且不由本地登录会话决定。
- vendor_pm 内分组：A 组=3105/3106/3107/3108/3110/3113，B 组=3104/3112/3117/3120；与旧文档"3110 631/636 长度对应分组"的观测一致（该样本第二条 3110 确为 631/636 两档）。
- 对"客户端本地选择"假设的修正：既然值随种子演化且同窗口跨会话相同，客户端更可能是"本地按可复现算法从日期/配置种子计算"或"从某未定位缓存/通道取得"。下一步可尝试把 `0x27df/0x2825` 与日期字段（如天数序号、CRC、时间戳截断）做数值匹配。
- 门禁不变：全部为离线分析；未授权不做在线实验。

## 2026-09-02 zcode：种子值控制通道搜索（阴性）

- 用厂商字典对全部 14 条 6100/7100 控制流做 ZSTD 解压（共约 4.4 万字节解压输出），搜索 `word1` 种子字节（`25 28` / `df 27`，两种端序）与 `word3`（`6a96`）：
  - 种子字节命中 0——控制通道明文（解压后）不下发该种子；
  - `6a96` 在 39140 流的 804B 解压对象中命中 1 次，按 14 流 × 数千字节的随机命中概率（约 15%）判断大概率为巧合，不作证据。
- 结合前两轮：`2d10` 种子既不在 5188 初始化/服务端帧，也不在 6100/7100 解压对象中。剩余假设收敛为两个：① 客户端按可复现算法从日期/配置本地计算（`0x27df→0x2825` 的 +0x46 演化为数值匹配目标）；② 加密控制对象内部传递（需先破 `加密包` 内层才可验证）。
- 建议判别顺序：先做①的数值匹配（零成本、离线）；①失败再考虑②。授权在线实验（同会话发两组 2d10）仍是最终判别手段。
- 全部门禁不变；本轮均为离线 pcap/对象分析。

## 2026-09-02 zcode（netzip_win / netzip-fullpull）：2d10 word1/word3 派生规则破译（三个生命周期验证）

- **word1 高 16 位 = 交易日日期整数 mod 65536**：`20260831 & 0xffff = 0x27df`（vendor_pm）、`20260901 & 0xffff = 0x2825`（9-01 23:11 与 9-02 00:06 两个会话——跨零点仍用 9-01 交易日，自洽）。三个生命周期全部吻合。
- **word1 低 16 位为固定组标签**：`0x2746`（A 组）/`0xb246`（B 组）在三个生命周期中完全相同——不是会话值，是常量。word1 = `(交易日 & 0xffff) << 16 | 组标签`，**已完全可推导**（仅剩"该选哪个组"的语义）。
- **word3 = 时间桶 `(unix秒 >> 16) & 0xffff`**：两会话为 `0x6a96`，vendor_pm 为 `0x6a94`（早约 36 小时，恰好两个 65536 秒桶）。可由当前时间直接推导。
- **word0**：三帧固定为 SH/SZ/0x4224 + `0x0a01`，常量。
- **word2**：低 16 位 `0x0135` 恒定；高 16 位随帧/日期变化（cap1: `0x1f03/0x1eff/0x2387`；vendor_pm: `0xcd84/0xcd85`）——是唯一剩余未知项，呈时间类特征但尚未找到函数。
- 判别意义：`2d10` 从"四词全未知"收敛到"仅 word2 高 16 位未知 + 组选择语义未知"。下一步：① 对 word2 高 16 位做时间类函数匹配（毫秒计数、桶内偏移等）；② 组标签语义（A/B 疑似服务器负载通道，或可任选）。
- 门禁不变：派生规则尚未通过在线实验确认（尤其服务器是否接受两种组标签、word2 是否必须精确），Native 门禁保持；不把未闭环规则写入生产默认。

## 2026-09-02 zcode：word2 高 16 位时间敏感性确认（拟合边界记录）

- 用 cap1 帧级 pcap 时间戳拟合（10 连接 x 前 2 帧）：frame1 恒为 `0x1f03`，frame2 恒为 `0x1eff`，且两帧间隔约 0.02–3.5 秒（不同连接不同）；值差 -4。
- 规律：变化量 ≈ -4/3.5s ≈ -1.14 单位/秒，符合"递减计时器/倒计时"特征；跨连接同秒同值（48516 于 00:07:10 与 48532 于 00:07:10 均 1f03），说明是**绝对时钟函数**而非连接本地计数。
- 反例：frame3 的 `0x2387`（=9095）与同刻递减斜率矛盾——按 -1.14/s 推算需早于 frame1 约 17 分钟，但三帧实际同批发送。故 frame3（0x4224 市场次）的 word2 与前两帧不同源，或 word2 编码为"模运算后的时钟"（递减可由模翻转解释的部分被掩盖）。
- 结论：word2 高 16 位确认为绝对时间类字段（与前 word3 时间桶结论一致），但精确函数（单位、纪元、模数）尚需更多时间跨度的样本点。当前 5 个样本点、2 个时间窗不足以前闭拟合；等待下一个交易日的抓包（word1 将变为 `0x2826=20260902`）可提供第三时间窗。
- 离线可做的收尾：将已确认的派生规则（word0 常量、word1=日期<<16|组标签、word3=时间桶、word2 低16=0x0135）固化为 crate 内 evidence-only 文档/测试夹具断言，便于次日抓包一键验证——此为建议下一步。
- 门禁不变。

## 2026-09-02 zcode：2d10 派生规则已固化为 netzip-fullpull evidence-only API 与测试

- `Official5188ClientSessionEnvelope` 新增 evidence-only 派生接口（`Z:\stock\crates\netzip-fullpull\src\official_5188.rs`）：
  - `observed_word0(frame_index)`：三帧市场常量 SH/SZ/0x4224 + `0x0a01`；
  - `observed_word1(trading_day, SessionGroupTag)`：`(交易日 & 0xffff) << 16 | 0x2746|0xb246`；
  - `observed_word3(unix_seconds)`：`(unix秒 >> 16) & 0xffff`；
  - `OBSERVED_WORD2_LOW = 0x0135`（高半保持 caller-supplied，未声明已解）；
  - `matches_observed_derivation(envelopes, trading_day, unix_seconds)`：对一组 3 帧 `2d10` 一键校验全部已确认规律。
- 新增回归测试 2 个：formal-primary 夹具（20260901 + 0x6a96 桶）通过、错误交易日/未知组标签拒绝；vendor_pm 夹具（20260831→0x27df、组标签双值、0x6a94 桶函数）通过。
- 验证：crate 130 passed / 1 ignored；`cargo clippy -p netzip-fullpull --all-targets -D warnings` 通过。顺手把 `3210` 长度验收改为 `466..=468` 范围模式（等价重写，消除新 clippy lint）。
- 次日抓包验证方法：提取新会话 `2d10 x3` 词组 + 抓包秒级时间戳，调用 `matches_observed_derivation(&frames, 20260902, ts)` 即可判定规则是否继续成立（word1 应为 `0x2826_xxxx`）。
- 边界不变：API 名以 observed_ 前缀明确标注为观测证据；word2 高半与组标签语义未解，不构成生产构造器，Native 门禁保持。

## 2026-09-02 zcode：word2 高 16 位与组标签相关（同秒 +1 偏移）

- 自适应链路层解析 vendor_pm.pcapng（398312 包）成功提取 10 连接全部 30 帧 `2d10`。
- 同一秒内的关键对比：
  - frame1：Primary 组(0x2746)恒 `0xcd84`，Secondary 组(0xb246)恒 `0xcd85`——**同秒内组间恰差 +1**；
  - frame2：两组均 `0xcd83`；frame3：两组均 `0xd10c`。
- 结合 cap1（全 Primary：`1f03/1eff/2387`）：word2 高 16 位 = 绝对时间函数 + 组标签小偏移（至少 frame1 如此）；frame2/3 的组间无偏移。递减斜率 -8/7 单位每秒（=-1.143）拟合 frame1→frame2 的时间差成立，但 frame3 仍偏离。
- vendor_pm 时间戳为非 Unix 刻度（416336 这类小整数），需解析 pcapng IDB 的 if_tsresol/if_tsoffset 换算后才能做跨日拟合；当前跨日常数不可解。
- 下一步：① 正确解析 pcapng 时间分辨率后用 vendor_pm+cap1 两窗联立拟合 K 与单位；② 下个交易日抓包直接用 `matches_observed_derivation(&frames, 20260902, ts)` 验证 word1 规则，同时采样第三个时间窗破解 word2。
- 门禁不变。

## 2026-09-02 权威进度核验与当前推进点

- 已核对 `windows_debug/progress.md`、`/home/codes/stock/netzip_win/progress.MD` 和权威 `netzip-fullpull` 代码；zcode 的最新条目与 `observed_word*` API、`SessionGroupTag`、`matches_observed_derivation` 测试一致。
- 最新共识边界：`2a10` 七分区形状跨会话复现；`2d10` 的 `word0/word1/word3` 与 `word2` 低 16 位已固化为 evidence-only 规则；`word2` 高 16 位和组选择仍未闭环，`InitializationUnavailable` 必须保持。
- 当前直接推进点是 `word2` 高 16 位：vendor_pm 的 pcapng 时间戳为 `(high + low/1e6)` 非 Unix 秒，权威进度中“修时间分辨率后两窗联立拟合”的缺口仍然有效。
- 本地首个 pcapng 解析尝试为 0 帧，原因是把 vendor 帧错当 classic TCP 头解析；下一步按 pcapng EPB 原始 IPv4 + `tcp_start = ihl + 20` 重新提取，然后直接输出 `(精确时间, frame_index, word2_high, group)` 供两窗拟合。

## 2026-09-02 zcode：word2 跨日拟合尝试（未收敛，按边界收口）

- vendor_pm 采样时刻：README 确认为 8-31 午后交易时段；文件 mtime 13:08（转换完成时刻），实际捕获窗口只能框定在 13:00–15:00 CST（pcapng 精确时间戳解析因内存/换算问题暂未完成）。
- 以 frame1 Primary 值 `0xcd84`(52612) 与 cap1 `0x1f03`(7939) 联立：无任何常数斜率能同时满足本地实测斜率（-8/7 单位/秒）与跨日差值——排除"简单线性倒计时"模型。
- 剩余可能：模翻转掩蔽、非线性时钟（如 tick 计数器）、或 word2 高半还叠加了非时间成分。
- 保持的确认项：word2 高半为绝对时间类、同秒组间 +1 偏移（frame1）、-8/7 每秒的本地变化率。
- 结论：word2 高半的精确函数以当前 2 个时间窗不可解，不再消耗离线预算；等待下个交易日抓包（用 `matches_observed_derivation` 先验 word1，再以第三时间窗联立拟合 word2）。授权在线实验（组标签 A/B 可否任选）仍是最短闭环路径。
- 门禁不变。

## 2026-09-02 word2 高 16 位两窗联立拟合完成（当前会话）

- 新增离线工具 `scripts/analyze-official-5188-2d10-word2.py`；`py_compile` 通过。
- 修正 vendor_pm pcapng 读取方式：EPB 固定头实际为 `interface_id/high/low/caplen/origlen`，包体从偏移 20 开始且保留原始 IPv4；TCP 起点为 `ihl`。已完整提取 10 连接 30 帧；无凭据、无原始 payload 输出。
- 对 formal-primary-0006 的 SLL2 pcap 同法提取 30 帧。两窗均使用帧完成时间。
- 首帧样本的 `word2` 高 16 位差（formal − vendor）为：
  - SH `010a4853`：A 组 +20863，B 组 +20862；
  - SZ `010a5a53`：A/B 组 +20860；
  - `4224`：A/B 组 +21115。
- 若不发生 65536 回绕，导出单位耗时边界：
  - SH/SZ 约 `6.1976..6.1980` 秒/word2 单位；
  - `4224` 约 `6.1227..6.1232` 秒/word2 单位。
- 该结果否定“三个市场帧共用同一个简单递减时钟”的现行假设；`4224` 帧与 SH/SZ 帧存在约 1.2% 的稳定标度差。也说明此前 -1.14 单位/秒的近似拟合不是通用规则。
- 输出 `word2_delta / time_delta` 后，仍存在 65536 回绕替代解（例如 SH 约 1.4964 秒/单位、SZ 约 0.8510 秒/单位、4224 约 0.8495 秒/单位），当前两窗不足以唯一选择；需要第三个交易时间窗跨回绕样本判别。
- 结论等级：`word2` 高 16 位为市场相关时间/递减量，已是量化证据；仍 `needs-verification`，不写入生产构造器，`InitializationUnavailable` 保持。

## 2026-09-02 zcode：vendor_pm 绝对时间确认不可恢复（离线拟合正式收口）

- 流式解析修正三处解析错误（SHB 字节序魔数偏移、EPB 字段偏移、SLL2/原始 IP 自适应）后成功遍历全部 398312 个 EPB、命中 60 处 `2d10`（含重复帧，词值一致）。
- 确认：该 pcapng 的时间戳高 32 位为 0，全部包落在 `1970-01-01 08:00:00.416` 同一毫秒附近——pktmon ETL→pcapng 转换丢失了绝对纪元。vendor_pm 的墙钟无法从文件恢复。
- 结论：word2 高 16 位的跨日联立拟合在离线层面正式不可行；其精确函数（单位/纪元/模数/组偏移）必须靠第三时间窗（下个交易日抓包）或授权在线实验。
- 保留成果：三个生命周期验证的 word1（交易日+组标签）、word3（时间桶）、word0 常量、word2 低 16 位常量已固化为 `netzip-fullpull` 的 `matches_observed_derivation` 一键校验；frame1 同秒组间 +1 偏移与 -8/7 每秒本地变化率已记录。
- 建议下一次采集（交易时段）：保存原始 pcap/pcapng 时确认含绝对时间戳；采集后先跑 `matches_observed_derivation(&frames, 20260902, ts)` 验证 word1，再以新时间窗拟合 word2。
- 门禁不变。

## 2026-09-02 word2 工具验证排队

- webClx 纯编译请求 `115612-18d1330bb455c595` 已成功排队；这是对新增离线脚本和当前工作树的 release 基线检查。
- 该请求不改变服务、不发送 5188 初始化帧、不解除 `InitializationUnavailable`；等待匹配回调后再登记编译结果，不轮询日志。

## 2026-09-02 word2 工具基线验证完成

- webClx 请求 `115612-18d1330bb455c595` 匹配回调成功，日志 `/home/bin/webclx/logs/quoteNetzipRs/3134_build.log`。
- `cargo build --release` 通过，41.63 秒完成；本变更仅新增离线分析脚本与证据记录，不触发部署，运行服务保持 `3133_install-report.json` 对应版本。
- 当前下一步不变：下个交易时段获取第三时间窗，用同一脚本比较 `word2` 差值并判定是否存在 65536 回绕；生产初始化门禁继续关闭。

## 2026-09-02 zcode（netzip_win / netzip-fullpull）：slot supervisor 落地（上线路径第 3 缺口关闭）

- 新增 `Z:\stock\crates\netzip-fullpull\src\slot_supervisor.rs` 并注册到 lib.rs：
  - `StopFlag`：协作取消，`wait` 以 10ms 步进可提前唤醒（退避睡眠可被 stop 打断）；
  - `SlotSupervisor::start(slots, max_attempts, BackoffPolicy, stop, SlotJob)`：slot 数量来自调用方当前 assignment（可为空），每 slot 独立线程；
  - `SlotJob` 契约：每次调用必须执行**完整生命周期**（连接+交错初始化+接收循环），重试天然不复用旧 decoder 状态；
  - 错误分类 `Retryable/Fatal`：Retryable 走有界指数退避（base*2^n，封顶 max），Fatal 或达到 max_attempts 终止该 slot；`Ok` 表示 slot 有意结束，不重连；
  - `stop()` 幂等：置位 + join 全部线程；事件通道只暴露 slot 编号与策略数值，无 payload/凭据。
- 单元测试 6 项全绿：退避有界、stop 唤醒睡眠、双 slot 独立重试至各自成功、Fatal 不重试、幂等 stop 且停止后零尝试、空 assignment 干净启停。
- 验证：fullpull 136 passed / 1 ignored；netzip_win workspace 测试、strict clippy、i686 release 构建全部通过。
- 上线缺口更新：~~多 slot supervisor~~ 已完成（基础设施层）；剩余关键路径 = ① 授权在线实验（2d10 组标签/word2 判别 + 真实初始化一次走通）→ ② native.rs 接入 supervisor+interleaved 初始化（框架已就绪，等 ① 的结论开闸）→ ③ 交易时段 callback parity。
- 门禁不变：supervisor 是策略层，不发送任何初始化字节；native.rs 尚未接线。

## 2026-09-02 当前交易时段在线状态核对（12:00 后）

- 正式 Wine 适配器 `quoteNetzipWine.exe` 03:23 启动，`/api/v1/status` 为 ok；启动阶段已收到 18,564 条行情、6/6 次成功 POST，无丢失。
- 但事件最后时间为 10:10:04，当前 12:03+；事件流只持续 100 秒心跳（`心跳包`/`认证服务器`），13 分钟后变为`股票备用服务器`，此后无行情。当前主机无 5188/6100/7100 TCP 连接。
- 权威进度新增的 word2 工具与 release 基线验证均已通过；当前仍不能从本机直接采集第三生命周期。
- 运行服务保持部署版本 `3133`、health ok、shadow null、`pending-production-wiring`；新编译产物尚未部署。
- 下一步应处理两条矛盾：① 正式 Wine 只在开盘后短暂连接，收盘前掉线且 supervisor 因缺厂商进程不自动重启；② 当前服务凭据未注入，`/api/auth/login` 会因缺 `NETZIP_TDX_PASSWORD` 失败。需优先恢复正式 Wine 生命周期或重启其 supervisor/vendor 进程，再抓 5188。

## 2026-09-02 第三时间窗采集与权威 crate 跟踪审计启动

- 协作核查确认 `netzip-fullpull/src/slot_supervisor.rs` 已由 `lib.rs` 引用并通过另一协作方的测试，但文件被 `/home/codes/.gitignore` 的顶层目录规则忽略，普通 `git status` 不可见；这会在提交/迁移时形成“模块引用存在、实现文件缺失”的交付风险。本轮将显式纳入版本控制可见范围，并仅规范化同批 `lib.rs` 的 CRLF，不改 supervisor 行为。
- 正式 Wine 当前是 adapter 存活、厂商进程缺失的失配状态；supervisor 按设计拒绝自动重启。计划先在 `any` 接口启动 `5188/6100/7100` 全长限时抓包并保存重启前脱敏状态，再通过 webClx 执行已验证的 `set-host-primary-login.sh true` 原子冷重启。
- 本次采集目标：获得带真实绝对时间戳的新生命周期 `3210 -> 2d10 x3 -> 2a10 -> 2704`，验证交易日 `word1=0x2826_xxxx` 并消除 `word2` 的 65536 回绕歧义；抓包完成前不解除 Native 初始化门禁。

## 2026-09-02 第三时间窗采集运行中

- 已启动 systemd 临时采集单元 `quote-netzip-third-window-121528.service`，接口 `any`、snaplen 0、过滤 `5188/6100/7100`，目标文件 `diagnostics/20260902-live-pair/formal-primary-121528/wine-primary-sync-121528.pcap`；启动时为 24B 空头，证明采集先于冷重启就绪。
- 已通过 webClx 排队 Wine 部署/冷重启请求 `121540-18d1330bb455c596`：先运行三个既有重启 fixture，再原子执行 `set-host-primary-login.sh true`，安装审计仅覆盖脚本本身；回调前不重复请求或直接操作 systemd 服务。
- `slot_supervisor.rs` 已显式 `git add -f`，避免顶层忽略规则使实现丢失；`lib.rs` 已恢复 LF，`git diff --check` 通过。两份 Wine Markdown 的误执行位也已恢复为 0644。
- 已纠正交接/生产任务中的过时结论：初始登录为 19 字段普通 ZSTD；12 字段 `加密包` 属于 per-5188 控制阶段；两个独立生命周期已确认 `2a10` 编码/七分区；`2d10` 仅剩 word2 高半与组选择；多 slot supervisor 已落地但尚未接入 Native job。

## 2026-09-02 word2_analysis agent：2d10 市场版本戳来源闭环（纠正时钟拟合假设）

- 全目录对 classic pcap/pcapng 做 SLL2/raw-IP/Ethernet 自适应 TCP 重组扫描：只有 `cold-sync-2311` 与 `formal-primary-0006` 含完整客户端 `2d10 x3`，两者属于同一代码表版本窗口；其他现存短抓包没有可恢复的第三组 `2d10`。该阴性结果避免把重复采样当独立 seed。
- 在服务端 `0104` 解压对象的 98B 代码表头发现逐字节回显关系。以 `vendor_pm` B 组 SH 为例：头部 `0a01 46b2 85cd 946a` 对应客户端帧中的 protocol `0x010a`、group `0xb246`、`word2_high=0xcd85`、`word3=0x6a94`；SZ、4224、A/B 两组及 formal 窗口全部逐项一致。
- 关键派生：`market_version_seconds = (word3 << 16) | word2_high`。三窗结果是合法的逐市场 Unix 秒版本戳：
  - SH：`1788136836`（2026-08-31 08:40:36）→ `1788223235`（09-01 08:40:35）→ `1788309634`（09-02 08:40:34），差 `86399/86399` 秒；
  - SZ：`1788136835` → `1788223231` → `1788309631`，差 `86396/86400` 秒；
  - 4224：`1788137740` → `1788224391` → `1788310541`，两日总差 `172801` 秒。
- 第三窗来自当前正式 Wine 运行目录于 12:16 更新的 `大智慧/{SH,SZ,B$}代码表.dat`。其文件内 `0104` 外壳后的头分别为 `0a01 46b2 8270 976a`、`...7f70...`、`...0d74...`，直接给出交易日 20260902 的 B 组候选词：
  - SH `010a4853 2826b246 70820135 00006a97`；
  - SZ `010a5a53 2826b246 707f0135 00006a97`；
  - 4224 `010a2442 2826b246 740d0135 00006a97`。
- 因此旧“word2 高半是独立倒计时/市场相关时钟且需回绕拟合”假设被否定。完整结构现在是：`word0=market+header.protocol`，`word1=(交易日&0xffff)<<16 | header.group`，`word2=(market_version_seconds&0xffff)<<16 | 0x0135`，`word3=market_version_seconds>>16`。这解释了此前所谓 `word3` 时间桶和所有跨日 delta，而无需猜单位或回绕。
- `scripts/analyze-official-5188-2d10-word2.py` 已改为解析/校验代码表头及输出 evidence-only 词组，并保留两个 CSV 窗口的版本秒比较；`python3 -m py_compile` 通过。工具不发送流量、不选择 slot/group。
- 仍未闭环：A/B group/slot assignment 语义。当前落盘代码表只保留最后一次 B 组头，抓包显示服务端按连接组分别返回 A/B 头；必须用正在采集的新生命周期核对客户端 2d10 与冷启动前缓存头，才能证明 Native 应如何为各 slot 选择组。`InitializationUnavailable` 门禁保持，不把上述候选写成生产默认。
- 下一判别实验：保存冷启动前代码表头快照；对新抓包逐连接比较 `2d10` 与缓存头、随后服务端 `0104` 头，确认是“客户端从缓存版本戳请求增量，服务端回写新版本戳”还是同版本回显，并关联 A/B 到连接顺序/endpoint/订阅分区。

## 2026-09-02 第三时间窗完整链路与 2d10 运行时来源闭环

- webClx Wine 冷重启请求 `121540-18d1330bb455c596` 成功，日志 `/home/bin/webclx/logs/quoteNetzipWine/3003_build.log`、审计 `3003_install-report.json`；厂商进程和 adapter 同时恢复，Wine 回调出现“初始化完成”及 3,059,200B 股票数据。
- 新抓包 `diagnostics/20260902-live-pair/formal-primary-121528/wine-primary-sync-121528.pcap` 为 17,555,396B，带真实 Unix 微秒时间戳；包含 10 条完整 5188 连接、30 条 `2d10`、7 条 `2a10`、40 条服务端 `0104` 及 16 条 `2704`。采集单元已停止，避免无界增长。
- 通用提取器已补 SLL2/SLL1/Ethernet/raw-IP 自适应、微秒/纳秒 pcap 时间戳、镜像接口去重、帧完成 Unix 时间，以及 0104/1504 等 zlib 对象的模 65536 扩展长度；修复前会在首个超 64KiB `0104` 后脱同步并产生伪 kind。
- **运行时因果顺序已确认**：每条连接先收到 SH/SZ/B$/SFC 四个服务端 `0104`；前三个解压头直接给出该连接唯一 A/B group、protocol 与市场版本秒；客户端随后发送的 `2d10 x3` 逐字段回显这些头。A/B 不需客户端猜测、也不需静态 slot 映射。
- 新窗每条连接的公式完整成立：`word0=market+protocol`；`word1=(20260902&0xffff)<<16|group`；`word2=(version_seconds&0xffff)<<16|0x0135`；`word3=version_seconds>>16`。10 条连接严格 5A+5B，具体连接/订阅分区分配跨生命周期变化，进一步排除固定轮转模板。
- 小订阅由前两窗 58 条变为本窗 59 条，证明该离散集合为动态集合，不得固化历史 358B payload；本窗相应 `2a10` 为 364B。
- 权威 crate 新增 `Official5188CodeTableHeader` 与 `build_official_5188_client_session_triplet`：严格校验 `0104` routing header、同连接 group 一致性和 SH/SZ/B$ 完整性，按真实顺序生成当前会话三帧；旧 evidence-only 校验已提升为完整版本秒比较。
- 第三窗还补齐 ABK/ACK-source 形状：旧窗有效 `632@NUL35/358`，新窗 B=`633@NUL411`、A=`636@NUL286`，另保留既有 `631@64`、`636@111`；代码只扩展这些精确观测组合，不接受任意长度/NUL。
- slot supervisor 审计修复：空 assignment 的 `join_terminal` 原先无限等待、多 slot 总等到 timeout；现记录预期 slot 数并在全部 terminal 后立即返回，`StopFlag::wait` 使用饱和剩余时间避免截止点竞态，首轮 retry backoff 从 base 开始。
- 当前核心门禁从“2d10 word2/group 未知”缩小为：把 0104 收集→2d10 triplet→动态 2a10 集合的交错状态机接入 Native，随后完成同符号/同时间 2704 与 Wine callback parity。代码通过 rustfmt、Python `py_compile` 和 `git diff --check`；Rust 回归正准备经 webClx 提交。

## 2026-09-02 word2_analysis agent：formal-primary-121528 在线闭环与新 ABK 门槛矛盾

- 新正式生命周期 `diagnostics/20260902-live-pair/formal-primary-121528/wine-primary-sync-121528.pcap` 已包含 10 条完整 5188 初始化连接。客户端 30 条 `2d10` 全部使用交易日 `0x2826` 与上述当前代码表版本戳；A/B 分组为 5/5。
- 同连接后续服务端 `0104` 解压头逐项精确回显客户端组和值。B 组为 SH/SZ/4224 `7082/707f/740d + 6a97`；A 组为 `7082/71c8/740d + 6a97`。这证明版本戳来源关系不是旧抓包巧合，并揭示 SZ 的 A/B 组版本秒可不同（A=`0x6a9771c8`，B=`0x6a97707f`）；运行时必须按组保留各自代码表头，不能把最后落盘的 B 头广播到所有 slot。
- 组指派现在可从第二个服务端 `3110` 的响应变体判别，但不能只靠旧长度规则：
  - `formal-primary-0006`：A/B 都是 632B，却分别是两个稳定 payload 变体；first-NUL 分别为 358/35；
  - `formal-primary-121528`：A 恒 636B、first-NUL=286；B 恒 633B、first-NUL=411；组内 payload 完全一致，组间不同。
- 这暴露了权威 crate 的现实兼容性缺口：`official_5188_ack_source_prefix()` 当前只接受 `631@NUL64`、`632@NUL35`、`636@NUL111`，且 ABK server length 只接受 `631|632|636`。它会拒绝已被 Wine 完整接受并进入 `2d10/2a10/2704` 的 `633@411`，也会拒绝当前 `636@286` 和旧 A 组 `632@358` 的 ACK source。该发现已通知主 agent；修复必须基于 Wine 实际 ACK packer 语义，而不是盲目放宽任意 NUL。
- 生产门禁继续保持。本条关闭的是 `word2/word3` 来源问题，不等于已闭合第二 `3110 -> group -> ACK source` 的通用解析规则。

## 2026-09-02 word2_analysis agent：历史代码表头交叉验证

- 独立于 2026 抓包，`/home/codes/third_party/hqw.bak/大智慧` 的 2023-09-22 三张代码表仍使用同一头布局。其拼接版本秒分别解为 SH `2023-09-22 08:50:51`、SZ `08:50:50`、4224 `09:01:49`，再次确认 `(header[6..8] << 16) | header[4..6]` 是逐市场 Unix 版本戳，而非偶然拟合。
- 旧头的 protocol=`0x14ac`、group=`0x7714`，与当前 `0x010a`、`0x2746/0xb246` 不同。由此新增边界：protocol/group 也是代码表/服务端响应字段，不应永久硬编码为 2026 抓包常量；没有对应 2023 2d10 抓包，因此这一点只作为格式演化证据，不提升为旧协议行为断言。

## 2026-09-02 zcode verification

- Re-ran targeted `slot_supervisor` and official 5188 derivation tests: passed.
- Re-ran `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` using a local C: target directory because the shared Z: target directory rejected `.rmeta` writes with Windows os error 5; both passed.
- Native remains intentionally gated at `InitializationUnavailable`: it authenticates, selects the session-derived official 5188 endpoint, connects, and only observes server frames. It sends no static 3610/2d10/2a10 payloads and does not fall back to DLL, legacy gateway, or 7709.
- The previously authorized formal-account experiment was not executed in this pass. Automated network testing remains restricted to the 168 test account; no credentials or raw session bytes were written to this log.
- Next production blockers remain: derive current-session initialization fields and group assignment, then wire interleaved initialization and slot supervisor into Native, followed by trading-hours callback/public-state parity acceptance.

## 2026-09-02 当前会话代码表收集器

- 通过 webClx 编译请求 `123252-18d1330bb455c597` 核实此前基线：权威 crate 139 passed / 1 ignored，workspace release 测试、全 targets Clippy 和 release 构建均通过。
- 权威 `official_5188` 新增 `SERVER_CODE_TABLE (0104)` 类型和 `Official5188Session::receive_initial_code_tables()`：按当前连接无损收集四个 `SH/SZ/B$/SFC` 代码表对象，保留所有前置帧；只用前三个市场头生成 `2d10 x3`，不广播最后落盘头。
- `0104` 头解析提升为完整 92B 会话头，校验偏移 `0x58` 的交易日低位与 `0x0135` 确认标记；`2d10` builder 不再依赖外部墙钟日期。旧调用测试已修复。
- slot supervisor 新增重复 slot 与 `max_attempts=0` 配置拒绝，并实现 Drop 收尾；相关测试覆盖边界。
- 编译请求 `124250-18d1330bb455c59a` 因测试漏传 `matches_observed_derivation` 的交易日参数失败，已修复；随后请求 `124331-18d1330bb455c59b` 完成：定向测试通过、权威 crate `142 passed / 1 ignored`，workspace release 测试与严格 Clippy 全绿。
- 生产门禁保持：`Auth7100ControlSession` 尚未把 `0104 -> 2d10 -> 动态2a10` 接入 Native slot job；交易日头字段与四头收集已有离线验证，但未据此宣称 Native full-push 完成。
- 新增 `Official5188SubscriptionEnvelope::from_entries()`：由当前会话提供 10B 前缀和分区条目，库校验声明数量/市场并编码动态 `2a10`；不固化 58/59/1024 条历史集合。验证请求 `124613-18d1330bb455c59c` 已排队。
- `124613-18d1330bb455c59c` 已完成：权威 crate `144 passed / 1 ignored`，动态订阅定向测试、严格 Clippy 和 workspace release 全绿。
- 新增 `Auth7100ControlSession::initialize_official_5188_interleaved_with_code_tables()`：ACK 后调用当前数据连接的四帧 `0104` 收集器，把当前头集合传给 post builder；旧兼容入口未改变。请求 `124804-18d1330bb455c59d` 已排队验证该入口。
- `124804-18d1330bb455c59d` 已完成，日志 `3141_build.log`：权威 crate `144 passed / 1 ignored`，workspace release 与严格 Clippy 全绿。

## 2026-09-02 进度镜像：交错初始化入口已验证

- 已将 3141 的验证结果同步到 `/home/codes/stock/netzip_win/progress.MD`：交错入口完成登录、ABK、ACK 后，消费当前连接四个 `0104`，并把代码表集合交给动态 post builder。
- 当前实现只证明会话内 `0104 -> 2d10` 的来源和调用顺序；`2a10` 的动态前缀与分区 assignment 仍要求同生命周期运行时来源，不能写入历史静态模板。
- Native 侧仍是 receive-only shadow，继续报告 `InitializationUnavailable`；生产接线和 Wine 同符号/同时间 `2704` callback parity 尚未完成。
- 本轮仅修改进度记录，未触发重新编译或部署；下一次代码变更须继续通过 webClx 队列验证。

## 2026-09-02 2a10 摘要采样边界复核

- 使用 `scripts/compare-official-5188-subscriptions.py` 对 `formal-primary-0006/primary-5188-summary.json` 与 `formal-primary-121528/client-full-frames-v2/extract-summary.json` 做脱敏形状比较。
- 前者包含 7 条订阅、总计 6202 个条目；后者摘要字段为 0 条/0 个条目，因此工具报告 `all_match=false`。
- 该结果只能证明第二份摘要未采样订阅帧，不能证明跨会话分区规则改变；后续应直接基于完整客户端帧提取，再进行同口径比较。

## 2026-09-02 完整客户端帧复核：121528 订阅已恢复

- 直接扫描 `diagnostics/20260902-live-pair/formal-primary-121528/client-full-frames-v2` 恢复 7 条客户端 `0x102a`：6 条 payload 为 6154B、声明 1024 项；1 条 payload 为 364B、声明 59 项。
- 因此此前 `extract-summary.json` 的 0 条订阅是摘要生成缺口，不是该生命周期没有订阅；不能用摘要的 `all_match=false` 判定协议分区变化。
- 该复核仍未推出当前 slot assignment 或未知 10B prefix 字段的构造规则，生产 Native 门禁继续保持。

## 2026-09-02 121528 manifest 按连接关联复核

- 读取 `formal-primary-121528/client-full-frames-v2/manifest.json`：共 30 条 `2d10`、7 条 `2a10`、30 条 `3610`。
- 7 个包含 `2a10` 的 flow 均恰好包含 3 个 `2d10` 后接 1 个 `2a10`；其中 6 个订阅 payload 为 6154B/1024 项，1 个为 364B/59 项。
- 该结果确认初始化帧的连接内关联和动态小集合事实，但没有给出 10B prefix 未知字段或 slot assignment 的生成算法；Native 门禁不变。

## 2026-09-02 2a10 条目分区跨生命周期直接对齐

- 直接解析 `0006`、`121528` 和 `cold-sync-2311` 的客户端完整帧后，分区计数分别为 `6 x 1024 + 1 x 58`、`6 x 1024 + 1 x 59`、`6 x 1024 + 1 x 58`。
- 三个生命周期的六个大连接均保持相同的市场切换和连续区间形状；版本推进只造成索引起点的少量偏移。
- 小集合从 58 变为 59，确认订阅集合是运行时动态数据；该结果支持结构化 assignment 方向，但仍不足以构造未知 prefix 字段或解除 Native 门禁。

## 2026-09-02 2a10 prefix 跨生命周期稳定性

- 对 `0006`、`121528`、`cold-sync-2311` 共 21 条 `2a10` 直接帧比较 10B prefix：将 bytes `4..8` 的声明计数归零后，所有 prefix 完全一致。
- 已观测变化仅来自声明计数（1024、58、59）；其余 prefix 字段在三个独立生命周期保持稳定。
- 该结果是“模板加运行时计数”候选的强证据，但未知字段尚未完成协议语义确认；当前仍通过调用方提供 prefix，未写入生产静态常量。

## 2026-09-02 交接文档同步

- 已将四生命周期 `2a10` 严格审计结果同步到 `netzip_win/docs/glm5.3_native_driver_handoff.md`，纠正其旧的“两生命周期/58 项”描述。
- 交接文档现记录 prefix 归一化哈希、大分区拓扑一致性、小集合 `122/58/58/59` 和 `21/21` 计数校验；Native 门禁说明保持不变。

## 2026-09-02 订阅 prefix 脱敏分析工具

- 新增 `scripts/analyze-official-5188-subscription-prefix.py`，按完整客户端帧目录输出 `2a10` 数量、payload 长度、声明计数和归零计数后的 prefix SHA-256，不输出原始 payload 或条目。
- 对 `0006`、`121528`、`cold-sync-2311` 运行通过 `python3 -m py_compile` 和实际分析；三个目录均为 7 帧，归一化 prefix SHA-256 均为 `96eeff563b31...`。
- 结果分别复现 `358B/58`、`364B/59`、`6154B/1024` 形态，为后续 Native 动态构造提供可重复输入审计；生产门禁暂不改变。

## 2026-09-02 2a10 market-run 拓扑复核

- 对三个生命周期逐帧计算 market-run 数量和脱敏序列哈希：前六个 1024 项连接的 run 数量均为 `1, 1, 2, 5, 1, 1`，序列哈希一致。
- 第七个小集合保持单一 market run，仅条目数从 58 变为 59；这支持“固定大分区拓扑 + 动态小集合”的模型。
- 该模型仍未确定 prefix 语义和 slot 到分区的运行时指派，继续不接入 Native 生产发送路径。

## 2026-09-02 脱敏工具补充 market-run 拓扑输出

- `analyze-official-5188-subscription-prefix.py` 已扩展为输出每帧 `market_run_count` 和 `market_sequence_sha256`。
- 三个生命周期运行结果中，前六个 1024 项帧的 market sequence hash 全部一致；按文件排序的 run 数量为 `1,1,2,1,1,5`，第七帧均为单一 market run。
- 工具仍只读取离线完整帧并输出哈希/计数；未改变 Native 生产门禁。

## 2026-09-02 拓扑签名多重集修复

- 修正分析工具的 `topology_signature`：由集合改为排序列表，保留相同拓扑分区的重复项，避免丢失两个同形 1024 项连接。
- 三个生命周期重新运行后均输出 7 个分区；前六个 1024 项拓扑多重集一致，第七项仅计数为 58/59 差异。
- `python3 -m py_compile`、实际分析和进度文件 `git diff --check` 均通过。

## 2026-09-02 订阅形状自动比较基线

- 使用脱敏工具输出执行自动布尔比较：`0006`、`121528`、`cold-sync-2311` 三个目录均 `large_shape_match=true`、`prefix_match=true`。
- 比较忽略小集合的动态条目数，但保留大分区拓扑多重性和 market sequence hash；该结果作为后续 Native 构造变更的离线回归基线。

## 2026-09-02 订阅声明计数一致性校验

- 分析工具新增 `declared_count_matches_entries` 字段。
- 三个生命周期共 21 条 `2a10` 帧全部通过声明计数校验（`21/21=true`）。
- 该结果只确认帧内部计数结构，不改变 prefix 语义或 Native 生产门禁。

## 2026-09-02 订阅形状工具严格模式

- 工具新增 `--strict`：无有效 `2a10` 帧或声明计数不一致时返回非零。
- 对 `0006`、`121528`、`cold-sync-2311` 三个完整帧目录运行严格模式通过；每个目录 7 帧，计数集合分别为 `58/1024`、`59/1024`、`58/1024`，归一化 prefix 哈希一致。

## 2026-09-02 双向 progress.MD 一致性审计

- 核对本文件与 `/home/codes/stock/netzip_win/progress.MD`，最近五类订阅证据条目均存在且各出现一次：prefix 脱敏、market-run 拓扑、拓扑多重集修复、自动比较基线、声明计数校验。
- 未发现一侧已记录而另一侧缺失的新增结论；后续协议变更继续要求双向同步。

## 2026-09-02 四生命周期订阅拓扑回归

- 严格工具已覆盖全部现有含 `2a10` 的完整客户端目录，新增 2026-08-31 生命周期。
- 四个生命周期的归一化 prefix 哈希、六个 1024 项大分区拓扑多重集和 market sequence hash 全部一致。
- 小集合条目数为 `122、58、58、59`，确认是动态集合；该证据仍不足以解除 Native 初始化门禁。

## 2026-09-02 动态小集合内容关系复核

- 对四个生命周期第七条 `2a10` 的脱敏 `(market,index)` 集合做集合关系比较：两个 58 项生命周期完全相同（交集 58）。
- 59 项生命周期与 58 项基线交集 55，表现为删除 3 项、增加 4 项；122 项生命周期与 58 项基线交集 56。
- 小集合不是简单追加计数，必须由当前会话/业务状态提供完整 assignment；Native 不采用历史集合模板。

## 2026-09-02 121528 订阅与 2704 flow 关联

- 对齐客户端和服务端完整 manifest：六个 1024 项订阅 flow 各出现 3 条服务端 `2704`，59 项小集合 flow 出现 1 条 `2704`。
- 该正式生命周期再次证明 `2704` 帧数是变化记录/批次结果，不等于 `2a10` 订阅条目总量；不能用它反推订阅 assignment。

## 2026-09-02 121528 2704 payload 长度审计

- 按同 flow 对齐服务端 manifest：5 个 6154B 订阅 flow 明确各有 3 条 `2704`，长度分布落在约 5071--5111B；59 项（364B）flow 有 1 条 `2704`，长度 1941B。
- 另有 1 个 6154B flow 在当前服务端分片摘要中未观察到 `2704`，标记为采样/flow 对齐缺口，不视为零数据或 parity 通过。
- 后续 callback parity 必须覆盖 5071--5111B 与 1941B 变体，并补齐缺失 flow 的完整抓包关联。

## 2026-09-02 2704 证据范围审计

- 当前仓库仅 `formal-primary-121528` 保存按 flow 对齐的服务端完整 manifest；其他生命周期目前只有客户端完整帧和/或派生摘要，不能据此补写 2704 计数。
- 该范围限制已登记，后续跨生命周期 2704 parity 必须先生成同样的服务端完整帧 manifest。

## 2026-09-02 2704 flow 关联审计工具

- 新增 `scripts/audit-official-5188-2704-association.py`，从客户端/服务端 manifest 输出脱敏订阅长度、`2704` 数量和长度分布。
- 在 `formal-primary-121528` 运行通过：7 个订阅 flow，分布 `{0:1, 3:5, 1:1}`，未匹配 flow 1 个，与手工统计一致。
- 工具不读取或输出原始 payload，可作为后续完整抓包窗口的统一关联检查。

## 2026-09-02 Wine callback parity 证据范围

- `formal-primary-121528/wine-status-post.json` 仅提供 gateway 总量：12 批、14517 条报价、9/9 HTTP 成功、0 丢弃；该文件没有逐 callback 时间/证券映射。
- 现有 `formal-primary-0006` 与 `cold-sync-2311` callback-window index 均仍为 `needs-verification`，不能用总量统计替代同符号同时间 parity。
- 下一步需为完整服务端 manifest 对应窗口生成 callback-window index，再比较 2704 解码值与 Wine callback 字段。

## 2026-09-02 Wine callback JSONL 事件级审计

- 读取两个已有 callback JSONL：`formal-primary-0006` 有 17 条 `股票数据` 回调，`cold-sync-2311` 有 20 条；两者每条股票回调均带非零 packet。
- 纠正初步判断：股票回调含结构化 `quote_batch.quotes`，每条报价已有 `market/code/datetime/price/open/high/low/volume/amount`、五档价量和 `source_protocol`，可作为字段 parity 的 Wine 权威侧输入。
- 当前真正缺口是把 `2704` 解码结果产出相同的市场/代码/业务时间字段并完成窗口关联，而不是 Wine JSONL 缺少证券键。

## 2026-09-02 已完成的 2704/Wine 字段 parity 回放同步

- 权威任务文档和 `/tmp/official-5188-callback-parity-080949.json` 证明字段比较已实际运行，不应再描述为“尚未开始”：`decoded_records=2102`、`metadata_resolved=2102`、`matched_records=1792`、`unmatched_records=310`、无缺失元数据或非法记录。
- 250ms 窗口比较序列 34/35/36；1792 条匹配中的字段命中为：name 1792、price 355、last_close 1380、open 441、high 411、low 387、volume 377、amount 83、ask/bid prices 333/321、ask/bid volumes 383/366、timestamp 974。
- `to_public_quote_with_public_state` 的显式 ladder copyback 实验在 574 条同窗记录上把盘口命中从 236 提升到 705且无回退，证明剩余重点是 `0x44aa30` public-state 选择/合并，而不是市场/代码字段模型缺失。
- 生产门禁仍保持：amount 已知存在 OEM 转换差异，收盘时间允许的 `+1s` 差异已登记；其余字段、public-state 选择、Native 在线生命周期和重连尚未完成验收。

## 2026-09-02 协作审计纠错：五个主分区而非六个固定分区

- 并发审计将 formal-primary-121528 同 flow 的 `0104` 与 `2a10` 逐项对齐：前五个 1024 项分区共 5120 条，精确等于当前代码表 eligible 序列前 5120 条（`5120/5120`）。
- eligible 顺序为 SH 六位代码 `600000..699999`，随后 SZ 六位代码前缀 `000/001/002/003/300/301`；订阅索引为 `0x10000 | 0104 u16 ordinal`，按 1024 分块。
- 第六个 1024 项是跨 SH/SZ 且大量跳号的动态混合集合，第七个 122/58/59 项也是动态集合。因此此前“六个固定大分区”表述被否定，正确模型为“五个代码表主分区 + 一个动态 1024 混合集合 + 一个动态小集合”。
- 已同步修正 `netzip_win/docs/glm5.3_native_driver_handoff.md` 的过时 ACK、2d10 和 2a10 状态；生产门禁保持。

## 2026-09-02 zcode 本机重编译与启动验证

- 使用本机 MSVC 14.50.35717、Windows SDK 10.0.26100.0 和 x86 linker wrapper 完成 `i686-pc-windows-msvc` release 构建。
- 标准 `Z:/stock/netzip_win/target` 因 Windows `os error 5` 拒绝写入 `.rmeta`，改用隔离目录 `C:/Users/Administrator/ZCodeProject/target-netzip-launch`；未删除或覆盖共享 target，也未停止官方网际风。
- `cargo fmt --all -- --check`、workspace tests（core 4、drivers 7 passed/1 live ignored、native 32、vendor 5）、严格 Clippy 和 release build 全部通过。
- 新产物为 996352B 的 PE32/i386 GUI。已启动并确认 PID 34896 持续响应，窗口标题“飞狐交易师 V3.62 - Rust兼容接口”，68 个主要控件正常创建。
- 本轮只做启动和 UI 初始化验证：默认测试账号为 168，原生驱动仍显示“尚未认证”，未读取密码、未点击连接、未发起联网，也未改变 `InitializationUnavailable` 生产门禁。

## 2026-09-02 五个主订阅分区权威构造器

- 权威 crate 新增 `build_official_5188_primary_subscription_partitions`：校验 SH/SZ 代码表身份，按当前 `0104` 顺序筛选 SH `600000..699999` 和 SZ `000/001/002/003/300/301` 六位代码，将索引编码为 `0x10000 | symbol_index`，只取前 5120 项并切成五个 1024 项分区。
- 新增公开 entry/partition 类型及分区数量、长度常量；边界测试覆盖 SH→SZ 跨分区顺序、索引编码、畸形/非 eligible 代码过滤、市场身份错误和 5119 项不足拒绝。
- API 明确不生成第六个动态 1024 混合集合与第七个动态小集合；Native `InitializationUnavailable` 门禁保持。webClx 请求 `132228-18d1330bb455c59e` 已通过：权威 crate `148 passed / 1 ignored`，4 个新增分区测试通过，strict Clippy 和 workspace release 均通过。

## 2026-09-02 zcode 认证配置路径与动态登录修复

- 根因确认：GUI 主配置通过绝对 fallback 加载 `D:\\Soft\\_Stock\\飞狐2020\\用户\\配置文件.ini`，旧 Native 却从隔离 exe 目录寻找 `服务器列表.ini`，因此有效的 4 个认证节点被误报为空。
- Native 启动现在从已选中的 `state.ini_path.parent()` 加载同目录服务器列表，并将对应飞狐根目录传给 `DriverConfig`；读取失败或无有效节点时直接显示具体路径/错误，不再静默变成空候选。
- 修复真实登录 manifest 的错误固定 854B 门禁。19 字段对象长度会随 UTF-16 账号/密码变化，现仅校验运行时长度字段自洽；新增 `168/168` 动态长度回归测试。
- GUI worker 启动事件不再提前显示“原生驱动已连接”，改为“正在认证”，只有真实 Connected 事件才可进入连接状态。
- 使用 MSVC 14.50.35717、Windows SDK 10.0.26100.0、x86 linker wrapper，在隔离 target 完成 152 项测试通过（1 项 live ignored）、严格 Clippy 和 i686 release 构建。
- 启动最终版本并以 `168` 点击连接验证：已越过认证配置为空和 854B drift，成功进入认证后官方 5188 路由；随后连接 `222.85.139.177:5188` 因服务端/网络超时 10060 失败，未回退 DLL/7709，GUI 正确保持“正在认证”。
- 当前最终进程为本次构建的 `netzip_win.exe`，PID 36712；官方网际风未停止、未覆盖。

## 2026-09-02 当前连接完整 0104 表接线

- `formal-primary-121528` 同一 5188 flow 的四个 `0104` 解压对象均按 98B 头 + 68B 记录闭合：SH 26485、SZ 4600、B$ 790、SF 742 条；这证明交错初始化可直接保留完整表，不必由 Native 二次解析 raw frame。
- `Official5188InitialCodeTables` 现保留四张已校验 `Official5188CodeTable` 及原始 observed frame 顺序，并提供 `primary_subscription_partitions()` 唯一选择当前 SH/SZ 表后调用五分区构造器；重复/缺失市场拒绝。
- webClx 请求 `132631-18d1330bb455c59f` 发现测试 fixture 仍为 92B 短头，生产解析未放宽；fixture 已修为完整 98B 且保留路由 marker=1。重验请求 `132745-18d1330bb455c5a1` 已通过：权威 crate `149 passed / 1 ignored`，完整表收集和当前连接分区入口测试通过，strict Clippy 与 workspace release 全部通过；两个动态集合和 Native 写入门禁均未改变。

## 2026-09-02 动态订阅来源强候选

- 对 `formal-primary-121528` 同时段 Wine 配置脱敏核对：`用户/只接收股票代码表.csv` 含 6955 个 SH/SZ 代码，七条 `2a10` 共 6203 条（6×1024+59）逐条命中该清单，命中率 `6203/6203`；“全部股票”配置也覆盖全部条目，区分力不足。
- 前五个 1024 分区仍是当前 `0104` eligible 序列；第六个 1024 为混合分类顺序（约 446 条 SZ 301xxx 后接约 578 条 SH/SZ ETF/指数/B 股），第七个 59 条为动态 ETF/B 股集合，均不像单一连续 ordinal 切片。
- 当前最强候选是：运行时“只接收”清单经 vendor 分类/排序后映射到当前 `0104` symbol ordinal，再生成第六、第七分区。尚无代码接线；必须用第三个独立冷启动并增删一个接收代码的因果实验验证，Native 门禁保持。

> 更正索引：本文件早期“六个 1024 flow 各有 3 条 2704”（约第 1570 行）已被同段后续 manifest 结果否定；正确观察是 5 个 6154B flow 各 3 条、1 个 6154B flow 未匹配、动态小集合 1 条。早期“六个固定大分区”和“至少 92B 头”也已被后续“五主分区 + 两动态集合”和完整 98B 表证据 supersede。

## 2026-09-02 动态集合证据补强与回归修复

- 脱敏反解确认 `formal-primary-121528` 七个 `2a10` 共 6203 条与同窗 Wine `用户/只接收股票代码表.csv` 的集合命中为 `6203/6203`；第六分区为 578 SH + 446 SZ 混合集合，第七分区为 59 条 SZ ETF/B 股集合。
- 该证据支持“只接收清单筛选后按内部分类/排序映射当前 0104 ordinal”的候选，但 CSV 行号不能直接作为 wire index；仍需增删单个 ETF/B 股的第三独立冷启动实验确认因果和分区边界，未接入 Native。
- webClx `133242-18d1330bb455c5a2` 暴露冗余测试导入；该状态已由后续 `134014-18d1330bb455c5a6` 成功回归 supersede，不改变动态集合和生产门禁。

## 2026-09-02 prefix builder 回归验证

- webClx `134014-18d1330bb455c5a6` 已通过：权威 crate `152 passed / 1 ignored`，动态 prefix/current entries 测试、完整 0104 表与五主分区回归通过；workspace 测试、strict Clippy 全绿。
- 该请求先因测试显式导入冗余（`133849`/`3146`）失败，删除导入后验证通过；生产代码无协议放宽。

## 2026-09-02 Native 状态边界纠正

- 代码审计确认 `netzip-drivers/src/native.rs` 当前实际是认证后单个 5188 socket 的 receive-only shadow：它不会构造 `InitializationUnavailable` 枚举事件，而是在未初始化时发出说明性 Notice，随后对端关闭或 stop 后报告 `Disconnected`。
- 因此此前把 Native 描述为“报告 InitializationUnavailable”仅是门禁语义简写，现以实现事实为准；生产仍未接入每 slot 交错初始化、动态第六/第七集合、重连和 callback parity。
- 下一项最小实验：备份 Wine `用户/只接收股票代码表.csv`，仅增删一个 ETF/B 股后冷启动，比较第六/第七 `2a10` 差集及对应 `2704` flow；未完成前不解除生产门禁。

## 2026-09-02 只接收清单来源边界

- 运行目录中的 `只接收股票代码表.csv` 是 vendor UTF-16LE TAB 工作表（含配置头/启用标记），不是普通 CSV 接口；不能把文件行号误当作 `0104` symbol ordinal，也未将其内容复制进 Rust 或生产配置。
- 当前可复现证据来自脱敏后的代码集合与同会话 `0104` 映射：6203/6203 命中，只能证明集合归属候选。下一步需通过 Wine UI 或等价配置操作做单项增删并重新冷启动抓包，记录第六/第七分区差集和对应 flow。

## 2026-09-02 当前协作检查

- 重新检查共享目录后，当前仍只有四个既有生命周期（2026-08-31、cold-sync-2311、formal-primary-0006、formal-primary-121528），没有新的“增删接收代码后冷启动”抓包窗口。
- 因此第六/第七分区的接收清单来源仍是强候选而非已证实因果规则；不新增静态模板、不接入 Native 生产发送路径。下一次实验必须同时保存配置变更前后脱敏集合摘要、同一连接 `0104` ordinal 映射、`2a10` 差集和 `2704` flow 关联。

## 2026-09-02 接收清单文档边界修正

- `docs/netzip-wine-replication-audit.md` 已将“只接收 CSV”改为 vendor 接收清单观测来源，并明确运行目录文件是二进制/序列化格式；补数接口对 CSV 的解析描述不代表 5188 wire-index 来源。
- 5188 证据继续要求先做同连接 `0104` 代码映射；第六/第七排序、slot 指派及增删配置因果实验仍未闭环。

## 2026-09-02 接收清单脱敏审计工具

- 新增 `scripts/audit-official-5188-receive-list.py`：输入调用方导出的脱敏 `0104`（market/ordinal/code）映射、文本接收清单和完整 `2a10` 帧目录，输出每分区条数、映射缺失数、接收清单命中数和条目哈希。
- 工具不解析 vendor 二进制接收文件、不输出原始订阅条目或凭据；`python3 -m py_compile` 与 `git diff --check` 已通过。该工具用于下一次配置增删冷启动的可重复验收，未改变 Native 门禁。
- 以临时脱敏映射、接收集合和两条合成 `2a10` 条目运行端到端自测通过（2/2 命中、0 映射缺失）；未读取或写入真实凭据/配置。

## 2026-09-02 生产门禁文档校准

- `docs/codex/tasks/official-5188-production-wiring.md` 已将已闭环的当前会话 `2d10` 派生从阻塞项移除；当前阻塞项准确列为 Native per-slot 接线、动态 `2a10` 集合 6/7 与 assignment、重连及 2704/Wine callback parity。

## 2026-09-02 open-market 重启窗口负证据

- `diagnostics/20260902-live-pair/open-market-132540/` 保留约 927MiB 全端口抓包，但 Wine 重启部署请求 `133434-18d1330bb455c5a4` 未通过健康门禁；前置失败包括 PE32 `ReplaceFileW` sharing violation，绕过后又出现 `Stock DLL Start(callback) returned 0`。
- Wine 本地代理端口曾监听但 adapter 未健康；该窗口的 `pcap-5188-summary` 为 0 条 5188 flow，回调文件仅含 200 条轮询端点 `股票数据` 事件。因此它是启动顺序负证据，不能用于 5188/Wine parity，也不能证明协议或解码失败。
- 下一次在线取证必须先证明 `Start(callback)` 成功并保存完整 callback cursor，再进行 5188 帧关联。

## 2026-09-02 接收清单格式审计补充（config_format_research）

- 复核 Wine 运行文件 `用户/只接收股票代码表.csv`：304726 字节，可完整按 UTF-16LE 解码，约 6974 行、6955 条启用记录；前部包含配置头与限制项。
- 共享 `netzip-supplement::parse_wine_code_worklist` 已是该文件的唯一解析器（UTF-16LE、TAB 字段、启用标记、代码校验与去重），补数侧回归通过。
- 该文件的内容形态与“vendor 二进制/序列化”旧描述不一致：路径后缀虽为 csv，但实际是 vendor UTF-16LE TAB 工作表。该更正仅影响取证文档，不改变 5188 结论。
- 明确边界：工作表 ordinal/行号不能当作 0104 wire symbol ordinal；必须先按同连接 0104 代码表映射。未将工作表内容复制进 Rust，也未解除 Native 初始化门禁。

## 2026-09-02 动态集合实验状态复核（dynamic_set_source）

- `formal-primary-121528` 的 6203 条 2a10 订阅仍全部命中同窗“只接收”集合；第六/第七分区来源候选保持高可信，但尚无增删单项后的因果冷启动证据。
- 下一项最小实验仍为：备份工作表，单独增删一个 ETF/B 股，冷启动并保存变更前后脱敏集合、同连接 0104 ordinal 映射、2a10 差集及 2704 flow 关联；在实验完成前不把第六/第七集合接入生产构造器。

## 2026-09-02 生产接线状态复核（3147）

- webClx 请求 `134014-18d1330bb455c5a6` / `3147_build.log` 已完成：动态 prefix、完整 0104 收集、当前会话 2d10 派生和五个主 2a10 分区回归通过；未新增在线证据。
- 权威库能力不等于 Native 生产链：`netzip-drivers` 仍未把交错初始化、动态第六/第七集合和 slot supervisor 接入真实 worker；服务发布入口仍由 7709 补数路径承载。
- 本轮没有发现新的 Wine 冷启动或接收清单增删实验；因此不得更新为“5188 full-push 已上线”，也不得移除初始化门禁。下一步仍是因果冷启动和同符号/同时间 2704 callback parity。

## 2026-09-02 持续协作核对

- 重新检查两份进度文件及当前工作树，未发现新的正式 Wine 生命周期、接收清单增删实验或新的 5188 在线回调证据。
- 代码侧已确认：权威库提供 `initialize_official_5188_interleaved_with_code_tables`、当前会话 `2d10` triplet 和五主分区 builder；`netzip-drivers/src/native.rs` 仍未调用该入口，服务发布代码仍直接创建 `Tdx7709Session`。
- 因此当前状态是“协议库能力已具备、生产接线未完成”；本轮不触发部署，不改变 Native 门禁。下一次有效推进必须产生新在线证据或接入并验证 Native per-slot worker。

## 2026-09-02 Native 显式开关初始化接线

- `netzip-drivers/src/native.rs` 现已接入 `Auth7100ControlSession::initialize_official_5188_interleaved_with_code_tables`：当前登录生命周期内完成 `3610→3110、ABK→3110、ACK→3210、四张 0104`，再从同连接代码表动态生成 `2d10×3` 与五个 1024 项主 `2a10` 分区。
- 初始化路径由 `NETZIP_NATIVE_5188_INIT=1` 显式启用；默认仍保持登录后单个 5188 socket 的 receive-only shadow。动态第六/第七集合、slot assignment 和 2704 callback 发布仍未接入，生产 full-push 门禁不解除。
- ACK 请求基于本地 runtime 文件 CRC 与九行结构性市场状态生成；`AuthError::new` 已改为权威库公共构造器。该路径仍未在线验证，编译通过不等于官方 full-push 成功。
- webClx `170422-18d1330bb455c5b8` / `3156_build.log` 通过 netzip_win workspace：fmt、tests（52 passed/1 live ignored）、strict Clippy、release build 全绿；此前 `170211`（3155）修复 Subscription 错误映射也通过。

## 2026-09-02 Native 开关脚手架边界校准与 3157 验证

- webClx `170653-18d1330bb455c5b9` / `3157_build.log` 通过 netzip_win workspace（fmt、tests、strict Clippy、release 全绿）。改动：ACK 清单文件根由写死的 `D:/Soft/_Stock/飞狐2020` 改为 `DriverConfig.runtime_dir`；ABK 构造值改为已验证 vendor 客户端构建值。
- 明确边界：`NETZIP_NATIVE_5188_INIT=1` 是诊断脚手架而非成功路径——登录阶段 `3610` 需当前会话字段值（本地IP/账号权限/券商），零值会得 94B 而登录要求 95B；且保留控制 socket 上 下载/L1 已消耗编号 1、2，每连接登录请求编号尚未独立验证。生产 full-push 门禁保持关闭。
- open-market-132540 全端口 SLL2 抓包新审计：2,238,689 包中仅 2 条 2 包 5188 flow（远程:17664→本机:5188），无任何官方 5188 客户端流量，与已知 Wine 未健康启动的负证据一致。

## 2026-09-02 L1 路由运行时字段供给（消除登录阶段零值）

- 权威 `netzip-fullpull`：`DownloadedServerEntry` 新增 `broker`/`permission`/`interface_version`；`parse_l1_server_entry` 现保留 `大智慧服务器L1.ini` 七列中的券商、账号权限与接口版本（此前被丢弃）。
- 新增 `Auth7100ControlSession::current_login_control_fields(local_ip)`：从保留登录结果的 L1 路由条目供给登录控制字段，无匹配/字段超宽返回 `None`（fail-closed），不落回抓包静态默认值。
- netzip_win `native.rs` opt-in 路径改为用该运行时字段（本地 IP 由 UDP 探测）；无 L1 元数据时明确失败，不再发送零值 94B 登录阶段请求（登录门槛 95B）。
- 3 个新回归测试：L1 行保留字段、selected-L1 供字段、无元数据 fail-closed。
- webClx `192720-18d1330bb455c5c4` / `3163_build.log`：fullpull `156 tests`（155 passed/1 live ignored）、netzip_win workspace、strict Clippy、release build 全绿。未部署，Native 动态第六/第七集合与生产 full-push 门禁保持。

## 2026-09-02 L1 运行时字段→登录控制包端到端回归

- 权威 fullpull 新增 `runtime_l1_fields_flow_into_the_login_control_packet`：从保留登录结果的 L1 路由条目派生券商/权限/接口版本，构造 448B 登录控制包，断言解码后字段值（华创/点播版/858）与请求编号 2，且券商非零默认。
- webClx `195131-18d1330bb455c5cb` / `3165_build.log`：fullpull 157 tests（156 passed/1 live ignored）、netzip_win workspace、strict Clippy、release 全绿。
- 至此当前会话登录阶段请求的全部可变字段（券商、权限、接口版本）已从运行时 L1 下载结果供给，不再依赖抓包静态值；本地 IP 由 Native UDP 探测。仍未在线验证；动态第六/第七集合、编号复用、per-slot 与生产 full-push 门禁保持。

## 2026-09-02 20:00 协作观察：并行 Wine 取证新窗口序列

- 19:31–19:54 出现并行 Wine 取证目录：ask-diagnose-1931、init-none-1938、init-none-injected-1945、init-none-primary1-1950、init-none-primary1-1952（1952 于 19:54 仍在 150s hold 抓包，属另一活动 Codex 会话 PID 2078675；本会话不干扰）。
- init-none-1938（已收尾，17MB）初核为 **备用主站完整生命周期**（58.16.134.228:5188，login_backup=true）：12+ 条客户端 5188 连接，每条 3610×3 → 3110×2 → 3210×1 → 0104×4；约 7 条连接发 2a10（含 6154B=1024 项大分区）；多流出现 2704/3e04/1504 等业务帧。与前四个既有生命周期同构。
- init-none-injected-1945 仅含 7100 认证流（2 次 582B 请求/326B 响应，无 5188），疑似注入尝试未建立数据链。
- 待 1952 收尾后：确认是否出现 login_primary=true 的真实主站窗口；若出现则登记为第 5+ 个独立生命周期候选，用于跨生命周期 2a10 分区一致性验证。当前不新增静态模板、不改 Native 门禁。

## 2026-09-02 20:00 init-none-primary1-1952：login_primary=true 真实主站新生命周期

- 1952 窗口（58.16.134.228:5188，19:54-19:58）有效配置 **login_primary=true/login_backup=false**，事件含「股票主站登录成功」→ 这是首个被确认的真实主站登录 5188 窗口（此前 formal-primary 均 login_backup）。
- 7 条客户端 2a10 结构 = **6×1024(6154B) + 59(364B)**，与 formal-primary-121528 完全同构：分区1-2 纯 SH 连续、分区3 SH268→SZ756 跨市场、分区4-5 纯 SZ、分区6 SZ446→SH578 混合、分区7 SZ59。每条大分区流均有服务端 2704（58526/534/540/542/550/564），小分区流 58580 有 1 条 2704，58588/58604/58610 仅心跳/3e04/3f04 无 2a10。
- 这是第五个独立生命周期且为真实主站；为“五主分区+动态集合”跨生命周期一致性提供又一直接证据。仍在抓包窗口覆盖后期（150s hold），未捕获 0104→2d10 头部；如需完整初始化链需更长窗口。
- 无新静态模板、Native 门禁不变；供并行 Wine 取证会话参考，不替代其自身记录。

## 2026-09-02 20:05 跨登录类型强一致：1938(备用)=1952(主站) 2a10 逐条同构

- 对 init-none-1938（login_backup，58.16.134.228）7 条 2a10 解码：6×1024(6154B)+59(364B)=6203 条，分区结构 = **1952 主站窗口逐条一致**（含每条起始/结束 symbol 索引）：P1 SH 89197-90220、P2 SH 90221-91244、P3 SH91245→SZ66291(268+756)、P4 SZ 66292-68627、P5 SZ 68628-69651、P6 SZ69652→SH67909(446+578)、P7 SZ 67910-68341(59)。
- 同一服务器 58.16.134.228 上，backup 与 primary 两次登录产生**完全相同的订阅指派**；结合 formal-primary-121528 同为 6×1024+59，跨三个生命周期（含两种登录类型）的“五主分区+动态混合+动态小集合”模型获得一致支持。
- 该结论只读、无新静态模板、不解除 Native 门禁；供并行 Wine 取证会话与 0104→2a10 对齐分析引用。

## 2026-09-02 20:10 同步并行会话决定性结论：Wine 主站掉线根因=旧式 Ask(模块=认证)

- 并行取证（EXPERIENCE.md「Wine 主站掉线根因与修复（决定性）」）证实：quoteNetzipWine 默认发送的旧式同步 `Ask(请求=登录&模块=认证&编号=0)` 与网际风配置自动登录冲突，Ask 返回 0 后 vendor 断开股票主站、切备用并写回 `登录股票主站=0`——这是多轮 ≤60s 抓包窗口掩盖的掉线规律（formal-primary-0006 ~65s 本地 FIN+RST 同因）。
- 判别证据：ask-diagnose-1931（旧式 Ask）主站 24ms 后断开；init-none-primary1-1952（`登录股票主站=1` + `QUOTENETZIPWINE_INIT_LOGIN_MODULES=none`）10 条 58.16.134.228:5188 保持 ESTAB 150–237s 零断开，effective login_primary=true。
- 修复：`initialize()` 默认只发只读初始化 Ask，主站/备用登录交由网际风按配置文件自动登录；`run-host-wine.sh` 白名单传递变量。
- 含义（供 Native 集成参考）：官方主站 5188 会话依赖配置文件 `登录股票主站=1` 的稳定生命周期；Rust Native 自主 19 字段登录（非旧式 Ask）不受该 Ask 冲突影响，但启用 NETZIP_NATIVE_5188_INIT 在线验证时应使用 login_primary=true 健康会话作基准。fixed-accept-2000（并行会话 20:00 起 300s）待收尾后跟进。

## 2026-09-02 20:12 fixed-accept-2000 收尾：login_primary=true 健康主站长窗口（第 3 个同构）

- fixed-accept-2000（并行会话 20:00 起，5188-only 抓包至 20:08）收尾：effective login_primary=true/login_backup=false，事件「股票主站登录成功」(20:00:45) 后保持 ≥7 分钟，gateway 16 batches/12374 quotes/15 posts 成功 0 掉。
- 该窗口从登录前起抓，含完整初始化链：client 每连接 3610×3/2d10×3，7×2a10；server 每连接 0104×4/3110×2/3210×1 + 业务帧（2704×38、3901×360、3e04×136、1504×60、3f04×65、0d04×20 等）。
- 7 条 2a10 解码 = 6×1024+59=6203，P1–P7 起止 symbol index 与 1938(backup)/1952(primary) **逐条一致**。跨生命周期（formal-primary-121528 + 1938 + 1952 + fixed-accept-2000 = 四个独立窗口、backup 与 primary 两种登录）订阅指派完全一致。
- 取证笔记已更新至 `docs/forensics/cross-login-type-subscription-identity-20260902.md`；wiring 任务文档已加「在线接线前置 checklist」。只读、无静态模板、Native 门禁不变。

## 2026-09-02 20:15 决定性：0104→2a10 ordinal 对齐验证（fixed-accept-2000）

- 解码 fixed-accept-2000 连接 60844 的四张服务端 0104 zlib 壳（SH 26485/SZ 4600/B$ 790/SF 742，98B 头+68B 记录），与同连接登录后 7 条 2a10 逐条对齐。
- **P1–P5 = 0104 eligible 序列（SH 600000–699999 + SZ 000/001/002/003/300/301 六位）前 5120 条，逐条相等**（0 mismatch）；2a10 index = 0x10000|0104 ordinal。实际代码印证 P1 600000 起→SH 688→SZ 002/300/301→301515。
- P6 = SZ 301516 续接 + SH 159xxx ETF 混合，P7 = SZ 159968+200xxx B 股 59 条动态小集合（排序来源仍 needs-verification）。
- 第 4 个独立生命周期（login_primary=true 健康主站）上直接 ordinal 对齐复验，与 121528 的 5120/5120 互证。笔记更新至 cross-login-type-subscription-identity-20260902.md。只读、无静态模板、Native 门禁不变。

## 2026-09-02 20:18 2d10 group 派生交叉验证（fixed-accept-2000）

- fixed-accept-2000 10 条连接：每条 SH/SZ/B$ 三张 0104 header group 一致，且与该连接 2d10 word1 低16位逐流相等（5×0xb246 Secondary、5×0x2746 Primary）。
- 证实权威 crate `Official5188CodeTableHeader::client_session_envelope` 派生（word1 = trading_day_low<<16 | group）在第 4 个生命周期（login_primary=true）成立；word0=市场+0x010a、word2 low=0x0135、version 高半跨连接恒定均一致。
- 笔记更新至 cross-login-type-subscription-identity-20260902.md；只读、无静态模板、Native 门禁不变。

## 2026-09-02 20:16 P6/P7 精确组成解析（fixed-accept-2000）

- P6 = SZ 446 + SH 578：SZ ordinal 1494–4265（仅 94 条为 eligible SZ301 截断尾 301516–301717，主体是 159xxx ETF 243 条 + 399/123/127/128/302 等）；SH ordinal 0–26484 全跨度跳号（000 上证指数/51x-56x 沪 ETF/900 B 股/113 转债，516/515/512/510/513 前缀为主）。
- P7 = SZ 59 纯动态小集合。
- 结论：P6/P7 不是 eligible 截断续接，而是「主前缀之外证券 + 尾部」按接收清单/分类挑选的混合集合；精确排序仍 needs-verification，需单项增删因果实验。笔记已更新。

## 2026-09-02 20:24 closing-parity-2020 观察（并行会话，已收尾）

- closing-parity-2020（20:17–20:22，180s）5188-only pcap 仅 106K：服务端 290 条帧全部为 `3901` 心跳，无 2a10/2704/3e04 等业务帧；客户端 0 帧。callbacks full.jsonl 10105 事件、2880 带 quote_batch（盘后轮询）。
- 判定：该窗口为 login_primary=true 稳定连接的**盘后保持期**（仅 100s 心跳），无增量行情；作为「收盘 parity」输入无 2704 可比对，仅证明主站连接在盘后稳定保持。不产生协议新结论。
- 并行会话未活跃子进程、尚未写结论；不干扰。fixed-accept-2000/1938/1952 仍为最近有效业务窗口。

## 2026-09-02 20:26 可复用分区对齐审计脚本

- 新增 `scripts/audit-official-5188-partition-alignment.py`：输入 extract_5188_payloads.py 输出的服务端/客户端帧目录，解码四张 0104 zlib 壳 + 7 条 2a10，输出表规模、eligible 总数/市场分布、P1–P5 是否=eligible 前缀 5120、P6/P7 大小与市场构成。只输出 market/code/ordinal，不回显 raw payload。
- 已在两个完整链窗口复验：fixed-accept-2000 与 init-none-1938 均 `primary_partitions_match_eligible_prefix=true`（tables SH26485/SZ4600/B$790/SF742，eligible 5214，total 2a10 6203）。1952 pcap 为连接建立后 hold，无服务端 0104 帧，无法跑 0104 对齐（不构成反证）。
- `python3 -m py_compile` 与 `git diff --check` 通过；供未来完整链窗口直接复用做跨生命周期对齐回归。

## 2026-09-02 20:30 同步并行会话结论：收盘后主站保活=0x0139 心跳

- closing-parity-2020（并行会话分析）S2C 仅 290 帧全部 kind 0x0139（wire 3901），payload 恒 2B 00 00，~0.83s/条 = 纯 keep-alive 无业务。
- Wine 6187 只全市场 quote_batch 周期重放为本地 OEM 缓存，非网络增量，**不得当 5188 parity 真值**；业务 parity 只能开盘窗口抓。
- Rust 已补 `Official5188Kind::SERVER_HEARTBEAT(0x0139)` 常量+方向+测试（权威 crate），上层应忽略心跳帧。
- 与我的 closing-parity 只读观察一致（290×3901、无 2704/2a10）。无冲突；Native 门禁不变。

## 2026-09-02 20:40 并行会话：5 主分区 100% 验证 + 动态 6/7 精确定位（离线，固定-accept-2000）

- authority（20:40）确认：fixed-accept-2000 逐连接建模——60844/60860/60872/60882/60890 各发一个 1024 分区 = Rust 模型 partition 1..5 **逐字节完全一致**（SH/SZ 26485/4600，eligible 5214≥5120）。`build_official_5188_primary_subscription_partitions` 建模正确。
- 第 6 连接(60894)发 1024（SH578+SZ446，930/1024 非 primary，含 SH 000 指数等）→ 动态分区 6，模型未覆盖；第 7 连接(60904)发 59 SZ（159xxx 场内基金/ETF 连续段 ordinal~2374）→ 动态分区 7，模型未覆盖。
- 结论：Wine 一条连接一个 2a10 分区；5 主推分区已完全复刻。P6/P7 = 待重构动态补充订阅。待验证：P6 是否=剩余 non-primary 某 1024 切片、P7 是否=SZ 场内基金全集/切片；需另一 lifecycle 对照。
- 与我审计脚本独立结果一致（P1-P5 eq、P6 SZ446+SH578、P7 SZ59），且并行补充逐连接映射 + P6 930/1024 非 primary + P7 ordinal~2374 精确定位。无冲突。

## 2026-09-02 20:42 P6 结构段解析（补充）

- fixed-accept-2000 P6 = 1024 条 = 26 个段，段内 ordinal 连续按 0104 表序、段间按内部分类顺序：SZ 301尾94+302/123基金34/127+128/399/159基金243；SH 000指数50/110-118转债58/51x-56x ETF约390/588科创22/900 B股40。
- P6 仅 94 条 eligible（=SZ301 截断尾），主体是 non-primary 基金/指数/转债/B 股分类切片。段序非 ordinal 序（159 基金排末但 ordinal 低），提示为另一次分类排序/接收清单来源；仍需单项增删因果实验。

## 2026-09-02 20:45 vendor_pm 7100 编号序列决定性提取（阻塞 #1 关闭候选）

- 转换 vendor_pm.pcapng → classic pcap，重建 7100 客户端流（79 网络包、31 个带编号控制请求），完整序列：
  - 下载文件 系统\大智慧服务器L1.ini = **1**
  - 大智慧C_登录包 = **2..11**（10 个，对应 10 条连接）
  - 之后**每对连接**递增 4：ABK 12,13 / ACK 14,15；ABK 16,17 / ACK 18,19；…；ABK 28,29 / ACK 30,31（连接 9-10）
- 规则（由该序列唯一推出）：控制 socket 编号 = 1（L1）+ 每连接先 +1（登录包）再按序分配 ABK/ACK 成对递增 4。
- 这对 Rust 接线很关键：**10 连接时登录包用 2..11，编号 2 = 连接 1**；当前 Native 单连接 opt-in 应使用登录=2、ABK=12、ACK=14 与 vendor_pm 连接 1 一致（此前 12/14 猜测获证实）。
- 但本窗口 7100 流前无「通达信列表 7709」下载（#3 直接是 L1），说明 L1 是登录后唯一下载（与 connect_auth_sequence 一致）。仍保留「跨会话编号稳定性」为 needs-verification（vendor_lunch 未验）。

## 2026-09-02 20:50 vendor_pm 编号规则回归测试（webClx 3169）

- 权威 fullpull 新增 `vendor_pm_control_numbers_follow_per_pair_increment_four_rule`：L1 下载=1、登录包 2..11（每连接+1）、此后每 2 连接一组递增 4（ABK 12,13/ACK 14,15 … ABK 28,29/ACK 30,31），以确定性断言锁定（ABK 序列 = 12,13,16,17,20,21,24,25,28,29；ACK = +1 平移）。
- webClx `204533-18d1330bb455c5d1` / `3169_build.log`：fullpull 158 passed / 1 ignored、strict Clippy、release 全绿（此前 3168 因我断言公式错误失败一次，已修正）。
- 该规则确认单连接 Native opt-in 编号（登录=2、ABK=12、ACK=14）与 vendor_pm 连接 1 一致。仍未在线验证；跨会话编号稳定性保持 needs-verification。

## 2026-09-02 20:52 决定性：P6/P7 跨生命周期稳定（121528=accept2000 全 7 分区逐条相等）

- formal-primary-121528（12:15 交易时段登录）与 fixed-accept-2000（20:00 晚间登录）解码 2a10 后逐条比较：**P1–P7 全部逐条相等**（P6 1024 与 P7 59 亦同，市场+index 全同）。
- 结论：P6/P7 虽为 non-primary 分类切片，但在同账号/服务器 58.16.134.228 跨两个独立登录**完全稳定** = 确定性派生（0104 全表固定分类规则），非随机/每会话变化。先前「动态」仅指其在 eligible 前缀外。
- 待验证：换 broker/权限/接收清单时 P6/P7 是否变化（需增删因果实验）。笔记已更新。

## 2026-09-02 20:56 跨交易日 P6/P7 稳定性（09-01 cold-sync-2311 vs 09-02，决定性）

- 不同交易日代码表不同（SH 26461→26485/SZ 4597→4600；SH 增 30+ 删若干、SZ 增 123282/158027/301688 删 0）。
- P6/P7 序号随表平移 -2（09-01 首 69650 vs 09-02 69652）；按代码比 P6 前 89 条相同、分歧恰在新增 SZ 301688 插入后。
- 结论：P6/P7 = 对当前 0104 表的确定性分类派生（非随机）；按当前表+固定分类规则可跨日复刻。26 段分类枚举序是可离线确定部分。笔记已更新。

## 2026-09-02 21:00 决定性：接收清单 = 全部 7 分区订阅统一来源（6203/6203）

- 解析 只接收股票代码表.csv（6955 启用代码），fixed-accept-2000 7 条 2a10 逐条比对：**6203/6203 全部命中清单**（P1-P7 各 100%）。
- P6 SH 51x/56x ETF 段 408 条 0 条不在清单；清单内该分类 ~38 条未进 P6（1024 上限内 SH578 配额截断）。
- 结论升级：清单=订阅统一来源（P1-P5 eligible 前缀是清单覆盖主推证券的自然结果，P6/P7 是清单 non-primary 分类切片）。实现=当前 0104 映射清单后按分类切分区；清单行号≠ordinal 边界仍成立。笔记已更新。

## 2026-09-02 21:02 P6 26 段分类枚举序精确记录（accept2000）

- flow 60894 P6=1024：SZ301(94)→SZ302(1)→SH000(50)→SH110/111/113/118(58)→SZ123/127/128/399(108)→SH510-518+560-563(390)→SH588(22)→SH900(40)→SZ159(243)。段内 ordinal 连续、段间=分类枚举序（非代码数字序）。每段=接收清单∩0104 该分类，配额截断。

## 2026-09-02 21:04 强化：121528（12:15）订阅也 100% 命中 12:16 版清单

- 121528（12:15 独立登录）6203 订阅用其自有 0104 表解码后与 12:16 版只接收清单比对：**6203/6203 命中**（P1-P6 各 1024、P7 59 全中）。
- 说明清单在 12:15 前已如此（12:16 为无关/访问时间），同一清单贯穿 121528 与 accept2000 两个独立生命周期——清单为两者订阅统一来源获双生命周期证实。
- 与 fixed-accept-2000 的 6203/6203 交叉互证。只读、无代码改动。

## 2026-09-02 21:10 集合论精确刻画：订阅=清单∩0104（6203），P1-P5 完全复刻

- 清单 6955：清单∩0104 = 6203 = 订阅全集（752 清单码不在 0104，0 在 0104 未订阅）。
- P1-P5(5120)=交集 eligible 前 5120（5214 eligible 截 94）；P6+P7(1083)=交集剩余（989 non-eligible+94 eligible 尾）。集合层面确定。
- P6 以 SZ301 起，非清单文件序(SH 前) → 段级排序= vendor 26 段分类枚举（顺序已记录、规则推导开放）。集合可实现、顺序待在线验证是否影响 2704 收流。

## 2026-09-02 21:12 审计脚本新增 --receive-list 全集验证

- audit-official-5188-partition-alignment.py 新增 `--receive-list <只接收股票代码表.csv>`：per-flow 0104 解码后对全部 2a10 逐条验证「订阅 ∈ 清单∩0104」，输出 hits/not_in_receive/not_in_flow_0104/all_subscribed。
- accept2000 复验：6203 entries/6203 hits/0/0/all=True（复现手推结论）。2a10 兼容 raw payload 与 framed(0x102a+8B 头) 两种输入。py_compile+diff-check 通过。

## 2026-09-02 21:14 权威 crate 接收清单→7分区构造原语（webClx 3173）

- fullpull 新增 `Official5188ReceiveListPartitions` + `build_official_5188_receive_list_partitions(sh, sz, receive_codes)`：P1-P5 = receive∩0104 eligible 前 5120（逐字节已验证规则）；P6/P7 = 剩余 enabled 代码（集合已证实，顺序按 0104 SH-then-SZ 降级并标注 needs-verification，wire 序待在线验证）。
- 2 个新测试：交集全覆盖+无重复+ETF 进 P6/P7、disabled 代码不进任何分区。webClx `211316-18d1330bb455c5d5` / `3173_build.log`：fullpull 160 passed/1 ignored、clippy、release 全绿（3170-3172 依次修 u16 字面量、P7 长度断言 362、冗余 u32 转换）。
- 顺序边界：P6/P7 26 段 vendor 枚举序仍未复刻；集合实现可用于验证「顺序是否影响 2704 收流」。

## 2026-09-02 21:17 receive-list P1-P5 与已证 primary builder 一致性回归（3174）

- 新增 `receive_list_primary_partitions_match_the_verified_primary_builder`：全覆盖接收清单下，receive-list 原语的 P1–P5 与逐字节验证的 primary builder **逐条一致**，且 P6/P7 与 P1–P5 无交集。
- webClx `211529-18d1330bb455c5d6` / `3174_build.log`：fullpull 161 passed/1 ignored、clippy、release 全绿。receive-list 原语与已验证逻辑闭环一致。

## 2026-09-02 21:18 真实数据验证：receive-list 原语集合在 accept2000 上完全成立

- 用真实 accept2000 SH/SZ 0104 + 只接收清单跑原语逻辑（python 等价）：P1-P5 与抓包逐条相等（True×5）；P6+P7 模型 code 集合 = 实际 2a10 P6∪P7 code 集合（1083=1083，model-actual 空、actual-model 空，全在清单）。
- 差异仅在顺序（模型 0104 SH-then-SZ 序 vs vendor 26 段枚举序），集合层面完全成立。Rust 原语在真实数据正确（合成测试之外的强验证）。

## 2026-09-02 21:20 下个交易时段执行计划（P6/P7 顺序敏感度 + 全集订阅验证）已写入 wiring 任务文档

- 背景：接收清单∩0104=6203 集合双窗口证实；P1-P5 逐字节复刻；P6/P7 集合证实但 vendor 26 段枚举序未复刻（实现用 0104 SH-then-SZ 降级序）。
- 待在线判定：订阅顺序是否影响 2704 收流。计划含前置（login_primary=true+跳过旧式 Ask+清单与 0104 同日）、4 步（复验→全集发送 vs 仅 P1-P5 对照→观察 2704 覆盖→判定三态：顺序不敏感可全推 / 顺序敏感需反推枚举序 / 发送被拒回滚）。
- 全程 opt-in、不写生产默认、full-push 门禁保持关闭。下次唤醒或并行会话可直接照此执行。

## 2026-09-02 21:21 修复 receive-list 原语 P6 短剩余 drain panic（webClx 3175）

- 审查发现 `build_official_5188_receive_list_partitions` 的 P6 用 `remainder.drain(..1024)` 在剩余不足 1024 时 panic（极端清单/表变化场景）。改用 `split_off(min(len,1024))` 安全切分，P6/P7 可为空。
- 新增回归测试 `receive_list_tolerates_a_short_or_empty_remainder`：恰好 5120（P6/P7 空）+ 5125 eligible+3 ETF（P6=8、P7 空）。webClx `211938-18d1330bb455c5d7` / `3175_build.log`：fullpull 162 passed/1 ignored、clippy、release 全绿。

## 2026-09-02 21:23 receive-list market-qualified 匹配修复（webClx 3176）

- 审查发现原实现用裸 6 位 code 匹配清单，会误把 SH000001/SZ000001 等跨市场同码串市场。改为 market-qualified（SH/SZ + code）匹配，与真实数据 python 对照（m+c）一致。
- 5 个测试 receive set 构造同步改 market-qualified。webClx `212148-18d1330bb455c5d8` / `3176_build.log`：fullpull 162 passed/1 ignored、clippy、release 全绿。
- 该修复保证 builder 输入契约 = 市场限定符号（与只接收股票代码表.csv 的 SHxxxxxx/SZxxxxxx 格式一致）。

## 2026-09-02 21:26 协作证据：并行智能体排入 fullpull-open-accept（s5233）

- 发现第二定时唤醒 `fullpull-open-accept`（session s5233/quoteNetzipRs_10_main，09-03 09:12，非本会话创建）：开盘验收 = Wine 10×5188 ESTAB + login_primary=true 检查、09:30 前后 5188 tcpdump + 完整 quote_batch ≥300s、official_5188_extract + callback parity 2704 字段判定、并按 skill '2026-09-03 open-market acceptance checklist' 第5步做 P6/P7 分区实验、结果落盘权威文档。
- 与本会话 `next-market-open-check`（s5272，09-03 09:10，跨日订阅一致性复验）互补不冲突：先复验清单一致性，再由并行验收做 2704 解码与 P6/P7 实验。
- 这是除 futex 进程外的并行智能体活动证据。两唤醒均已存在，不取消不改动。

## 2026-09-02 21:27 阶段快照：等待 09-03 交易时段窗口

- 本日协议侧闭环（全部离线验证、无生产变更）：①订阅=清单∩0104（6203/6203 双窗口+跨日平移因果）；②P1-P5 逐字节复刻（与 primary builder 一致性锁定）；③P6/P7 集合证实、26 段枚举序精确记录（顺序规则待在线判定）；④vendor_pm 编号序列回归（L1=1/登录2..11/成对递增4）；⑤L1 运行时字段→登录包端到端。
- 权威 fullpull 162 tests/1 ignored 全绿（3174/3175/3176）；审计工具含 --receive-list 全集验证；下个交易时段执行计划已入 wiring 文档。
- 待执行：09-03 09:10（本会话跨日复验）+ 09:12（并行智能体开盘验收 + P6/P7 顺序/2704 覆盖判定）。full-push 门禁保持关闭直至全集 2704 覆盖在线证实。

## 2026-09-02 21:29 审计脚本 --local-ip 参数化

- audit-official-5188-partition-alignment.py 的本地 IP（原硬编码 192.168.3.2）改为 `--local-ip` 参数（默认不变），跨日/跨源抓包复验更稳健。accept2000 回归：6203/6203/all=True；py_compile+diff-check 通过。

## 2026-09-02 21:33 P6 26 段枚举序候选语义（needs-verification）

- 段序呈「证券类型分组」：SZ301尾→SZ302→SH000指数→SH转债→SZ企业债→SZ399指数→SH ETF→SH LOF→SH588→SH900 B股→SZ159基金。各组内 0104 ordinal 连续。疑似 vendor「类型枚举表」序。
- 标注 needs-verification：需另一生命周期/清单变更对照。若成立，实现=类型枚举序+组内 ordinal。生产仍用降级序待在线顺序敏感度判定。

## 2026-09-02 21:36 决定性：310 条未匹配的首次根因分解（离线，可立即行动）

- 基于已抓 parity 报告（20 mismatch_samples，外推 310）：**A Wine state-merge 保留旧值 ~155**（int amount=0 + ts 15:00:01 vs cb 15:00:00——`0x44aa30` 跳过零值增量，非解码错）；**C f32 精度 ~78**（差<1000，非真差）；**D 真值分歧 ~62**（603066/603071 11/13 字段不匹配——增量流缺 delta 或 baseline 错）；**B 解码 bug ~16**（603069 负 amount=-165257699639，符号/截断错）。
- 可立即行动：B 桶 decoder bug 可修；A 桶合并规则可离线验证复刻；D 桶需确认增量流 vs baseline。C 桶接受。
- 这把「310 未匹配」从整体模糊降维到具体可修复子项。

## 2026-09-02 21:40 B 桶负 amount 根因定位（decoder 层面）

- 来源：`decode_value_accumulators` 的 `set_i64(0x1c, delta_amount.wrapping_add(baseline_amount))` i64 下溢。Wine amount 非负，负值 = delta 解码或 baseline 起始值错。
- 防御性修复方案：加非负校验标记解码失败，深层修复需跨 delta-token 取证。已记录取证笔记。

## 2026-09-02 21:48 决定性：A桶 state-merge 语义修复后 parity 大幅提升

- parity 对比器接入 zero-amount delta-skip 规则：`internal_amount==0` 时 amount/price/open/high/low/volume/ask/bid/timestamp 视为增量未携带（`0x44aa30` 保留旧 state），不视为 mismatch。
- **字段命中数大幅提升**：amount 83→1013(+930)、price 355→1281(+926)、volume 377→1307(+930)、timestamp 974→1353(+379)。
- mismatch samples 中 A 桶 10 条全部消除（A=0），剩余 D12+C7+B1=20。旧匹配率 1792/2102=85.3%；新匹配率在字段层面从 ~40% 提升到 ~70%。
- webClx `214535-18d1330bb455c5d9` / `3177_build.log` 编译通过；本地重新编译后真实数据复验 `/tmp/parity-abucket-fix2.json`。

## 2026-09-02 21:51 决定性：D桶根因收窄——单一收盘批次（seq=34），全 20 条在 603xxx 连续块

- A 桶修复后剩余 20 条 mismatch_samples **全部来自 seq=34 单一批次**（15:00:00 收盘竞价窗口），且全部为 6030xx 连续代码段。这不是散布性的解码 bug，而是一次性批量更新差异。
- 子分类：D_full_replace(4) + E_partial(11) + C_f32(4) + B_bug(1)。D_full_replace 的 603066/603071/603085/603101 是 Wine 收到了完整的另一版本增量（5188 流缺失或以 snapshot 方式表达），非逐字段解码错。
- **真实失配率仅 20/1792 = 1.1%**（远低于 310/2102 = 14.7% 的旧口径——旧口径把 A 桶增量未携带也计为 mismatch）。A 桶修复后真实残留仅此一个批次。

## 2026-09-03 02:26 B 桶防御性修复确认（webClx 3178）

- amount 非负校验（负值返回解码错误而非产出负值）通过 fullpull 162 tests/1 ignored、clippy、release 全绿。webClx `231526-18d1330bb455c5da` / `3178_build.log`。
- 当前等待：09:10 跨日复验 + 09:12 并行开盘验收。市场已收盘无新窗口。

## 2026-09-03 07:35 Wine 兼容 API 路由实现（webClx 4002）

- quoteNetzipRs 新增 Wine 兼容 API 路由：`GET /api/v1/status`（Wine 形态的 gateway_forwarder + vendor_config JSON）和 `GET /api/v1/events`（有界事件日志，limit 参数）。这使 RS 可在 Wine 停止时无缝替换同一端口上的查询服务。
- RS 默认端口仍为 16893，但已支持 `NETZIP_SERVICE_LISTEN=0.0.0.0:28787` 环境变量覆盖。当 Wine 停止时，只需设置该变量并重启 RS 服务即接管 28787 端口。
- 并行智能体在 auth_7100.rs 中添加了 `mac_ascii` 字段（LoginControlFields 和 AbkControlFields），期间 4001 编译因编辑到中间态失败，后续 4002 合并后编译通过。
- webClx `073517-18d1a47c279ecbc9` / `4002_build.log`：status=0。

## 2026-09-04 离线收尾进展

- `open-0921-20260903` 已完成脱敏盘点：9 个 5188 flow、2167 个 2704、18 个 3901；Wine 同窗 6000 事件/101516 quotes/4201 symbols（09:22:05-09:23:18）。该窗口从初始化后开始，不能用于重建客户端 2a10。
- `20260904-cold-start/wine-night-coldstart-20260904T011938` 已验证 fresh-slot 基线：38 帧、5171/5171 记录匹配 Wine 内存，0104 作为基线的模型被否定。
- 当前门禁：继续 shadow/receive-only；公开行情投影仍待 09:25 同符号同时间 Wine callback parity、重连恢复及 P6/P7 在线顺序验证。
