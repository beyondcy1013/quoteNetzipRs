# NetzipRs

Linux 原生 Rust 行情服务，直连网际风 `7709` 上游，不依赖 Windows、Wine、
`Stock.dll` 或 FoxTrader。生产链路为：

```text
网际风 7709 -> NetzipRs -> quoteGateway(netzipRust7709) -> stockScreener
```

## 快速使用

- HTTP/Web GUI：`http://192.168.3.2:16893/`
- 监听：`0.0.0.0:16893`（局域网可用，无鉴权，不应映射到公网）
- 健康检查：`GET /health`
- 主动查行情：`GET /api/quotes?codes=SZ000001,SH600000`
- 服务：`netzip-rs.service`
- 常驻全推：`netzip-rs-full-push.service`

```bash
curl -fsS http://192.168.3.2:16893/health
curl -fsS 'http://192.168.3.2:16893/api/quotes?codes=SZ000001,SH600000'
systemctl status netzip-rs.service netzip-rs-full-push.service
```

全推只发布到 quoteGateway 独立来源 `netzipRust7709`，不写入
`quoteNetzipWine`。传输优先使用 `127.0.0.1:16889` 的持久 TCP MessagePack + zstd，
失败时回退到 `POST http://127.0.0.1:16886/api/source/netzipRust7709/ingest`。

### 公共行情时间

`0x0547` 的 `time_hhmmss_raw` 是原始诊断字段，闭市后可真实出现沪市 `153050`、
深市 `153000`；它不会被修改。对外 `datetime / quote_datetime` 则复刻当前生产
`网际风.exe` 的 OEM 公共对象语义：有效源时间超过 `150000` 时返回 `15:00:00`，
盘中时间保持原值。无固定时间字段而使用 `extra0` 提示兜底时也遵循同一规则。

证据来自生产文件 `/home/third_party/quoteNetzipWine/网际风.exe`（SHA-256
`de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29`）：
`0x4839a3` 比较源对象时间与 `0x249f0`（`150000`）并取上限值，随后
`0x438800` 将 `HHMMSS` 转成当日秒数并写入公共对象时间。历史分析中出现的
`Stock.dll 0x1007e4d0 / 0x1007ef80` 只适用于对应旧样本的结构研究，不能替代
当前生产桥的哈希和反汇编证据。

发布到 quoteGateway 的行以 `source_protocol=netzip-rust-7709-0547.v2` 标记这次
公共时间语义。quoteGateway 只允许已有 v1 行被 v2 行迁移覆盖；除此以外的行情时间
倒退仍按 stale 数据拒绝，避免为了修正旧缓存而放宽正常的时序保护。
公共成交额复刻由 `netzip-rust-7709-0547.v3` 标记；quoteGateway 仅允许同一时间的
v2 缓存行迁移到 v3，跨交易日和普通同版本冲突仍按原规则拒绝。

本仓库同时保留 `Stock.dll`、Wine 和历史图文卡协议的分析资料，用于字段校对和回归；
它们不是当前 Linux 生产链路的运行依赖。

## 术语说明

在当前仓库里，和网际风这套行情/文件下载链路相关的旧生态兼容体系，统一注明为：

- `图文卡`
- `通视`
- `分析家通用`

这里的含义是：

- 它们指向的是同一类旧版炒股软件兼容生态，典型对象包括 `分析家`、`飞狐交易师`、`大智慧` 这类能对接 `StockDrv.dll / Stock.dll` 的软件或接口层。
- 这组名称可用于描述“兼容目标/历史生态/接口风格”，方便和现有抓包、DLL 导出、参考样本建立对应关系。

但需要特别区分：

- 这并不等于已经证明“当前在线 TCP 负载就是公开老 `图文卡/通视` 裸协议”。
- 当前仓库实际抓到的线上链路，仍然明显带有网际风自有的 `网络包 / penc / ZSTD字典` 外层与后续壳。
- 因此，`图文卡/通视/分析家通用` 在本仓库中首先是“兼容生态与术语标签”，不是对当前线上字节流形态的过度简化。

## 参考目录

本项目有一个重要的参考目录（已移入本项目内）：

- 相对路径：`./netzip_api_bin/NetzipAPI`
- 绝对路径：`/home/codes/quoteNetzipRs/netzipapi-rust-demo/netzip_api_bin/NetzipAPI`

这个目录保存了官方/历史样本中的接口规范、C++/C#/Python 示例、DLL 与配置文件、服务器列表和相关资源。当前仓库中的 DLL 调用方式、请求串格式、运行依赖、抓包分析结论，都需要和这个参考目录交叉核对。

更完整的目录关系、用途说明和使用约定见：

- [AGENTS.md](./AGENTS.md)

当前结论：

- 可以持续推进 Rust 版本，而且当前实施优先级已经切到纯 Rust + Linux。
- 当前已经有一条不依赖 Windows DLL 的 `7709` Linux 原生链，可直接支撑 `代码表 / 实时行情 / K线 / F10分类`。
- 如果目标是完整替代 `Stock.dll + 网际风.exe + 127.0.0.1:2000` 整条 Windows 主链，仍需要继续逆 `7100 / 2000` 更深壳层。
- `Ask(...)` 的请求串是 DLL 上层调用语义，不应直接等同为远端 `6100/7100/7709/7719` 的原始网络帧。

## 当前分层全景

基于 `windows_debug/` 下 `2026-03-29/30` 的现场取证，当前仓库需要按 3 层来理解，而不是把所有东西压成“一条直连远端协议”：

1. Windows 上层 API
   - 对外仍是官方 `Start / Ask / Stop`。
   - `Ask(...)` 的输入仍是明文请求串，例如 `股票数据?请求=实时数据...`。
   - 这些请求串会先在 DLL 内部转换成对象后再下发，不能直接当成远端 wire 字节流。
   - 但返回不只有一种形态，当前现场已经确认同时存在：
     - 同步文本/提示信息
     - 异步 JSON 状态回调
     - `OEM_DATA_HEAD + payload` 的结构化数据回包
   - 当前库层已经把这三类统一落成 `src/stock_message.rs`：
     - `ret=0` 视为“同步空返回但请求已受理”，后续等待 callback
     - `ret>0` 既可能是 JSON/文本，也可能是 `OEM_DATA_HEAD + payload`
     - callback 已确认至少有 `提示信息 / 消息 / 股票数据 / 错误` 四类 `form`
   - 现场样本还确认：
     - 请求串里的 `编号` 不能直接等同于回包里的 `ask_id`
     - 例如 `编号=101/102/103` 的同步结构化回包，实际 `ask_id=4/5/6`

2. Windows 本地桥
   - 当前现场主链是 `FoxTrader.exe / Stock.dll -> 127.0.0.1:2000 -> 网际风.exe`。
   - `127.0.0.1:2000` 不是简单的明文请求口；真实包体里已经看到 `网络包 / penc / hypenc`。
   - `127.0.0.1:2001` 在现场处于监听但未被主链使用；`5188 / 22223` 更像其它兼容接口而不是本次 `FoxTrader` 主链。

3. 远端认证与行情链
   - `网际风.exe` 再负责连向远端 `6100 / 7100 / 7709 / 7719 / 7708 / 14017` 等端口。
   - 当前仓库里对 `6100 / 7100 / 7709 / 7719` 的抓包重组、对象解析、回放验证，主要发生在这一层。
   - 因此文档里所有“`网络包 / penc / ZSTD字典 / 下载文件`”相关结论，默认是在“本地桥之下的远端链”这一层成立。

更细的现场证据与原始结论见：

- `./windows_debug/findings.md`
- `./windows_debug/loop2000_local_protocol_notes_20260329.md`
- `./windows_debug/progress.md`
- `./windows_debug/task_plan.md`

## 对完成 Rust 行情软件的直接价值

基于当前 `windows_debug/` 现场成果，真正能直接用到“完成 Rust 版行情软件”的结论，可以分成 4 类：

1. 可以直接落成产品主链的结论
   - 当前最先可交付的主链已经切到纯 Rust + Linux 的 `7709` 原生链：
     - Rust 程序
     - 直连 `7709`
     - 代码表同步
     - `0x0547` 实时行情
     - 在线 `K线`
     - `F10` 栏目查询
   - 当前仓库已经把这条链落成：
     - `src/tdx7709.rs`
     - `src/bin/netzip_linux.rs`
     - `POST /api/tdx7709/snapshot`
     - `POST /api/linux/pure-rust-mvp`
   - 这意味着“先做可用 Rust 行情软件”的推荐路线，已经切到 Linux-first：优先补会话复用、缓存、统一请求配置和一致性回归。

2. 可以直接变成初始化/缓存能力的结论
   - 现场已经完整抓到并拆开初始化阶段的大对象顺序：
     - `代码表`
     - `除权`
     - `财务`
     - `文件: 数据\\财务V6.fin`
     - `实时数据`
   - 这些对象里，当前仓库已经能稳定结构化解析：
     - `OEM_STKINFO`
     - `OEM_SPLIT_HEAD / OEM_SPLIT`
     - `OEM_FINANCE`
     - `OEM_REPORT`
     - `财务V6.fin / 财务V8.fin`
   - 这直接决定了 Rust 行情软件可以先做“启动即同步本地缓存/代码表/财务/除权”，不必等 `7100` 壳层完全还原。

3. 可以直接当回归样本和校对锚点的结论
   - 本地 `2000` 不是明文接口，而是带 `网络包 / penc / hypenc` 的桥协议，这一点已经落成：
     - `src/local_2000.rs`
     - `POST /api/debug/local-2000-log-scan`
   - `Ask(...)` 的三类回包已经落成：
     - `src/stock_message.rs`
   - 本地 `2000` 与远端 `7100` 的壳层对位已经落成：
     - `src/local_2000_vs_auth7100.rs`
     - `GET/POST /api/debug/local-2000-vs-auth-7100`
   - 因而 Windows 现场产物现在不只是笔记，而是 Rust 侧可持续回归的 fixture。

4. 还不能阻塞 MVP 的低优先级结论
   - 这些项重要，但不该阻塞“先交付可用 Rust 行情软件”：
     - `hypenc` 的精确定义
     - 本地 `2000` 第二个/后续 `penc` 的 payload 内语义
     - `7100` 登录后 `Tdx_Encrypt / penc / ZSTD字典` 的完整可逆链
   - 它们影响的是：
     - 是否要无 DLL 纯 Rust 直连上游
     - 是否要做字节级兼容旧本地桥
   - 它们不影响先做：
     - Linux 下的 Rust 原生行情客户端
     - 基于 `7709` 的稳定查询、缓存和聚合探针

因此，当前仓库更合理的交付顺序是：

1. Linux-first MVP
   - 继续扩展纯 Rust `7709` 主线
   - 优先补单连接复用、代码表缓存、统一请求配置和回归链
   - 这是当前推荐主线
2. 初始化缓存闭环
   - 落地代码表、除权、财务、实时快照的持久化与回放
   - 用 `财务V8.fin` 和 `OEM_FINANCE` 做一致性校验
3. 纯 Rust 远端链完整替代
   - 继续推进 `2000 <-> 7100` 壳对位
   - 再推进 `7100` 的 `Tdx_Encrypt` 与登录后对象链
4. Windows 主链对账锚点
   - 保留 `x86 Stock.dll + 网际风.exe + 127.0.0.1:2000` 作为行为校验和字段对账来源
   - 不再作为最高优先级交付路径

## 已确认的 DLL 导出

- `Start`
- `Ask`
- `Stop`

## 运行前提

根据参考目录与 `2026-03-29/30` Windows 现场，运行前提不能只看 `Stock64.dll` 本体，还要区分当前到底走哪条链：

- x64 示例链：`Stock64.dll`
- 现场主用链：`x86 Stock.dll`
- 本地桥接进程：`网际风.exe`
- 运行时资源：`Stock.dat`、`Stock.字典`
- 驱动/桥接相关：`系统/Stockdrv.dll`
- 用户与升级侧配置：`用户/配置文件.ini`、`用户/服务器列表.ini`、`升级配置.ini`
- 用户与数据侧文件：代码表 `csv`、`数据/*.fin`、`数据/*.pwr`
- 如果要复现飞狐现场主链，还需要本地桥进程 `网际风.exe` 正在运行；当前现场主入口是 `127.0.0.1:2000`

当前现场还额外确认了一点：

- `D:\\Soft\\_Stock\\飞狐2020` 真实安装目录里使用的是 `x86 Stock.dll`，没有 `Stock64.dll`
- 因而仓库里的 `Stock64.dll` 示例可继续保留，但不能把它当成当前 Windows 现场的唯一事实来源

项目内的参考位置：

- `./netzip_api_bin/NetzipAPI/StockC++/Stock.dll`
- `./netzip_api_bin/NetzipAPI/StockC++/Stock64.dll`
- `./netzip_api_bin/NetzipAPI/StockC++/网际风.exe`
- `./netzip_api_bin/NetzipAPI/StockC++/Stock.dat`
- `./netzip_api_bin/NetzipAPI/StockC++/Stock.字典`
- `./netzip_api_bin/NetzipAPI/StockC++/系统/Stockdrv.dll`
- `./netzip_api_bin/NetzipAPI/StockC++/用户/配置文件.ini`
- `./netzip_api_bin/NetzipAPI/StockC++/用户/服务器列表.ini`
- `./netzip_api_bin/NetzipAPI/StockC++/升级配置.ini`

## 用法

Windows 下：

```powershell
cargo run --release -- "C:\path\to\Stock64.dll"
```

不传 DLL 路径时，默认尝试当前目录下的 `Stock64.dll`。

如果要对照 `2026-03-29/30` 的飞狐现场，需要额外注意：

- 现场主用的是 `x86 Stock.dll`
- 这条链路依赖正在运行的 `网际风.exe`
- 因此不能把当前示例的 `Stock64.dll` 启动方式，直接等价成现场真实运行方式
- 同时也不要把 `Ask(...)` 的明文请求串直接当成远端抓包里的原始发送帧

启动后会先按官方示例发送三条初始化请求：

1. 登录认证模块
2. 登录股票备用模块
3. 初始化分析软件

之后进入交互模式，可以直接粘贴类似下面的调用串：

```text
股票数据?请求=实时数据&代码=SH600000&数量=1&等待=10000
```

输入 `q` 退出。

这里的 `股票数据?请求=...` 只是 DLL 上层 API 语义，不等于最终出现在公网抓包里的原始 TCP 字节流。Windows 现场已经确认，这些请求至少还会经过 DLL 内部对象封装和本地 `127.0.0.1:2000` 二进制桥层。

Linux 下如果优先走纯 Rust 路线，当前直接可用的入口是：

```bash
cargo run --bin netzip_linux -- --help
cargo run --bin netzip_linux -- snapshot SH600000
cargo run --bin netzip_linux -- live-quote SH600000 SZ000001
cargo run --bin netzip_linux -- kline SH600000 --count 10
cargo run --bin netzip_linux -- f10-categories SH600000 --limit 5
cargo run --bin netzip_linux -- sync-code-table --out /tmp/tdx7709_codes.csv
```

这个入口不会调用任何 Windows DLL，而是直接复用当前仓库里已经落地的 `7709` 纯 Rust 协议能力。它当前适合：

- 快速验证 Linux 原生 `实时行情 / K线 / F10分类 / 代码表同步`
- 在不启动 HTTP 服务的情况下，直接从命令行拉 `live-quote / kline / f10-categories / f10-content`
- 把“哪些子能力可用、哪些子能力仍失败”一次性打成一份 JSON 探针结果
- 作为后续 Linux-first 行情客户端的直接入口，而不是继续只靠 example 和调试接口拼装

### Rust Stockdrv 主动请求路径

`crates/stockdrv-compat` 当前实现了一个经过 Windows 隔离探针验证的主动实时请求路径：

```text
飞狐/测试程序
  -> GetStockByCode(L"SH600000", OEM_REPORT*, flags)
  -> Stockdrv.dll HTTP GET /api/quotes?codes=600000&refresh=false
  -> 校验响应 market 与请求前缀一致
  -> 编码并写回 500 字节 OEM_REPORT
```

网关地址默认是 `192.168.3.2:16886`，可通过 `TUWENCA_GATEWAY_ADDR` 覆盖。任何连接失败、HTTP/JSON 错误、市场不匹配或关键字段缺失都会返回 `0`，并追加到工作目录的 `stockdrv-compat.log`；不会拼装虚假行情。

2026-07-26 在 `192.168.3.38` 的 32 位 Windows 探针验证得到：

- `SH600000` 成功返回浦发银行，收盘价 `9.04`、成交量 `506751`、成交额约 `459285300`。
- `SH000001` 被拒绝，因为当前 quote-gateway 将它与 `SZ000001` 归一化后返回了深圳市场数据。
- `SZ000001` 被拒绝，因为当前有效响应缺少名称和十档盘口字段。

当前仍不是可直接替换飞狐生产 DLL 的完整版本：代码表、`GetTradeData`、日线、1/5 分钟线和真实 FoxTrader 指针参数合同尚未闭环。必须先完善这些接口并在备份可恢复的客户端沙盒中验证，不能只凭隔离探针成功就覆盖现有 `Stockdrv.dll`。

### Rust Stockdrv 全推回调

`Stock_Init(HWND, message_id, reserved)` 已按原 x86 DLL 的窗口消息机制实现：

```text
Stock_Init 注册窗口和消息号
  -> 后台读取 quote-gateway 全市场工作表
  -> 生成 158 字节兼容记录
  -> 每批最多 2000 条
  -> SendMessageW(hwnd, message_id, 0x3f001234, packet_ptr)
```

`lParam` 是原版 Stockdrv 的私有 Fox 容器：292 字节管理头（`magic(u32)` 位于偏移 0，`count(u32)` 位于偏移 4，GBK 类型标识位于偏移 `0x14`，首记录指针位于偏移 `0x11c`）后跟 `records[count]`，每条记录固定 158 字节。默认读取 `/api/codes/worklist?include_unknown=true&limit=6000`；`TUWENCA_PUSH_CODES` 可在隔离调试时覆盖为逗号分隔的指定代码，`TUWENCA_PUSH_INTERVAL_MS` 控制轮询间隔。

2026-07-26 在 32 位 Windows 隐藏窗口中完成真实 `SendMessageW` 验收：5531 条股票被拆为 `2000 + 2000 + 1531` 三批，总耗时约 303ms；收到的首批首条为 `SZ000001`，价格 `11.1`。`Stock_Quit` 会停止并等待推送线程退出。

## 作为库使用

当前 crate 已经提供库入口，可以直接解析返回包：

```rust
use netzipapi_rust_demo::{parse_answer_buffer, Packet};

fn handle_answer(buf: &[u8]) {
    if let Some(packet) = unsafe { parse_answer_buffer(buf) } {
        match packet {
            Packet::Realtime { items } => {
                if let Some(first) = items.first() {
                    println!("{} {} {}", first.label, first.close, first.time);
                }
            }
            other => {
                println!("{}", other.summary());
            }
        }
    }
}
```

## 当前已做的结构化解析

- `代码表`
- `实时数据`
- `分笔`
- `分时`
- `1/5/15/30/60 分钟线、日线、周线、月线、季线、年线、多日线`
- `除权`
- `财务`
- `F10资料`
- `6到10档挂单`

如果后面要接进正式项目，更建议直接用库接口，而不是继续扩写示例 `main.rs`。

## HTTP 服务

当前仓库已经补了一层最小可用的 HTTP 服务，入口是 `src/bin/netzip_service.rs`：

```bash
cargo run --bin netzip_service -- --listen 0.0.0.0:16893
```

也可以用环境变量：

```bash
NETZIP_SERVICE_LISTEN=0.0.0.0:16893 cargo run --bin netzip_service
```

当前已提供的稳定接口：

- `GET /`
- `GET /health`
- `GET /api/capabilities`
- `GET /api/quotes?codes=SH600000,SZ000001`（仅作为 `quote-gateway` 的 Linux/Rust 上游行情接口）
- `POST /api/hqw/publish`（将指定行情直接投递到 quote-gateway 现有 HQW 数据源）
- `POST /api/hqw/publish-worklist`（按 quote-gateway 处理股票工作表，从 7709 单会话分批全推）
- `POST /api/fin/parse`
- `POST /api/fin/query-record`
- `POST /api/fin/getter-value`
- `POST /api/fin/getter-specs`
- `POST /api/tdx7709/sync-code-table`
- `POST /api/tdx7709/query-code-table`
- `POST /api/tdx7709/live-quote`
- `POST /api/tdx7709/snapshot`
- `POST /api/tdx7709/kline`
- `POST /api/tdx7709/f10/categories`
- `POST /api/tdx7709/f10/content`
- `GET /api/tdx7709/bootstrap-plan`
- `GET /api/debug/tdx118-dump-compare-plan`
- `POST /api/debug/answer-summary`
- `POST /api/debug/blob-compare`
- `POST /api/debug/quote-0547-decode`
- `POST /api/quote/0547/query`
- `POST /api/quote/0547/extra-profile`
- `POST /api/debug/quote-0547-query`
- `POST /api/debug/quote-0547-extra-profile`
- `POST /api/debug/quote-frame-scan`
- `POST /api/debug/quote-replay`
- `POST /api/debug/proto-probe`
- `POST /api/debug/pcap-summary`
- `POST /api/debug/local-2000-log-scan`
- `GET /api/debug/local-2000-vs-auth-7100`
- `POST /api/debug/local-2000-vs-auth-7100`
- `POST /api/debug/stream-analyze`

示例：

```bash
curl -fsS http://127.0.0.1:16893/health
curl -fsS http://127.0.0.1:16893/api/capabilities
curl -fsS 'http://127.0.0.1:16893/api/quotes?codes=SH600000,SZ000001'
curl -fsS -X POST http://127.0.0.1:16893/api/hqw/publish \
  -H 'Content-Type: application/json' \
  -d '{"symbols":["SH600000","SZ000001"],"trade_date":"2026-07-24"}'
curl -fsS -X POST http://127.0.0.1:16893/api/hqw/publish-worklist \
  -H 'Content-Type: application/json' \
  -d '{"batch_size":100,"limit":6000}'
curl -fsS -X POST http://127.0.0.1:16893/api/fin/parse \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/full_sh.FIN","limit":2}'
curl -fsS -X POST http://127.0.0.1:16893/api/fin/query-record \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/full_sh.FIN","symbol":"SH600000"}'
curl -fsS -X POST http://127.0.0.1:16893/api/fin/getter-value \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/full_sh.FIN","symbol":"SH600000","field_id":43}'
curl -fsS -X POST http://127.0.0.1:16893/api/fin/getter-specs \
  -H 'Content-Type: application/json' \
  -d '{}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/sync-code-table \
  -H 'Content-Type: application/json' \
  -d '{"preview_limit":1}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/query-code-table \
  -H 'Content-Type: application/json' \
  -d '{"host":"120.195.71.160","port":7709,"query":"600000","limit":5}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/live-quote \
  -H 'Content-Type: application/json' \
  -d '{"host":"120.195.71.160","port":7709,"symbols":["SH600000","SZ300948"],"settle_ms":300}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/snapshot \
  -H 'Content-Type: application/json' \
  -d '{"host":"120.195.71.160","port":7709,"symbol":"SH600000","kline_type":"1d","kline_count":3,"f10_limit":5,"read_timeout_ms":400,"connect_timeout_ms":1000,"settle_ms":100}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/snapshot \
  -H 'Content-Type: application/json' \
  -d '{"symbol":"SH600000","include_f10":false,"kline_type":"1d","kline_count":3}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/kline \
  -H 'Content-Type: application/json' \
  -d '{"host":"120.195.71.160","port":7709,"symbol":"SH600000","kline_type":"1d","count":5,"settle_ms":300}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/f10/categories \
  -H 'Content-Type: application/json' \
  -d '{"host":"120.195.71.160","port":7709,"symbol":"SH600000","limit":5,"settle_ms":300}'
curl -fsS -X POST http://127.0.0.1:16893/api/tdx7709/f10/content \
  -H 'Content-Type: application/json' \
  -d '{"host":"120.195.71.160","port":7709,"symbol":"SH600000","category_name":"最新提示","preview_chars":300,"settle_ms":300}'
curl -fsS http://127.0.0.1:16893/api/tdx7709/bootstrap-plan
curl -fsS http://127.0.0.1:16893/api/debug/tdx118-dump-compare-plan
curl -fsS -X POST http://127.0.0.1:16893/api/debug/answer-summary \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/answer.bin"}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/blob-compare \
  -H 'Content-Type: application/json' \
  -d '{"left_path":"/tmp/tdx118_plain.bin","right_path":"/tmp/tdx118_cipher.bin","compare_len":280,"block_size":8}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/quote-0547-decode \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/quote0547_server/frame063_sub2900_tag0547_off528759_inflated.bin","limit":5}'
curl -fsS -X POST http://127.0.0.1:16893/api/quote/0547/query \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/quote0547_server","query":"SH600000","limit":5}'
curl -fsS -X POST http://127.0.0.1:16893/api/quote/0547/query \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/quote0547_server","query":"","prefix3":"300","state_matrix":"with_quote_head_time_present","limit":5}'
curl -fsS -X POST http://127.0.0.1:16893/api/quote/0547/extra-profile \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/quote0547_server/frame067_sub2a00_tag0547_off558932_inflated.bin","anomaly_limit":8}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/quote-frame-scan \
  -H 'Content-Type: application/json' \
  -d '{"path":"/tmp/flow_7709_2400.bin"}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/quote-replay \
  -H 'Content-Type: application/json' \
  -d '{"path":"/home/codes/quoteNetzipRs/netzipapi-rust-demo/tmp/flow_2655_7709_probe.bin","host":"120.195.71.160","port":7709}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/proto-probe \
  -H 'Content-Type: application/json' \
  -d '{"host":"120.195.71.160","port":7709,"payload":"0c0100000000020002001500","encoding":"hex","read_secs":1}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/pcap-summary \
  -H 'Content-Type: application/json' \
  -d '{"path":"/home/codes/quoteNetzipRs/netzipapi-rust-demo/tmp/netzip_full_tcp.pcap","segment_limit":12}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/local-2000-log-scan \
  -H 'Content-Type: application/json' \
  -d '{"path":"/home/codes/quoteNetzipRs/netzipapi-rust-demo/windows_debug/tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11.log","port":2000,"small_max":4096,"code_preview_limit":20}'
curl -fsS http://127.0.0.1:16893/api/debug/local-2000-vs-auth-7100
curl -fsS -X POST http://127.0.0.1:16893/api/debug/local-2000-vs-auth-7100 \
  -H 'Content-Type: application/json' \
  -d '{"local_log_path":"/home/codes/quoteNetzipRs/netzipapi-rust-demo/windows_debug/tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11.log","auth_pcap_path":"/home/codes/quoteNetzipRs/netzipapi-rust-demo/tmp/netzip_full_tcp.pcap"}'
curl -fsS -X POST http://127.0.0.1:16893/api/debug/stream-analyze \
  -H 'Content-Type: application/json' \
  -d '{"path":"/home/codes/quoteNetzipRs/netzipapi-rust-demo/captured_windows_traffic/client_to_server_full.raw","is_hex":false}'
```

`GET /api/quotes` 是面向 `quote-gateway` 的紧凑生产契约，数据链为
`NetzipRs -> quote-gateway`，不依赖 FoxTrader、Windows 或 Wine。当前 `0x0547`
解析已提供价格、昨收、开高低、总成交量、当前成交量、成交额和行情时间。
正的累计成交量会在 `0x0547` 解码边界从 wire 值归一化为 OEM 公共值
`wire_volume + 1`；wire `0` 保持 `0`。这个规则不应用于当前成交量。
`Tdx0547Record.amount / amount_raw` 保留 wire 诊断值；紧凑生产接口在公共对象边界复刻
网际风按证券类别执行的 `f32` 有损量化，不做固定偏移补偿，quoteGateway 也不二次计算。
`POST /api/tdx7709/live-quote` 保留为详细诊断协议，便于与紧凑接口做 A/B 对比。

`POST /api/hqw/publish` 只使用 quote-gateway 的独立 Rust 入口
`POST /api/source/netzipRust7709/ingest`，身份固定为
`schema=netzipRust7709.quote_batch.v1`、`source=netzipRust7709`。它不会回退、转写或发布到
`quoteNetzipWine`/`hqw`；新入口不可用时请求直接失败。默认目标为 `127.0.0.1:16886`，可通过
`NETZIP_QUOTE_GATEWAY_ADDR` 修改；token 使用
`NETZIP_QUOTE_GATEWAY_NETZIP_RUST_7709_TOKEN`。请求必须显式提供 `trade_date`，服务不会把周末或
节假日取到的上一交易日行情伪装成当天数据。每批携带 `batch_id`，响应中的
`gateway_protocol` 固定标明 Rust 协议。

`POST /api/hqw/publish-worklist` 面向最终的
`NetzipRs -> quote-gateway -> stockScreener` 链路。它从 quote-gateway 读取的只有处理股票代码和
目标交易日，不读取 quote-gateway 缓存行情；行情由 NetzipRs 通过一个 7709 TCP 会话按最多
100 只一批重新获取并投递。默认节点未返回的 SH/SZ 与全部 BJ 会通过第二个单会话
`139.9.43.31:7709` 重试，可用 `NETZIP_TDX7709_FALLBACK_HOST` 修改该节点。`limit` 默认
6000，可先设小值验证；响应包含主/回退批次数、回退恢复数量、最终成功数量及缺失代码。

## Linux 服务安装

release 构建完成后通过项目安装脚本部署：

```bash
bash scripts/install-service.sh
```

systemd 单元为 `netzip-rs.service`，运行文件为
`/home/bin/netzip/netzip_service`，默认监听 `0.0.0.0:16893`。运行参数可写入
`/etc/default/netzip-rs`，例如：

```bash
NETZIP_SERVICE_LISTEN=0.0.0.0:16893
NETZIP_QUOTE_GATEWAY_ADDR=127.0.0.1:16886
NETZIP_QUOTE_GATEWAY_NETZIP_RUST_7709_TOKEN=replace-when-rust-ingest-token-is-enabled
NETZIP_TDX7709_FALLBACK_HOST=139.9.43.31
NETZIP_FULL_PUSH_INTERVAL_SECS=5
NETZIP_FULL_PUSH_IDLE_INTERVAL_SECS=5
NETZIP_FULL_PUSH_BATCH_SIZE=100
NETZIP_FULL_PUSH_LIMIT=6000
NETZIP_FULL_PUSH_WORKERS=8
NETZIP_FULL_PUSH_MODE=poll
NETZIP_NATIVE_PUSH_SESSION_SECS=240
NETZIP_NATIVE_PUSH_AUDIT_INTERVAL_SECS=30
# 仅在显式切换并完成 shadow 验证后设置：
# NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1
```

安装脚本会启用常驻的 `netzip-rs-full-push.service`，并删除旧的
`netzip-rs-full-push.timer`。发布进程全天保持在线：交易时段内默认使用 8 个持久 7709 会话分片拉取全市场行情；午休、闭市和
周末只按 `NETZIP_FULL_PUSH_IDLE_INTERVAL_SECS` 等待，不访问行情与网关发布接口。只有工作表的
`as_of_date` 与 `required_quote_trade_date` 一致时才拉取行情，避免节假日重推上一交易日快照。

常驻进程按“交易日 + 上游行情时间”记录每只股票最近成功发布版本。上游返回相同或更早时间的
快照时计入 `unchanged_count`，不会发给 quoteGateway；一轮没有任何新行情时按
`NETZIP_FULL_PUSH_INTERVAL_SECS` 等待，避免无数据空转和重复源事件。
`NETZIP_FULL_PUSH_WORKERS` 可设置为 `1..=16`，实际 worker 数不会超过当前阶段批次数；接口响应会返回
`elapsed_ms / primary_elapsed_ms / fallback_elapsed_ms / *worker_count / *slowest_batch_ms`，用于持续观察扫描周期。

`NETZIP_FULL_PUSH_MODE` 默认为 `poll`。显式设为 `push` 后，resident runner 才会调用
`POST /api/hqw/push-worklist`：沪深按每连接 100 条建立持久订阅，100 ms 内按标的保留最新记录并
分批发布；故障分片每秒重连并用初始快照补缺，每 30 秒继续执行一次全市场轮询校验，北交所也由
该校验路径覆盖。服务端还要求 `NETZIP_NATIVE_PUSH_PUBLISH_ENABLE=1`，缺少该二次授权时会拒绝
真实发布；请求省略 `publish` 或传 `false` 时仅作影子观测。

首次影子验证或独立入口尚未验收时，部署必须保持自动全推关闭：

```bash
NETZIP_INSTALL_ENABLE_FULL_PUSH=0 bash scripts/install-service.sh
```

该模式只重启 `netzip-rs.service`，并确保 `netzip-rs-full-push.service` 为 inactive。
独立入口验证完成后才允许显式恢复常驻发布服务。

手工触发一次完整推送：

```bash
NETZIP_FULL_PUSH_NOW=1:092500 NETZIP_FULL_PUSH_FORCE=1 NETZIP_FULL_PUSH_ONCE=1 \
  /home/bin/netzip/run-full-push.sh
```

查看自动服务：

```bash
systemctl status netzip-rs.service netzip-rs-full-push.service
journalctl -u netzip-rs-full-push.service -f
```

这层服务目前优先解决两件事：

- 把已经稳定跑通的 `FIN` 解析、`7709` 代码表同步、在线报价、在线 `K线`、`F10` 查询封成可直接调试的本地 API
- 为后续 GUI 界面、脚本调用和调试面板提供统一入口
- `answer-summary` 适合喂 DLL 回包或抓到的 OEM answer buffer；它会尝试解析任何二进制文件，但对 `.FIN` 这类非 OEM 包只会给出无意义摘要
- `fin/query-record` 用来从本地 `FIN` 文件按 `symbol/code` 查真实财务记录
- `fin/getter-specs` 会把当前已确认、派生/固定值、以及尚未解开的 `field_id` 一起返回，Web GUI 会直接显示未解字段列表和更清晰的 getter 名称
- `tdx7709/query-code-table` 会实时走一次 `7709` 代码表同步后按 `code/name` 过滤，只返回真实在线代码表结果
- `tdx7709/live-quote` 会真实连一次 `7709`，完成代码表会话后再发在线 `0x0547` 报价请求
- `tdx7709/snapshot` 是 Linux-first 的组合入口，会把 `live-quote + kline + 可选 F10` 收成一条请求，并支持 `include_live_quote / include_kline / include_f10` 跳过慢阶段
  - 现在会优先复用单个 `7709` 会话，避免重复 `bootstrap/代码表同步`
  - 如果共享会话在中途失败，会自动回退到独立请求模式，并返回 `shared_session_fallback_phases`
- `tdx7709/kline` 会真实连一次 `7709`，完成 bootstrap/代码表会话后请求在线 `K线`
- `tdx7709/f10/categories` 会拉 `F10` 栏目清单，返回 `name / filename / start / length`
- `tdx7709/f10/content` 支持按 `category_name` 或 `filename + start + length` 拉真实 `F10` 正文
- `tdx7709/bootstrap-plan` 会直接返回当前已知的 `probe.hello -> 0x7b00 -> 0x9400 -> 0x9900` 模板和整帧 hex，调试时不需要再手工翻源码
- `linux/pure-rust-mvp` 会把 `7709` 代码表、`0547` 实时行情、在线 `K线` 和可选 `F10` 栏目查询串成一条 Linux-only 验证链，直接返回分阶段成功/失败状态与预览数据
  - 现在支持 `include_sync / include_live_quote / include_kline / include_f10`
  - 现在会优先复用单个 `7709` 会话，失败时自动回退到独立请求模式
  - 跳过阶段时会显式返回 `enabled=false, skipped=true`
  - `overall_ok` 只按本次启用的阶段计算
  - `include_sync=false` 只表示不单独报告 sync phase；如果后续阶段启用，transport 层仍会做内部 `bootstrap/代码表准备`
- `tdx118-dump-compare-plan` 是下一步 `0x118` 明文/密文对照的占位入口，当前只返回计划和待比对工件，不改协议核心
- `blob-compare` 用来直接对照两个 dump 文件，输出字节级 diff、前后缀相等长度，以及 `8-byte` 块相等/不同分布
- `local-2000-log-scan` 用来直接吃 Windows Frida 的本地桥日志，按 `declA` 连续消费 `127.0.0.1:2000` 外层包，识别单次 `recv` 串包、大对象跨 `10240` 块续传、`[recv-large]`、`index=` 前缀、ANSI 颜色和 `NNN:` 行号前缀
- `local-2000-vs-auth-7100` 会把本地 `2000` 完整小包前缀和远端 `7100` 前缀提示放到同一张表里，直接输出共享不变量、本地独有特征、远端独有布局，以及按 `field40 / attr / fixed_overhead / header_len / outer_wrapper` 做出的精确桥接候选
  - 当前样本里，这个精确桥接只命中 `download_file_record / object_type=1`，不会命中 `compressed_zstd / compressed_zstd_dict`
  - 同一套桥接规则现在也已经命中本地初始化大对象的起始包，说明“小包前缀”和“大对象起始包”目前都落在同一族
  - 现在还能直接看到更深一层的锚点：本地和远端 `download_file_record` 的首个 `penc` 共享精确偏移 `60`，并且都落在 `payload boundary` 前 `8` 字节；本地 `hypenc` 则贴着 `payload boundary`
  - 当前远端 `1540 download_file_record` 还能直接扫出 `penc_offsets = [60, 416]`，所以剩余缺口已经收缩成“`hypenc@68` 和第二个 `penc@416` 的业务意义是什么”
  - 新一轮对位也已经把“首个对齐之后的分叉”量出来了：远端第二个 `penc@416` 落在 `payload boundary` 后 `348` 字节，而本地大对象里后续 `penc` 当前落在 `payload boundary` 后 `2 / 190 / 53888` 字节；首个 marker 已对齐，后续 deeper shell 仍未对齐
- `quote-frame-scan` 会附带一个轻量摘要，直接显示 `7709/7719` 这类 bootstrap 标签、代码表请求计数、`phase_order / phase_counts`，以及首个 `0x7b00` 主站校验帧“去掉前置 tag 后”的 `8-byte` 块统计
  - 当前已能自动标出 `post-login.bulk-record-29b-*`、`post-login.fin-143b-*`、`post-login.quote-0547-*`、`post-login.quote-054c-*` 这几段登录后阶段
- Windows 动态取证步骤单独写在 [TDX118_DUMP_GUIDE.md](/home/codes/quoteNetzipRs/netzipapi-rust-demo/TDX118_DUMP_GUIDE.md)
- 如果怀疑之前抓包混入了通达信或其它证券软件的连接，先按 [WINDOWS_PROCESS_CAPTURE_GUIDE.md](/home/codes/quoteNetzipRs/netzipapi-rust-demo/WINDOWS_PROCESS_CAPTURE_GUIDE.md) 做“按目标进程归因”的干净抓包

## Web GUI

现在不再走桌面 `eframe`，而是直接由 `netzip_service` 提供一层浏览器可用的 Web GUI。

启动服务后，直接打开：

```text
http://127.0.0.1:16893/
```

局域网其它机器访问时，用服务所在机器的局域网 IP：

```text
http://<server-lan-ip>:16893/
```

当前这版 Web GUI 直接调用同进程服务里的现有 API，左侧已经按 `行情数据 / 财务数据 / 公告/F10 / 调试工具`
分成独立 TAB，方便直接输入股票代码做真实查询。第一批工作页是：

- `7709 Kline`
- `0x0547 Body Query`
- `7709 Live Quote`
- `7709 F10 Categories`
- `7709 F10 Content`
- `FIN Parse`
- `FIN Getter`
- `FIN Record Query`
- `7709 Sync`
- `7709 Code Query`
- `0x118 Dump Compare`
- `Answer Summary`
- `Quote Frame Scan`
- `PCAP Summary`
- `Stream Analyze`
- `Quote Replay`
- `Proto Probe`

当前 `7709 Live Quote / 7709 Kline / 7709 F10 Categories / 7709 F10 Content`
都是真实在线请求，不是 mock 数据。它们都会先完成 `7709` bootstrap/代码表会话，所以单次调用会比本地
文件解析面板慢一些，但返回的是当前线上真实结果。

当前 `0x0547 Body Query` / `POST /api/quote/0547/query` 除了已确认的 `market / code / count / len`
以外，还会在记录头满足公开 TDX varint 前缀并且 `low/open/price/high` 自洽时，额外返回一组严格校验后的
`quote_head`：`price / last_close / open / high / low`。不满足校验的记录会保持 `quote_head = null`。另外，
`active1_raw` 会始终作为原始 `u16` 暴露出来，但当前还不把它解释成具体业务语义。

当前 `0x0547 Body Query` 和 `POST /api/quote/0547/query` 只返回已坐实字段：

- `market / code / start / len`
- 当 `path` 指向目录时，`POST /api/quote/0547/query` 会自动聚合目录下全部 `*_inflated.bin`
  样本，当前优先用于 `/tmp/quote0547_server`
- 目录聚合查询会额外返回 `source_mode / source_files_total / matched_source_top`，并且每条记录
  自身也会带 `source_name / source_path`
- `active1_raw`
- `record + 15` 上通过严格 `HHMMSS` 校验的可选时间字段；无效值不会强行解释
- 第 1 个额外 varint 当前只保守暴露成 `extra0_raw`；当它命中已经被样本验证过的时间编码范围时，
  才额外返回 `extra0_time_hhmmss`
- 当 `record + 15` 的 `time_hhmmss` 和 `extra0_time_hhmmss` 同时可用时，还会额外返回
  `time_fields_match / time_fields_delta_seconds`，只表达两路已验证时间来源是否一致
- 后 3 个额外 varint 当前只保守暴露成 `extra1_raw / extra2_raw / extra3_raw`，不附会业务语义；
  目前只确认它们是紧跟在 `extra0` 后的连续 raw varint
- 记录头通过严格校验时才返回的 `quote_head(price / last_close / open / high / low)`
- 查询会尝试走一次 `7709` 代码表同步，为命中的记录补 `code_table_name / code_table_pre_close`；
  在未手工指定 `decimal_point` 时，还会自动补 `decimal_point_used` 并在可能时返回 `normalized_quote_head`

当前 `0x0547` 记录字段可以更明确地分成下面几档：

| 字段 | 当前状态 | 说明 |
| --- | --- | --- |
| `market` / `code` / `start` / `len` | 已解析 | 直接来自 `xor93` 后 record 起点与长度切分 |
| `active1_raw` | 仅 raw | 稳定读出 `u16`，但还没命名业务语义 |
| `time_hhmmss_raw` | 已校验 | 只在 `record + 15` 通过严格 `HHMMSS` 校验时返回 |
| `extra0_raw` | 仅 raw | 确认是第 1 个额外 varint |
| `extra0_time_hhmmss` | 条件派生 | 只在 `extra0_raw` 命中已验证时间窗口时派生 |
| `extra1_raw / extra2_raw / extra3_raw` | 仅 raw | 只确认它们是连续 varint，暂不附会语义 |
| `quote_head.active1` | 部分已解析 | 与 `active1_raw` 同源，但当前仍只保留为结构头原值 |
| `quote_head.price / last_close / open / high / low` | 已解析 | 只在公开 TDX varint 头和高低开收自洽时返回 |
| `volume` | 已解析并归一化 | 正的 wire 累计量按 `wire + 1` 转为 OEM 公共累计量；wire `0` 保持 `0` |
| `current_volume` | 已解析 | 当前成交量保持 wire 解码值，不套用累计量归一化 |
| `amount / amount_raw` | 已解析 | 详细诊断接口保留 0547 浮点解码值和原始 `u32`；紧凑接口另在 OEM 公共边界执行已验证的类别相关 `f32` 量化 |
| `normalized_quote_head.*` | 条件派生 | 依赖 `decimal_point` 或代码表精度做缩放 |
| `code_table_name / code_table_pre_close` | 外部补充 | 来自在线 `7709` 代码表，不是 `0x0547` 体内原始字段 |
| `code_table_name_keyword_tag(s)` | 外部补充 | 来自代码表名称的保守标签，不是 `0x0547` 体内原始字段 |
| record 后半段剩余 payload | 未结构化 | 仍不能逐字段、逐字节映射到最终 `OEM_REPORT` |

解析方式当前也是保守的：

1. 先对服务端 `0x0547` 正文做 `zlib` 解压。
2. 再对正文做固定 `xor 0x93`。
3. 通过 `market + 6位代码` 识别 record 起点，再按相邻起点切出 record。
4. 在 record 内部只解析已经被样本和公开 TDX varint 规则共同印证的那部分字段。

- 当 `7709` 代码表名称本身已经足够明确时，查询还会额外补一个非常保守的
  `code_table_name_keyword_tag`；当名字同时命中多个高置信关键字时，还会额外返回
  `code_table_name_keyword_tags[]`，例如 `上证指数ETF => [ETF, 指数]`。这些标签当前只覆盖
  `ETF / 转债 / REIT / LOF / 指数`，它们来源于代码表名称，不是 `0x0547` 体内原始字段
- 当 `normalized_quote_head` 可用时，还会额外返回基于同一条记录直接派生的
  `normalized_change_value / normalized_change_percent`
- 当 `normalized_quote_head` 可用时，还会额外返回基于同一条记录直接派生的
  `normalized_amplitude_percent`
- 当 `normalized_quote_head` 可用时，还会额外返回基于同一条记录直接派生的
  `normalized_open_gap_value / normalized_open_gap_percent`
- 当 `normalized_quote_head` 可用时，还会额外返回基于同一条记录直接派生的
  `normalized_intraday_range_value / normalized_return_from_open_percent /
  normalized_drawdown_from_high_percent`
- 当 `normalized_quote_head.last_close` 和 `code_table_pre_close` 同时可用时，还会额外返回
  `code_table_pre_close_delta / code_table_pre_close_matches`，直接验证这条 `0x0547`
  记录和 `7709` 代码表昨收是否一致
- 当请求里显式提供 `decimal_point(2..=6)` 时，会覆盖自动查到的精度，但不会关闭代码表名称/昨收对照
- 这一步当前已经用 `600000(auto=>浦发银行, pre_close=10.06, dp=2)` 和
  `113638(auto=>台21转债, pre_close=126.57, dp=4)` 的真实样本验证过；派生出的
  `chg=-0.04/-0.397614%` 与 `chg=+0.415/+0.327882%` 也已经过样本核对；同样，
  `last_close - pre_close = 0.0` 这一层也已通过真实样本对齐
- `extra0_raw -> extra0_time_hhmmss` 这一步当前已经用 `600000(-4723=>15:00:03)`、
  `113638(-4721=>15:00:01)`、`300948(5480=>15:30:00)` 的真实样本核对过
- `extra1/2/3` 的当前样本分布也已经验证：`600000 => 2/0/0`，`510030 => -10/2/0`，
  `300948 => 2/0/4100`
- 查询结果现在还会额外给出两个纯 raw 派生标记：`extra3_positive`
  和 `special_zero_bucket`。前者只表示 `extra3_raw > 0`，后者只表示它命中了当前
  已经被 `frame069` 样本验证过的那组“占位/未成形”原始条件
- 查询结果现在还会额外给出一个纯结构分类 `pattern_bucket`，当前只按已验证 raw 条件
  分成 `default_2_0_0 / extra2_2_zero_extra3 / extra3_positive / special_zero_bucket / other_anomaly`
- 查询结果现在还会额外给出更细的 `pattern_subbucket`。当前已验证的子类包括
  `extra2_2_zero_extra3_neg10 / extra2_2_zero_extra3_neg21 / extra3_positive_hint_153000 /
  extra3_positive_hint_153012`，以及 `default_2_0_0` 内部按 `quote_head` 是否存在、
  `extra0_time_hhmmss` 落在 `15:00` 还是 `15:30` 窗口、`time_hhmmss` 是否存在做的
  保守子类，例如 `default_2_0_0_quote_head_hint_1500_time_present`、
  `default_2_0_0_quote_head_hint_1500_time_absent`、
  `default_2_0_0_quote_head_hint_1530_time_present`、
  `default_2_0_0_quote_head_hint_1530_time_absent`
- `POST /api/quote/0547/query` 现在还支持可选 `pattern_subbucket` 过滤，可以直接在
  同一份 `0x0547` body 里只列出某个已验证子桶的样本代码，用于页面和 API 交叉验证
- 同一个查询接口现在也支持可选 `pattern_bucket` 过滤，可以先按
  `default_2_0_0 / extra3_positive / special_zero_bucket / extra2_2_zero_extra3`
  这种大类筛一次，再继续按 `pattern_subbucket` 收窄
- 这个查询接口现在还支持可选 `quote_head_state(with/without)` 和
  `time_presence(present/absent)` 过滤，方便直接把“有头/无头、时间有/无”的记录列出来
- 同一个查询接口现在还支持可选 `prefix3 / decimal_point_filter / state_matrix` 过滤，直接按
  代码前三位、代码表精度或 `with/without quote_head × time_present/absent` 这种联合状态收窄
- 同一个查询接口现在还支持可选 `name_keyword_tag` 过滤，直接按代码表名称里已经验证过的
  `ETF / 转债 / REIT / 指数` 这类安全标签筛样本
- 同一个查询接口现在还会返回命中集自己的摘要分布：
  `matched_prefix3_top / matched_decimal_point_top / matched_pattern_bucket_top /
  matched_pattern_subbucket_top / matched_quote_head_state_top / matched_time_presence_top /
  matched_state_matrix_top / matched_name_keyword_top`，这样一次查询
  就能直接看出命中样本主要落在哪些稳定簇里
- 如果后端再补更细一层 `pattern_subbucket`，它也只会是纯 raw 子分类，例如
  `extra3_positive_hint_153000 / extra3_positive_hint_153012 / extra2_2_zero_extra3_neg10`，
  仍然不附会业务语义
- `POST /api/quote/0547/extra-profile` 会把整份 `0x0547` body 的 `extra0/1/2/3`
  分布、主 pattern、异常样本，以及异常样本按 `prefix3 / market / decimal_point`
  的聚类直接摊开，方便继续逆向；现在还会额外返回按代码表名称关键字归类的异常簇，
  以及异常样本在 `time_hhmmss / extra0_time_hhmmss` 上的时间聚类，但它仍然只返回
  `raw` 值和已验证时间格式，不附会业务语义
- 同一个 `extra-profile` 里现在还会额外返回 `anomaly_pattern_correlations`，把每个异常
  raw pattern 最常见的 `prefix3 / market / decimal_point / time_hhmmss / extra0_time_hhmmss`
  摊开，方便把 `(-10,2,0)`、`(2,0,positive)` 这类簇和样本分布直接对上
- `extra-profile` 现在还会额外返回 `anomaly_pattern_bucket_top`，把异常样本按同一套
  `pattern_bucket` 直接汇总成更易读的结构 bucket
- `extra-profile` 现在还会额外返回 `anomaly_pattern_subbucket_top` 和
  `default_pattern_subbucket_top`，分别摊开异常桶和默认主桶内部的纯 raw 子分类
- `extra-profile` 现在还会额外返回 `default_pattern_subbucket_correlations`，把默认主桶
  内部这些 `1500 / 1530 / none` 风格的保守子类，继续按 `prefix3 / market /
  decimal_point / time_hhmmss / extra0_time_hhmmss` 摊开，方便直接看默认簇的稳定分布
- 同一个 `extra-profile` 现在还会返回 `default_state_matrix_top`，把默认主桶按
  `with/without quote_head × time_present/absent` 的联合状态直接摊开
- `extra-profile` 现在还会显式返回 `default_no_quote_head_count /
  default_no_quote_head_subbucket_top / default_no_quote_head_examples`，只把当前已抓到的
  默认桶 `quote_head=null` 样本摊开，不额外附会业务语义
- 同一个 `extra-profile` 现在还会显式返回 `default_quote_head_count /
  default_quote_head_time_present_count / default_quote_head_time_absent_count /
  default_quote_head_subbucket_top / ...examples`，把默认主桶里更稳定的
  `quote_head + time_present/time_absent` 结构直接摊出来
- `extra-profile` 现在还会补 `default_quote_head_name_keyword_top` 和
  `default_no_quote_head_name_keyword_top`，直接看默认簇在 `ETF / 转债 / REIT / 指数`
  这些已验证安全标签上的分布
- `extra-profile` 现在还会单独给出 `anomaly_extra3_positive_count` 和
  `anomaly_special_zero_bucket_count/examples`，把 `frame069` 里那组
  `0/0/0 + active1=0 + time=00:00:00 + pre_close=0` 的占位样本从普通 anomaly 里拆出来

当前 `7709 Code Query` 还会直接返回代码表里的 `decimal_point` 和 `pre_close`。这对校验
`0x0547` 价格缩放很关键：例如 `600000` 会返回 `decimal_point = 2`、`pre_close = 10.06`，
而转债样本 `113638` 会返回 `decimal_point = 4`、`pre_close ≈ 126.57`。

顶部还提供：

- `Health`
- `Capabilities`

这样后面继续补功能时，不需要再维护一套桌面壳和一套网页壳，服务和调试界面是一体的。

其中 `FIN Getter` 面板会把 `getter-specs` 的 `unresolved_field_ids` 直接摊开显示，`FIN Record Query` 面板只查本地 `FIN` 的真实记录，`7709 Live Quote` 面板会真实连一次 `7709` 完成代码表会话后再发在线 `0x0547` 报价请求，`7709 Code Query` 面板会实时走一次在线代码表同步后再过滤，`Quote Frame Scan` 面板会直接显示 bootstrap 摘要和 `0x7b00` 对齐后的块统计，`7709 Sync` 面板还多了一个 `Show Bootstrap Plan` 按钮，`0x118 Dump Compare` 面板则能直接对两个 dump 文件做偏移/长度级比较。
后续一旦接到 Windows 动态 dump，直接在 `0x118 Dump Compare` 面板里对照 `0x10066b60` 的 `0x118` 明文和出网前 `payload[2..]` 即可。

## 纯 Rust/Linux 逆向辅助

当前仓库除了调试 example，还补了一个 Linux-first 的业务入口：

```bash
cargo run --bin netzip_linux -- --help
cargo run --bin netzip_linux -- snapshot SH600000
cargo run --bin netzip_linux -- live-quote SH600000 SZ000001
cargo run --bin netzip_linux -- snapshot 600000 --kline-type 1m --kline-count 5
cargo run --bin netzip_linux -- kline SH600000 --kline-type 1d --count 10
cargo run --bin netzip_linux -- snapshot SH600000 --skip-f10
cargo run --bin netzip_linux -- snapshot SH600000 --skip-live-quote --skip-f10
cargo run --bin netzip_linux -- f10-categories SH600000 --limit 5
cargo run --bin netzip_linux -- f10-content SH600000 --category-name 公司概况
cargo run --bin netzip_linux -- sync-code-table --out /tmp/tdx7709_codes.csv
```

`netzip_linux` 当前不是“完整替代 Windows 主链”的成品，而是纯 Rust + Linux 路线的统一探针：

- `snapshot` 会组合跑一次 `实时行情 + K线 + F10分类`
- `snapshot` 现在支持 `--skip-live-quote / --skip-kline / --skip-f10`
- `snapshot` 现在会优先复用单个 `7709` 会话，并在 JSON 里返回 `shared_session_*` 状态
- 如果共享会话中途失败，`snapshot` 会自动回退到独立请求模式
- `live-quote` 支持直接拉多个标的的 `0547` 行情，并返回 `requested_symbols / transport_symbols / unmatched_symbols`
- `kline` 支持单标的直接拉在线 `K线`
- `f10-categories` 支持直接列出栏目清单
- `f10-content` 支持按 `category-name` 或 `filename + start + length` 直接拉正文；如果先按栏目名定位，会优先复用同一个 `7709` 会话完成“先查栏目再取内容”
- `sync-code-table` 会直接完成 `7709` 代码表同步并输出 JSON 摘要
- 当某一子能力失败时，它会保留其它已成功部分和错误字段，适合当前阶段持续校对 Linux 直连能力

除此之外，仓库里还保留了一批更底层的不依赖 DLL 的协议探测工具：

```bash
cargo run --example proto_probe -- --help
cargo run --example stream_analyze -- captured_windows_traffic/client_to_server.raw --bin
cargo run --example pcap_reassemble -- /tmp/netzip_full_all.pcap --src 192.168.3.38:2697 --dst 39.108.103.69:7100 --out /tmp/flow.bin
cargo run --example pcap_flow_timeline -- /tmp/netzip_full_tcp.pcap --host 192.168.3.38 --ports 6100,7100,7708,7719,14017
cargo run --example dll_call_xrefs -- ./netzip_api_bin/NetzipAPI/StockC++/Stock64.dll 0x1800129a0 0x1800128c0
cargo run --example quote_frame_scan -- /tmp/flow_9278_7719.bin
cargo run --example quote_replay -- /tmp/flow_9278_7719.bin 110.41.14.158 7719 --segments '0:582:0,582:260:420,842:58:550,900:582:2010,1482:260:444'
cargo run --example quote_tag_extract -- /tmp/timeline_7709_server_7171.bin --out-dir /tmp/quote0547_server
cargo run --example quote_0547_scan -- /tmp/quote0547_server
cargo run --example tdx7709_sync -- --out /tmp/tdx7709_codes.csv
cargo run --example tdx_fin_dump -- /tmp/full_sh.FIN --csv /tmp/full_sh.csv
```

对完整无截断包，`stream_analyze` 现在还会自动：

- 在一整条 TCP 重组流里自动切出多个 `网络包`
- 识别重复包与唯一包数量
- 提取 `数据` 字段中的 `ZSTD` 帧
- 解压后继续显示里层对象头和 UTF-16 字段摘要
- 如果解压失败，继续打印原始 UTF-16 / ASCII 线索

`quote_frame_scan` 则专门针对已经重组好的行情 raw 流：

- 自动识别 `7719` 客户端 `10` 字节头请求帧
- 自动识别 `7708/7709/7719` 服务端 `16` 字节头回复帧
- 对标准 `zlib` 正文直接解压
- 自动识别 `7709` 的 `0x0450` 代码表块请求
- 对 `29` 字节代码表记录直接打印首尾代码和中文名称
- 提取 `tdxlevel2`、`www.tdx.com.cn` 这类 ASCII 线索

`quote_replay` 用来把已抓到的 raw 流重新发回行情节点，验证 Linux 侧能否直接复现业务通道：

- 支持整包一次性重放
- 支持按 `offset:len:delay_ms` 分段重放
- 即使中途 `RST / Broken pipe` 也会保留已经收到的部分 reply

`quote_tag_extract` 用来从重组后的 `client10/server16` raw 流里直接提取指定 `tag` 的帧，默认就是 `0x0547`：

- 客户端流会把匹配的请求 payload 单独落文件
- 服务端流会把匹配的压缩 body 落文件；如果是标准 `zlib`，还会继续导出解压后的正文
- 这很适合把 `7709` 长会话里的大 `0x0547` 正文和尾段 `0x2902/0x2a02` 小 ACK 分开，后面直接拿去和 DLL dump 做对照

`quote_0547_scan` 则专门扫提取出来的 `0x0547` 正文，输出目前最有用的几个结构信号：

- 文件长度
- 明文数字串数量
- 高频 `0x9393` token 次数
- 最大同字节连续 run
- 长 `0x93` run 的数量、首批位置和间距分布
- 候选 record stride 的 `best-prefix / consensus-score / tail-fit`
- 按公开 TDX `parse_price` 规则盲扫时的 varint 长度分布
- top `u16` 词频
- 多份正文之间的公共前后缀长度

`tdx7709_sync` 则是当前第一条真正脱离 DLL、直接用 Rust/Linux 跑通的业务示例：

- 可复用协议逻辑已经收进 `src/tdx7709.rs`，crate 会直接导出 `sync_code_table` / `write_code_table_csv`
- 固化了 `7709` 的 `3` 个 bootstrap 请求和 `50` 个 `0x0450 + bucket + offset` 代码表块请求
- 直接连接公网 `7709` 节点，收 `53` 个 `server16` 回复帧
- 自动解 `zlib`、解析 `29` 字节代码表记录并导出 `CSV`
- 名称字段按固定 `8` 字节 `GBK` 槽做容错解码，允许尾部遗留半个双字节字符而不中断整次同步

`tdx_fin_dump` 则把已经确认的 `FIN` 文件格式落成了可复用库接口：

- 解析 `full_sh.FIN / full_sz.FIN` 这类 `magic(0x223fd90e) + record_size + records...` 文件
- 直接还原 raw finance record 前 `12` 字节 `symbol/key` 槽，以及后面的 `time / baoGao / shangShi + 48` 个 float 字段
- crate 会直接导出 `parse_fin_bytes` / `parse_fin_file` / `write_fin_csv`，以及 `SH_FIN_URL / SZ_FIN_URL`
- 已内置 `quarter_from_bao_gao`、`symbol -> market/code` 辅助，以及 `CSV` 导出
- 还补了一层 `FIN getter` 映射：
  - `fin_getter_specs()` 只列出已坐实的 `field_id -> 字段`
  - `fin_getter_gap_specs()` / `fin_getter_unresolved_ids()` 直接暴露当前 9 个未解 id：`0x2d / 0x2e / 0x2f / 0x31 / 0x32 / 0x37 / 0x38 / 0x39 / 0x45`
  - `TdxFinRecord::getter_value(field_id)` 对这些 gap 继续返回 `None`
- 实测直接解析官方 `full_sh.FIN / full_sz.FIN` 通过：前者 `2412` 条、后者 `3012` 条，`record_size` 都是 `224`，且 `trailing-bytes = 0`

`pcap_flow_timeline` 用来回答“这条流是不是从会话起点开始抓到的”：

- 对经典 `pcap` 里的 TCP 流按时间去重、归并
- 标记 `started-with-handshake / midstream-first-packet-had-payload / connect-attempt-only`
- 很适合快速排除 `SYN` 重试、纯保活和中途接入的业务流

示例：

```bash
cargo run --example proto_probe -- --read-secs 2
cargo run --example proto_probe -- --payload '股票数据?请求=登录&模块=认证&账号=168&密码=168&自动升级=稳定版&版本=20221120&等待=3000&编号=0' --utf16le --nul --read-secs 2
```

当前逆向结论见：

- `PROTOCOL_NOTES.md`
- `CAPTURE_GUIDE.md`

另外还提供了一个经典 `pcap` 摘要工具，用来检查抓包是否被重复/截断：

```bash
tcpdump -r captured_windows_traffic/capture_netzip.pcapng -w /tmp/netzip_6100.pcap 'tcp port 6100'
cargo run --example pcap_summary -- /tmp/netzip_6100.pcap
```

旧的 Windows `pktmon` 样本已经确认存在两类副作用：

- 同一条 TCP 包会重复 4 次
- 每条大包只保留前 `128` 字节，导致应用层只剩 `74` 字节前缀

所以旧样本只足以确认 `网络包` 外层头，不足以恢复完整请求体；新的 `--pkt-size 0` 样本已经可以做完整 TCP 重组。

如果改用 `pktmon start -c --pkt-size 0` 抓完整 payload，并用 `pcap_reassemble` 先做流重组，当前工具已经能自动区分：

- `6100` 上的认证服务器 `测速` 请求/回复
- `7100` 上的认证 `登录` 请求
- `7100` 返回的 `数据 | 下载文件`
- 以及后续带 `penc / ZSTD字典 / Tdx_Encrypt` 线索的包
- `14017` 上已经变成高熵随机二进制、无法再按 `网络包` 直接切开的业务流

当前已经确认：

- `419 -> 345` 这组 `6100` 小包是 `请求=测速`，不是后续登录/行情请求
- `2026-03-28` 的 `capture_timeline_run` 里，`7100` 和 `7709` 都已经从 `SYN` 开始抓到完整会话
  - 这轮样本足够继续做离线协议分析
  - 但它不是“完美冷启动、单实例、全程按 PID 绑定”的唯一归因样本，所以这里把它当强协议证据，不单独当唯一归因证据
- `7100` 上的 `611` 字节包是 `认证 | 请求=登录`
  - 登录体里已能直接读到：
    - `账号=168`
    - `密码=168`
    - `运营商名称=泉州移动`
    - `模块=股票客户端`
    - `网际风.exe / Stock.dll / Stock.字典`
- `7100` 还至少能看到 `3` 条独立短 `测速` 会话：
  - `413 -> 341`
  - `413 -> 345`
  - `415 -> 341`
- `7100` 上的 `1540` 字节包是 `数据 | 下载文件`
  - 文件名里明确出现 `系统\\通达信股票服务器.ini`
  - 包头字段里能直接读到：
    - `请求 = 系统\\通达信股票服务器.ini`
    - `来源 = 认证服务器`
    - `名称 = 系统\\通达信股票服务器.ini`
    - `压缩 = 无压缩`
  - 正文里已能完整解到：
    - `账号 = NetCardMac`
    - `密码 = l123321`
    - `主端口 = 7709`
    - `次端口 = 7712`
    - `客户ID = 1031`
  - 这条 `1540` 下载包当前还能直接定位到 `penc_offsets = [60, 416]`
    - 首个 `penc@60` 与 `header_len = 68` 对位后，正好等于 `payload boundary - 8`
    - `市场 = SH;SZ`
  - 并可见多组 `7709` 行情服务器地址：
    - `124.70.183.173`
    - `124.71.163.106`
- `7709` 的完整长会话里已经直接观测到：
  - bootstrap：`0x7b00 -> 0x9400 -> 0x9900`
  - 代码表阶段：`0x6d00 / 0x6e00`
  - 批量请求阶段：`0x7600 / 0x7502`
  - post-login 行情阶段：`0x2900 / 0x2a00` 以及后续 `0x2902 / 0x2a02`
  - 其中 `0x2900 / 0x2a00 / 0x2902 / 0x2a02` 的内层 `tag` 都已经直接看到是 `0x0547`
  - 就 `capture_timeline_run` 这条长流而言：
    - 客户端先连续发 `10` 个 `0x7600`
    - 然后 `frame[63]` 首次出现 `0x2900 / tag=0x0547`
    - `frame[65]` 紧跟 `0x2a00 / tag=0x0547`
    - `180.101.48.175`
    - `120.195.71.160`
    - `122.96.107.241`
- 后续 `field44 = 12` 的包不是标准裸 `ZSTD`，而是带 `penc / ZSTD字典 / Tdx_Encrypt` 线索的后续壳
- `14017` 客户端/服务端重组流从字节 `0` 开始就是高熵二进制：
  - 没有 `网络包`
  - 没有 `OEM_DATA_HEAD`
  - 当前 `stream_analyze` 的摘要大约是 `entropy≈7.93~7.98`
- `7719` 行情流已经能稳定按应用层帧切开：
  - 客户端流 `1742` 字节可切成 `5` 帧：
    - `0x440c / 0x2b02 / 0x0100`，`80` 个代码，请求区间 `000973..001313`
    - `0x6b0c / 0x0a07 / 0x0200`，`34` 个代码
    - `0x450c / 0x2a02 / 0x0100`，`4` 个指数代码：`399002 / 399003 / 999998 / 999997`
    - `0x460c / 0x2b02 / 0x0100`，`80` 个代码，请求区间 `001314..002034`
    - `0x6c0c / 0x0a07 / 0x0200`，同一组 `34` 个代码
  - 客户端这组帧的规则已经明确：`bytes[6..8] == bytes[8..10] == payload_len`
  - 服务端流 `20665` 字节可切成对应的 `5` 帧，固定头是 `b1 cb 74 00`
  - 服务端规则也已明确：
    - `bytes[12..14] == compressed_len`
    - `bytes[14..16] == original_len`
    - 大回复正文是标准 `zlib`
  - `zlib` 解压后正文前 `5` 字节是小头：
    - `status = 0`
    - `count = 80 / 34 / 80 / 34`
    - 后面紧跟对应代码的逐股记录
- `7708` 服务端流也复用了相同的 `16` 字节回复头：
  - 当前已切出 `75 / 908 / 420 / 908 / 420` 五帧
  - 但正文还不是标准 `zlib`
  - 客户端除首个 `912` 字节请求外，后续帧头还没完全还原
- `7709` 也属于同一套 `client10/server16` 行情协议家族：
  - 小握手流 `2655 -> 7709` 是 `12 -> 143`
  - 服务端 `143` 字节回复可解出 `20260328_085435`、`ININ Cloud V6.72`、`/tdx/hostl/`
  - 主数据流 `2400 -> 7709` 是 `1147 -> 445457`
  - 客户端帧里可直接看到 `tdxlevel2`
  - 后续 `50` 个 `6` 字节请求体都是 `0x0450 + bucket + offset`，按 `1000` 条一块请求代码表
  - 服务端对应返回 `50` 个 zlib 块，每块都是 `1000 * 29` 字节记录
  - 这些记录已经能直接解到中文名称，例如首块从 `395001d 主板Ａ股` 到 `002111d 威海广泰`
  - Linux 侧直连回放结果也已经分化清楚：
    - `12` 字节小握手原样回放会稳定收到 `143` 字节回复
    - `1147` 字节主流如果一次性整包发完，服务端直接 `EOF`，回复 `0`
    - 但按 `53` 个 client10 帧边界分段回放后，可以稳定拿回完整 `445457` 字节回复
    - 按帧比对时，`53` 帧里只有第 `1` 个小元数据帧不同；后面的代码表大块与抓包原样一致
  - 现在已经有一个 Linux 原生同步示例 `tdx7709_sync`：
    - 直接按同样的 `53` 个请求帧去拉取 `7709`
    - 稳定拿回 `445457` 字节、`53` 帧回复
    - 去重后导出 `49040` 条代码记录到 CSV
    - 头尾抽样是：
      - `000001d 上证指数`
      - `999999d 上证指数`
    - 样本里还包含一些有趣代码，例如 `880654d ChatGPT`
  - 对完整 `pcap` 做首帧归并后，当前 `7709` 客户端流只剩两类模板：
    - `20` 条 `347/1147` 字节主会话，首个 `292` 字节 bootstrap 完全一致
    - `5` 条 `12` 字节探活/测速流，正文固定为 `0c0100000000020002001500`
  - 2026-03-28 新补的按时间线重组样本 `capture_timeline_run.pcapng` 又把这条链压实了一层：
    - 注意：截至 `2026-03-29`，当前仓库快照里已经找不到 `captured_windows_traffic/capture_timeline_run.pcapng` 原文件；这里关于 `7128 / 14124 / 14715 / 14717` 的结论来自当时已经完成的离线分析记录，而不是今天仍可直接复跑的本地样本
    - 在该样本里，真正从 `SYN` 开始抓到的目标端口只有 `7100` 和 `7709`
    - `7100` 不再只是零散包头，而是能完整看到 `测速` 短会话和 `登录 -> 下载 系统\\通达信股票服务器.ini -> penc / ZSTD字典` 长会话
    - 当前 sample pcap 里这条 `7100` 链已经可以直接按会话矩阵输出：
      - `192.168.3.38:2695 <-> 39.108.103.69:7100` 是 `419 -> 345` 的 `auth_probe`
      - `192.168.3.38:2697 <-> 39.108.103.69:7100` 是 `611/271/333 -> 443/1540/395` 的 `auth_login`
      - 对应调试接口是 `GET /api/debug/auth-7100-flow-matrix`
      - 现在同一路径也支持 `POST /api/debug/auth-7100-flow-matrix`，请求体只要给 `{ "path": "/abs/path/to/file.pcap" }`
      - 也可以直接给 `.pcapng`；服务端会调用系统 `tcpdump` 临时转换成 classic `pcap`
      - `POST` 还支持可选过滤：
        - `local_endpoint`
        - `session_role`
        - `source_endpoint`
        - `destination_endpoint`
      - 这条矩阵输出里，现在还会同时带外层和解压后内层的前缀提示：
        - `field32 / field36` 在当前样本里都是重复的包长声明
        - `field40` 在已观测 `6100/7100` 样本里恒为 `2`
        - `field56 - field44` 在已观测 `网络包 / 认证 / 数据` 记录里都固定为 `28`
        - 因而当前代码侧已经把它们先按 `packet_len_declared / packet_len_duplicate / field40_constant / payload_len_hint / object_span_len_hint / fixed_overhead_len_hint` 输出，供后续继续校对
    - 这条 `7100` 登录长会话里的客户端 `271 / 333` 两个壳包，现在都已经在仓库 dump 集合里找到精确命中的 Windows 样本
      - `271` 对上 `spawn_dump_271_1774689981.bin` / `spawn_dump_271_1774690043.bin`
      - `333` 对上 `spawn_dump_333_1774689982.bin` / `spawn_dump_333_1774690043.bin`
      - 这一步已经落成调试接口 `GET /api/debug/auth-7100-client-shell-correlation`
    - 现在又新增了一个双向总览接口 `GET /api/debug/auth-7100-shell-correlation`
      - 它会把 sample pcap 里 `7100` 登录会话的双方向壳包统一列出来
      - 同一路径现在也支持 `POST /api/debug/auth-7100-shell-correlation`，请求体只要给 `{ "path": "/abs/path/to/file.pcap" }`
      - 也可以直接给 `.pcapng`；服务端会调用系统 `tcpdump` 临时转换成 classic `pcap`
      - `POST` 还支持可选过滤：
        - `local_endpoint`
        - `session_role`
        - `source_endpoint`
        - `destination_endpoint`
      - 当前已覆盖：
        - 客户端 `611 / 271 / 333`
        - 服务端 `443 / 395`
      - `271 / 333` 仍然能走 exact dump 对位
      - 现在 match 明细里还会同时带 `zstd_block_body_compare`
        - 可直接看到块体级的 `equal_prefix_len / equal_suffix_len / diff_bytes / first_diff_offset`
        - 当前 `271` 的同长度近邻已经能明确成“只差 `1` 字节，且首个差异位于 zstd-like 块体偏移 `58`”
      - `443 / 395` 因为仓库里还没有同长度 dump，目前只走近邻长度 fallback；当前首个近邻样本都落在 `dump_411_1774689402.bin`
      - `220 / 273` 这组仍只在 timeline 文档里出现，当前 sample pcap 还没覆盖到，所以暂时不会出现在这个接口里
    - `7709` 在整份 timeline 里一共出现 `84` 条从 `SYN` 开始的真实流，分成 `4` 轮；每轮都是 `1` 条 `12 -> 143` probe 加 `20` 条主流
    - `7709` 的长会话则直接出现了 `bootstrap -> 0x6d00/0x6e00 代码表 -> 0x7600 -> 0x2900/0x2a00(tag=0x0547) -> 0x2902/0x2a02(tag=0x0547)` 的真实顺序
    - 客户端 `0x0547` 请求体现在也已被线上样本直接拆开：`1104 = 4 + 100 * 11`，单项就是 `market_flag + 6位代码 + u32 token`
    - 并且能看到 token 的闭环变化：首批 `0x2900/0x2a00` 大多还是 `0`，到尾段 `0x2902/0x2a02` 已经变成全量非零，这和 DLL 里“回包写回 token，再用于续订”正好对上
    - 这使得 `0x0547` 不再只是 DLL 静态逆向和旧抓包之间的强推断，而是已经在完整 `7709` 会话中被直接观测到
    - 再对照本地 [rustHq packet.rs](/home/codes/crates/rustHq/src/packet.rs) / [rustHq parser.rs](/home/codes/crates/rustHq/src/parser.rs) 这套公开 `7709` 实现去试后，现在线索已经推进到“同家族的逐股 record 流”，而不是“完全黑盒”：
      - 公开 TDX quote 请求是 `2 + N * 7`，reply 靠 [parser.rs](/home/codes/crates/rustHq/src/parser.rs#L619) 这种 `6+7` bit 的 signed-varint `parse_price` 继续解
      - 原始 `0x0547` 大正文里，确实仍然搜不到直接的明文 `6` 位代码
      - 但固定 `xor 0x93` 后，`quote_0547_scan` 已经能稳定扫出真实 record 头：
        - `frame063 -> count=100, records=100, first=[SH600000,SH600004,SH600006,SH600007,SH600008]`
        - `frame065 -> count=100, records=100, first=[SZ000815,SZ000816,SZ000818,SZ000819,SZ000820]`
        - `frame067 -> count=99, records=99, first=[SH113638,SH113640,SH113643,SH113644,SH113646]`
        - `frame069 -> count=100, records=100, first=[SH688690,SH688691,SH688692,SH688693,SH688695]`
        - `frame071 -> count=100, records=100, first=[SZ300945,SZ300946,SZ300947,SZ300948,SZ300949]`
      - 同时 record 间距不是固定长度，而是明显的变长分布：
        - `frame063/065` 主要集中在 `113..118`
        - `frame069/071` 主要集中在 `105..112`
        - `frame067` 还出现一批 `143..148`
      - 这说明 `0x0547` 服务端大正文更像“公开 TDX quote/varint 家族上的另一种逐股对象流”，而不是已经能直接按公开 `0x053e` 规则逐股落表的明文记录
      - 这一步已经单独落成了 `POST /api/debug/quote-0547-decode`
        - 当前只返回已经坐实的字段：`xor93_count / filtered_records / market / code / start / len`
        - 实测 `frame063` 返回 `count=100`、首批 `[SH600000,SH600004,SH600006,SH600007,SH600008]`
        - 实测 `frame065` 返回 `count=100`、首批 `[SZ000815,SZ000816,SZ000818,SZ000819,SZ000820]`
        - 这一步故意不返回价格类字段，避免把还没证实的映射包装成“已解析成功”
- Linux 直连回放实验已经做过：
  - `7719`
    - 原样一次性回放 `1742` 字节：连接成功，但 `0` 字节回复
    - 按抓包节奏回放 `582 -> 260 -> 58 -> 582 -> 260`：第二帧后被 `RST`
    - 单独回放 `7709` 的 `12` 字节探活帧时，会稳定收到 `139` 字节回复：
      - 同样是 `server16 + zlib`
      - 解压后可见 `20260328_085110`、`Level2  V6.71`、`/tdx/hostl/`、`FA163EE04559`
    - 但“先探活，再发 `292` 字节主站校验帧”时，第二步仍然返回同一个 `96` 字节错误包：
      - `行情主站校验,无数据参数`
    - 这说明 `7719` 和 `7709` 共享探活/选站协议，但探活阶段不会给 `292` 字节主站校验补足参数
  - `7708`
    - 按抓包节奏回放后，第一帧 `912` 字节会收到一个 `20` 字节短回复
    - 短回复内容是：
      - `b1 cb 74 00 0e 02 ab 01 00 00 b9 0b 04 00 04 00 00 00 00 00`
      - 也就是 `server16` 外层头 + `4` 字节全零正文
    - 在后续 `128 + 155` 字节请求后被 `RST`
    - 单独回放同一个 `12` 字节探活帧时没有任何回复
    - 先探活再发 `912` 字节首帧，会比原始重放更早被 `RST`
  - 这说明 Linux 机器可以直接连上行情节点，但真正的 `7708/7719` 行情会话仍依赖更早的登录/握手状态，不是单靠重放业务包就能复现
- `pcap_flow_timeline` 已确认当前完整抓包里的关键时序：
  - `9278 -> 110.41.14.158:7719` 的首个可见包就是 `582` 字节 payload，没有 `SYN`，属于 midstream
  - `1871 -> 221.236.15.16:7708` 的首个可见包是纯 `ACK`，随后才出现 payload，也属于 midstream
  - `2925 -> 123.125.108.156:7708` 只有两次 `SYN` 重试，没有任何 payload，不是真正业务会话
  - 在首个 `7719` payload 出现前，抓包里唯一完整建连成功的 `14017` 流是 `2923 -> 183.236.97.134:14017`
  - 把这条 `2923` 流完整回放后再立刻回放 `7719`，结果仍然是在第二帧后被 `RST`
  - 所以当前最像缺口的不是“另一条已抓到的 `14017` 前置流”，而是 `7719/7708` 自身更早的会话建立包没有被抓到
- 目前已重组出来的多条完整 `14017` 建连流也都保持同一特征：
  - 从字节 `0` 开始就是高熵随机二进制
  - 彼此之间、以及和当前 `7708/7719` 客户端 raw 流之间，都找不到公共的 `4/6/8` 字节窗口
  - 这更像“每会话随机化/加密”的二进制通道，而不是带固定头的静态长度协议

如果继续做 DLL 逆向，`dll_call_xrefs` 可以快速枚举 `.text` 里对某个函数地址的所有直接 `call rel32` 调用点，适合追 `Ask -> 网络包 -> send` 这类调用链。

`dll_data_xrefs` 也已经补了“附近字符串”输出，适合直接读中文上下文。例如现在可以很快确认：

- `0x18006f388` 一簇是 `提示信息 / 节点 / 等号 / 注释 / 说明`
- 同区附近还有 `GET /`、`HTTP`
- 这组更像 HTTP/配置解析器，不像真正的 `Tdx_Encrypt` 解密核心

另外，`Stock.dat` 这条 `32-bit` 分支已经能直接读到 `CTdxClient` 的 UTF-16 模板串，对 `7709` 的前置链语义非常有帮助。当前已确认能对上这些中文说明：

- `%s(股票备用)测速包`
- `申请服务器信息`
- `登记客户端版本号，获得通行证信息`
- `网络验证成功`
- `正在读取代码表`
- `验证用户合法性`

这说明 `7709` 的 `12` 字节探活、`347` 字节 bootstrap、以及后续代码表同步，至少在 DLL 内部文案层面已经能和实际线上行为对应起来。

`dll_data_xrefs` 现在也已经支持 `PE32` 的常见绝对地址引用，不再只能看 `PE32+ / RIP-relative`。拿 `Stock.dat` 实测后，当前已经能直接定位到 `7709` 三段 bootstrap 的构造点：

- `0x1005ce67`
  - 通过 `0x10066c90` 构造 `0x7b000b`
  - 对应首个 `292` 字节 `0x010c / 0x7b00 / ... / 0x000b`
- `0x1005db3f`
  - 通过 `0x10066c90` 构造 `0x94000d`
  - 对应第二个 `13` 字节 `0x020c / 0x9400 / ... / 0x000d`
- `0x1005de15`
  - 通过 `0x10066c90` 构造 `0x990fdb`
  - 随后在 `0x1005de50` push `tdxlevel2`
  - 对应第三个 `42` 字节 `0x030c / 0x9900 / ... / 0x0fdb`

同一轮静态分析还确认了两点：

- `0x10066c90` 是通用 `client10` 头构造器，负责把 `0x7b000b / 0x94000d / 0x990fdb` 这类模板写进内部小结构
- `0x10064c40` 才会在发送阶段回填 session 序号和总长度，所以线上看到的 `0c 01 00 7b ...`、`0c 02 00 94 ...`、`0c 03 00 99 ...` 是“构造器 + 会话态”共同产物，不是单个静态模板直接拷出去

这一轮又把首个 `292` 字节校验帧往前推进了一层：

- `dll_data_xrefs` 现在会直接打印目标地址的 `hex-head`，查 `.rdata` 常量时不必再手工来回 `objdump/xxd`
- 通过正确的 `VA -> section -> raw` 映射，已经确认：
  - `0x10115218` 是 `GBK` 的 `中信证券`，后面跟 `3` 个 `0x00`
  - `0x10115224` 是 ASCII `NetCardMac`
- 这正好和 `0x10066b60` 的写法对上：
  - `+0x65` 固定拷 `中信证券`
  - 比较第一个参数是否等于 `NetCardMac`
  - 若相等，则把 `+0xb1` 那段内容再复制到 `+1`
- `0x1005cea9` push 的 `0x10103258` 也已经确认是 UTF-16 `Tdx_Encrypt`
  - 它会进入 `0x1007b7c0 -> 0x1002bf90`
  - `0x1002bf90` 再通过 `0x1002da10` 初始化一个 `0x1414` 字节上下文，把原始 `0x118` body 喂进这条 `Tdx_Encrypt` 路径
- 强推断：
  - 线上首帧那段高熵 `0x118` body 不是 `0x10066b60` 的明文直接出网，而是经过 `Tdx_Encrypt` 处理后的等长结果
  - 当前抓包里连续重复的 `8` 字节块，更像“按 `8` 字节块处理”的结果，而不是裸结构体明文

这一轮又把首帧明文参数的来源钉住了：

- `0x10049530` 已经能确认是 IPv4/端口格式化 helper：
  - `fmt0 = "%d.%d.%d.%d"`
  - `fmt1 = "%d.%d.%d.%d:%d"`
- `0x1004f5d0` 在成功选中当前服务器对象后，会把该对象的地址字段格式化到 `ctx + 0x6490f4`
  - 所以 `0x6490f4` 不是抽象 token，而是当前服务器的 IPv4 文本（必要时带端口）
- `0x1002e940` 则直接给出 `0x6490e8 + 0xc4 / +0xe4` 的语义：
  - 它把这两个字段代入格式串 `&account=%s&password=%s&version=%d&localIp=%u&volumeId=%u`
  - 也就是说，这两个偏移就是“账号 / 密码”字符串槽
- 因此 `0x1005ce72 -> 0x10066b60` 这条首帧 builder 的三个核心输入现在可以写成：
  - `arg1 = 当前服务器对象.account`
  - `arg2 = 当前服务器对象.password`
  - `arg3 = 当前服务器 IPv4 文本`

一个额外的状态机线索也对上了：

- 在首个 `292` 字节 `0x010c/0x7b00` 发完之后，`0x1005cf60` 会立刻调 `0x1004fce0`
- 随后再调 `0x1004fe40`
- 这两步看起来是在“根据当前服务器对象实例化/筛选后续连接项”
- 成功后才继续往下走下一阶段，而失败则会走 `0x1004f690` 的错误日志分支

同一条 `Tdx_Encrypt` 封装也不只服务这一个首帧：

- `0x1006daa7` 是另一条兄弟调用链
  - 先 `0x10066c90(0x26ba, len=0x50)`
  - 再 `0x1006dfa0`
  - 最后同样走 `0x1007b7c0(Tdx_Encrypt)`
- 这说明 `0x1007b7c0 / 0x1002bf90` 更像通用“加密后发送”包装，而不是某个单独业务包的特例

继续往下追之后，`Stock.dat` 的登录 bootstrap 已经能从首帧一路连到“读取代码表”：

- 这轮新增了一个导入表辅助工具：
  - `cargo run --example dll_imports -- ./netzip_api_bin/NetzipAPI/StockC#/Stock.dat 0x100f22cc 0x100f22ec 0x100f22f0 0x100f22d0`
  - 它会直接把 IAT 地址映到导入符号，省掉手算 thunk 偏移
- 结合它和 `objdump`，现在已经能确认：
  - `0x100f22c8 = WSACreateEvent`
  - `0x100f22cc = WSAEventSelect`
  - `0x100f22d0 = WSACloseEvent`
  - `0x100f22ec = WSAWaitForMultipleEvents`
  - `0x100f22f0 = WSAEnumNetworkEvents`
- 因而 `0x1004fce0 / 0x1004fe40` 的语义可以写得更硬：
  - `0x1004fce0` 会把候选服务器对象实例化成 `ctx + 0x254 + n * 0x1016c` 里的临时连接项
  - 每个连接项会复制源对象的两段地址相关结构：
    - `src + 0x404 -> entry + 0x8c`
    - `src + 0x414 -> entry + 0x40`
  - 然后创建 `WSAEVENT`，调用 `WSAEventSelect(socket, event, 0x30)`，再对源对象地址发起 `connect`
  - `0x1004fe40` 则收集这些 `event`，用 `WSAWaitForMultipleEvents + WSAEnumNetworkEvents` 做 `2s` 轮询，把成功/失败候选分到不同容器
- `0x1005d050` 的三段登录 reply handler 也已经能顺下来了：
  - `0x7b000b` 回复进入 `0x1005db14`
    - 直接组第二包 `0x94000d`
    - 模板串就是 `{"信息":"申请服务器信息", ...}`
  - `0x94000d` 回复进入 `0x1005dc47`
    - 会把服务端返回的一整块服务器/授权信息写到 `ctx + 0x64fc54`
    - 同时把当前服务器对象快照复制到 `ctx + 0x64f810`
    - 然后再组第三包 `0x990fdb`
    - 模板串是 `{"信息":"登记客户端版本号，获得通行证信息", ...}`
  - `0x990fdb` 回复进入 `0x1005df49`
    - 命中后会记录 `{"信息":"网络验证成功","说明":"通过网络验证", ...}`
    - 随后立刻调用 `0x100618f0` 发 `0x6d0450 / 0x6e0450`
    - 模板串是 `{"信息":"正在读取代码表", ...}`
- 更关键的是，`0x10061a00` 已经把 `0x6d0450 / 0x6e0450` 回复的结构钉住了：
  - 包头开头 `u16 count`
  - 后面是 `count * 0x1d` 的固定长记录
  - 也就是每条 `29` 字节
  - 这和 Linux 侧已经打通的 `7709` 代码表记录大小完全一致
- `0x10061a00` 的循环在 `count < 1000` 时会推进到下一个 bucket；全部结束后把 `ctx + 0x64d4bc` 置为 `-1`
  - 因此 `0x100618f0` 返回 `0` 不是“失败”，而是“代码表所有分片已经请求完”
  - 此时 `0x1005e0fc` 后半段会转到 `0x10061bd0` 做代码表收尾/落地

继续顺着代码表完成后的链路往下追，`0x750010 -> 0x290547 -> 0x2a0547` 这条 post-login 业务流也已经开始成形：

- `0x10061bd0`
  - 会遍历 `ctx + 0x64f798` 里的 `29` 字节代码表对象
  - 做分类、去重，再压到 `ctx + 0x64f700 / 0x64f748` 这套后续业务容器
- `0x100653c0`
  - 已确认就是 `market_flag + code[6] -> SZ/SH + 六位代码`
  - 其中：
    - `0 -> SZ`
    - `1 -> SH`
- `0x10062340 -> 0x100625c0`
  - 发送 `0x76000f`
  - 单包最多 `50` 只股票
  - body 结构就是：
    - `u16 count`
    - `count * (1 byte 市场 + 6 byte 代码)`
  - reply 每股固定 `0x1d = 29` 字节
- `0x10062830 -> 0x10062ab0`
  - 紧接上一段，继续对同一批股票发 `0x750010`
  - 这轮每次最多 `20` 只
  - reply 每条跨度固定 `0x8f = 143` 字节
  - 新的 `capture_timeline_run` 代表流也把这一点坐实了：
    - 客户端 `frame[511..594]` 基本都是 `144 = 2 + 20 * 7`
    - 最后收尾帧是 `67 = 2 + 9 * 7`
    - 服务端对应 `original` 基本都是 `2862 = 2 + 20 * 143`
    - 最后收尾帧是 `1289 = 2 + 9 * 143`
  - 跑完后进入的 `0x1007d6b0 / 0x1007e710` 挂的是 `股票数据 / 除权财务 / 数据接口` 这组模板串，更像公共结果分发层，不像新的网络登录入口
  - 继续往下拆后还能看出：
    - `0x1007d6b0 -> 0x1007de80` 对应 `除权数据`，单条输出 `0xc8`
    - `0x1007e710 -> 0x1007e210` 对应 `财务数据`，单条输出 `0x15e`
    - 旁边兄弟函数 `0x1007e4d0` 对应 `实时数据`，单条输出 `0x1f4`
    - 这一层已经明显是共享数据对象组装层，不是新的 socket 登录链
- `0x10062c40`
  - 调用点已经确认在 worker 侧 `0x1005ec35`
  - 会对活跃连接项发送 `0x290547`
  - 这时每股请求项已经扩成 `11` 字节：
    - `7-byte code tuple`
    - `u32 token`
  - 单包最多 `100` 只
  - `capture_timeline_run` 的代表流里也已经直接能看到这一阶段：
    - 客户端 `frame[63]` 首次出现 `0x2900 / tag=0x0547 / payload=1104`
    - 后续 `frame[65] / [67] / [69] / [71]` 继续出现 `0x2a00 / tag=0x0547 / payload=1104`
    - 服务端对应出现大 `0x0547` zlib 正文，单帧 `original` 约 `11KB~12KB`
  - 再对照本地 [rustHq](/home/codes/crates/rustHq/README.md) 这套公开通达信实现后，可以把它理解得更准确：
    - 公开 TDX 的普通 quote 请求在 [packet.rs](/home/codes/crates/rustHq/src/packet.rs#L71) 里是 `2 + N * 7`
    - 而当前 `0x0547` 已经直接在线上样本里显示成 `4 + N * 11`
    - 也就是在 `market + code` 之外，又追加了 `u32 token`
  - 因而 `0x0547` 更像“带 token 的实时行情增量/续订变体”，不是公开 TDX 那个最原始的 `0x053e` quote 包
- `0x10062f00`
  - 是 `0x290547` 之后的增量/续订发送器
  - 发送常量是 `0x2a0547`
  - 同样是 `11` 字节一项、最多 `100` 只
  - 但会额外参考每股对象上的时间戳做筛选
  - timeline 尾段那组反复出现的 `0x2802 + 0x2902/0x2a02`，现在更像就是这条续订/状态链，而不是最初那批大正文实时数据
- `0x100631e0`
  - 是这条链的 reply dispatcher
  - 它由 `0x1005d85e` 调用
  - 会先把 payload 丢给 `0x100632b0` 做解析，再根据当前 opcode 决定是否继续调 `0x10062f00`
- `0x100632b0`
  - 现在不只是“会解析逐股回包”这么粗了
  - 它还会把回包里拿到的 `token + 时间戳` 写回股票对象
  - 后面 `0x10062f00` 之所以能按时间筛选并继续发 `0x2a0547`，就是直接读这里写回的槽位
  - 这一轮又确认了一个关键分支：
    - 它会调用 `0x10064300(code, market_flag)`
    - 如果是 `SZ` 且代码前缀是 `39`，或者是 `SH` 且代码前缀是 `8 / 00`，就走“特殊代码”路径
    - 这条路径下，回包里那组 `5 * 4` 价格列直接按 `0.001` 缩放写入
    - 否则会以 `parsed_record + 0x1d` 为基准价，再叠加回包里的增量值
    - 所以 `0x10064300` 不是权限判断，更像“按代码族切换盘口价阶解码规则”的 classifier
- `0x1007e4d0 -> 0x1007ef80`
  - 注意：以下地址属于当时分析的 DLL 样本，只用于说明该样本的 OEM 结构布局；
    当前生产公共行情时间转换已重新定位到 `网际风.exe 0x4839a3 -> 0x438800`，
    不应把这里的地址直接套到当前生产 `Stock.dll`。
  - `0x1007e4d0` 现在已经能比较明确地视为 `实时数据` 的批量导出器：
    - 先按 `count * 0x1f4 + 0x3e8` 分配输出区
    - 再逐条调用 `0x1007ef80`
  - `0x1007ef80` 则基本坐实为“内部实时对象 -> 对外 `OEM_REPORT(pack=1)`”的映射器
  - 这里和 [OemStock.h](/home/codes/quoteNetzipRs/netzipapi-rust-demo/netzip_api_bin/NetzipAPI/StockC++/OemStock.h) 里的 `#pragma pack(push, 1)` / `OEM_REPORT // 实时数据，500 字节` 偏移已经能一一对上：
    - `+0x058..0x084` 对应 `time/foot/openDate/openTime/closeDate/open/high/low/close/volume/amount/inVol`
    - `+0x088..0x176` 对应 `pricesell / volsell / vsellCha / pricebuy / volbuy / vbuyCha`
    - `+0x178..0x1a6` 对应 `jingJia/avPrice/isBuy/nowv/nowa/change/weiBi/liangBi/last/limitUp/limitDown/isIndex/isDaPan/isStock/bsNum/tickNum`
  - getter 也已经对上了：
    - `pricesell[i] = (this + 0x232 + i*4) / scale`
    - `volsell[i] = this + 0x272 + i*4`
    - `pricebuy[i] = (this + 0x212 + (7-i)*4) / scale`
    - `volbuy[i] = this + 0x252 + (7-i)*4`
    - `vsellCha / vbuyCha` 分别直接取自 `this + 0x19f / 0x18b`
    - `last / limitUp / limitDown` 分别来自 `0x10005130 / 0x10005160 / 0x100051c0`
  - 头文件里这几组盘口数组虽然声明成了 `float [10]`，但当前 exporter 只显式填前 `5` 档
  - 旁边 `0x10083980` 只是单纯 `memset(0, 0x1f4)` 的清零 helper
  - 这说明 `0x0547` 回包在 DLL 内部已经被解析成统一实时对象，最后再由 `0x1007ef80` 落成外部可见的 `OEM_REPORT`
  - 而 `0x1007e4d0` 的 5 个 caller 周围都能看到 `股票数据 / 除权财务 / 数据接口 / 补分笔 / 补K线 / 补F10` 这组模板串，进一步说明这层属于 API 导出分发，而不是新的建链逻辑
- `0x10025f30(field_id)`
  - 已经能确认它是一个通用“字段号 -> 取值公式”分发表
  - 其中：
    - `field_id = 0x0b` 最终落到 `OEM_REPORT.change`，语义是 `换手率`，不是涨跌幅
    - `field_id = 0x15` 最终落到 `OEM_REPORT.weiBi`，公式是 `(委买 - 委卖) / (委买 + 委卖) * 100`
    - `field_id = 0x17` 最终落到 `OEM_REPORT.liangBi`，公式是按已过分钟数折算后的 `量比`
- `0x1007d810`
  - 现在更准确地说，是 `OEM_MARKETINFO + OEM_STKINFO[]` 的代码表初始化导出器
  - `0x10083a00(base) = base + 0xc8`，`0x100839c0(n) = 0xc8 + n * 0xfa`
  - 这和 [OemStock.h](/home/codes/quoteNetzipRs/netzipapi-rust-demo/netzip_api_bin/NetzipAPI/StockC++/OemStock.h) 里的 `offsetof(OEM_MARKETINFO, stkInfo) = 0xc8`、`OEM_STKINFO = 250字节` 完全对上

另外，`rustHq` 这份公开通达信实现对 `0x750010` 这条线也给了一个很强的交叉印证：
- [packet.rs](/home/codes/crates/rustHq/src/packet.rs#L168) 的标准财务请求低 `16` 位就是 `0x0010`
- [parser.rs](/home/codes/crates/rustHq/src/parser.rs#L352) 的标准财务 reply 是 `2-byte prefix + N * 143B record`
- 而我们在 `capture_timeline_run` 里实测到的 `0x7501 / 0x7502` 回复长度恰好是 `2862 = 2 + 20 * 143`、`1289 = 2 + 9 * 143`

所以 `0x750010` 现在更应该直接视作“批量财务阶段”，而不是一段尚未定名的 post-login 固定长数据流。
  - caller 链和附近模板串 `%s代码表 / 股票代码日期 / 期货代码日期 / 除权数据 / 财务数据 / 实时数据` 说明这就是“初始化阶段发送代码表、再接除权财务”的 API 输出分支
- `0x1007de80 / 0x1007e210`
  - 前者现在可以视为 `OEM_SPLIT_HEAD + OEM_SPLIT[]` 的除权导出器，因为它正好按 `0xc8 + n * 0xc8` 组织数据
  - `OEM_SPLIT` 单条也已经基本拆开：
    - 前 `0x14` 字节就是 `time + give + allocate + price + earnings`
    - 后面的 `explain` 则是把原始记录 `+0x14` 的 `0x28` 字节文本转成宽字符后写入
  - 后者则正好按 `n * 0x15e` 输出，而 `0x15e = 350`，与 `OEM_FINANCE` 完全一致
  - 财务这条至少已经能确认：
    - `label -> 0x00`
    - `name -> 0x18`
    - `0xcc` 字节连续 payload -> `0x58`
    - 这正好覆盖 `time / baoGao / shangShi` 加后面全部 float 字段
    - 更细一点是：DLL 实际先生成一块 `0xe0` 字节中间缓冲，再跳过前 `0x0c` 字节，只把后面的 `0xcc` 映射进 `OEM_FINANCE`
    - 这轮进一步确认：财务对象实际是 `0xe4` 字节槽位，`obj + 0xe0` 是 free-list 指针，索引里存的是对象基址；因此 `skip 0x0c` 更准确的含义是“跳过 raw finance record 前导的 12 字节 code/key 槽，从 `obj + 0x0c` 开始贴 `time / baoGao / shangShi + 全部 float payload`”
    - 同时 `0x10019710` 会把上游原始财务记录整块 `0xe0` 字节拷进对象基址，而且 caller 会把同一个源指针同时当 `record_src` 和 `lookup_key` 传进去；再结合 `0x100653c0 -> 0x10019d20(flag=0) -> 0x10017a00/0x1000fe20` 这条链，当前可以把这 `12` 字节前缀基本定成“ASCII 的 `SZ/SH + 6位代码`，再加尾部零填充”的 symbol/key 槽
    - `0x10018b00` 也已经能确认为这块对象上的 `finance getter(field_id = 0x2a..0x4b)`，它读取的偏移序列和 `OEM_FINANCE + 0x58` 完全同相，只是整体前移了 `0x0c`
    - 其中 `0x2a` 已可基本定性成“由 `baoGao` 推导出的报告季度号”，因为它会把 `YYYYMMDD` 风格的 `obj + 0x10` 折成 `1..4`，并被直接用在 `price / EPS * quarter / 4` 这种 annualize 公式里
    - 已对上的 getter case 里，`0x2b / 0x2c / 0x33 / 0x35 / 0x44` 已可直接写成 `zongGu / liuTongAG / bGu / mgShouYi / mgJingZhi`，而 `0x3a..0x42`、`0x46..0x4b` 也分别落在 `总资产链` 与 `收入利润链`
    - 另外 `0x30 / 0x34 / 0x36 / 0x3c / 0x43` 这几项 getter 当前就是硬编码返回 `0`
    - finance raw record 的文件来源也已对上：`0x10018240` 解析的就是 `full_sh.FIN / full_sz.FIN` 这类下载文件，格式是 `magic(0x223fd90e) + record_size + records...`，其中单条 record 仍然是“前 12 字节 key/symbol，后面 `time / baoGao / shangShi + floats`”这套布局
    - 同时 `0x10062b8f -> 0x10066d70` 还能在本地按同一 raw layout 合成 finance record，并且已经能直接对上多组 raw 偏移，例如 `+0x18 -> mgShouYi`、`+0x1c -> mgJingZhi`、`+0x44 -> zongZC`、`+0x98 -> zongGu`、`+0xa0 -> liuTongAG`
    - 这些结论现在已经固化进 `src/tdx_fin.rs`，可以直接用 `parse_fin_file` / `parse_fin_bytes` 读 `FIN` 文件，并用 `examples/tdx_fin_dump.rs` 导出 `CSV`
    - 进一步实测官方 `full_sh.FIN / full_sz.FIN` 后，当前可确认两边 `record_size` 都是 `224`，分别有 `2412 / 3012` 条记录，且 raw record 尾部 `8` 字节目前都固定为 `00 00 80 3f 00 00 00 00`
  - 这意味着当前 DLL 导出层里，`代码表 / 除权 / 财务 / 实时数据` 这四类外部结构都已经能和头文件尺寸对上

强推断：

- `0x290547 / 0x2a0547` 大概率就是此前抓包里那条 `0x0547` 行情业务链的 DLL 侧构造/续订逻辑
- 现在还不能把它和某一个具体端口 `100%` 绑定死，但从“低 16 位常量一致 + 批量按股票请求 + 每股回包解析”这三点看，已经非常接近了
