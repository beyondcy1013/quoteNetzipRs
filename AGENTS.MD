# AGENTS.md


## RTK 命令执行规则

- 本项目启用 `/home/root/.codex/RTK.md`。
- 在 Codex CLI 会话中运行 shell 命令时，默认使用 `rtk <command>` 前缀。
- 遇到 `rtk` 不支持的复杂 shell、`find` 谓词或原始管道语法时，使用 `rtk proxy <command>` 执行原始命令。
- 可用 `rtk --version`、`rtk gain`、`which rtk` 验证 RTK 是否生效。

本文件适用于整个 `/home/codes/quoteNetzipRs` 仓库。

本项目继承 `/home/codes/AGENTS.MD` 的公共执行策略、上下文记忆规则、系统级硬约束、服务部署总约定和文档分层规则。若本文件与上级系统目标冲突，以上级为准。

## Project Highest Goal

Provide a correct, timely, observable, and progressively Wine-independent Netzip market-data client
for the stock system, while retaining the vendor client as a controlled comparison anchor until the
native Rust chain has equivalent production evidence.

## Current Market Client Progress

- The Linux-native `7709` path provides code-table synchronization, live quotes, K-line, F10, FIN,
  and `0547` parsing through CLI and HTTP entry points.
- Native account `1522` authentication to remote `7100` succeeds with the verified three-response
  sequence, but it is currently a standalone Rust module/CLI rather than a resident service session.
- `netzip-rs.service` and `netzip-rs-full-push.service` are deployed under `/home/bin/netzip/` and
  publish native Rust quotes to quoteGateway.
- Commit `28ea978` bounded the gateway TCP client cache after thread-keyed entries exhausted file
  descriptors with retained `CLOSE-WAIT` sockets. Post-deployment idle checks reported `fd=7` and
  `CLOSE-WAIT=0`; trading-load acceptance remains required.
- Wine remains the behavioral comparison source for the local `2000` bridge, post-login object
  chain, field parity, and long-lived same-account behavior.

The living progress, evidence, acceptance criteria, and unresolved issues are maintained in
[`docs/codex/tasks/netzip-rs-client-progress.md`](docs/codex/tasks/netzip-rs-client-progress.md).

## 修复 Bug 的根本原则

本规则与 `/home/codes/AGENTS.MD` 中的同名章节完全一致，是 `quoteNetzipRs` 项目排障的最高优先级准则，在此显式重复一次以避免被忽略。

- **故障内容是指示，不是目标**。报错信息、异常值、错误状态、日志堆栈、监控告警、用户投诉等，都只是指向问题根因的线索，不是要去消除的对象。
- 修复 Bug 的正确做法是**顺着故障内容往上追，一直追到真正出问题的上游逻辑，并修正那一层**；故障只是下游的投影，掩盖下游投影不会让上游逻辑变正确。禁止在下游故障出现的位置就地“处理”掉：吞异常、静默兜底、把错误状态伪装成成功、用假数据替换真实数据、绕过触发条件、关掉告警、注释掉错误分支、加一段只在表象上让结果正常的补丁等。
- 简单自检：修复后用同样的输入重新触发，上游逻辑是否已不再产生坏数据/坏状态；如果只是下游不再报错，等于没修。
- 真正的修复应能解释清楚：故障内容向上指向了哪一层上游逻辑，这一层为什么出错，修复后为什么同样的输入流过这一层就不会再产生该故障。
- 排障顺序固定为：先回答“坏数据/坏状态是在哪一层上游产生的、为什么产生”，再回答“下游应该怎样不再被它带坏”。绝不允许跳过上游、直接在下游做症状消除。

## 项目定位

这个项目不是对网际风协议的纯 Rust 重写，而是围绕官方 `Stock64.dll / Stock.dll`、抓包样本和逆向笔记建立起来的 Rust 分析与调试仓库，当前主要承担 4 类工作：

1. Rust FFI 调用官方 DLL。
2. 对 `OEM_DATA_HEAD` 及各类返回包做结构化解析。
3. 对 `6100 / 7100 / 7708 / 7709 / 7719 / 14017` 等相关流量做抓包、重组和协议分析。
4. 提供本地 HTTP/Web UI 调试入口，方便手工调用和验证分析结论。

## 参考目录

本项目的重要参考目录（已移入本项目内）：

- 相对路径：`./netzip_api_bin/NetzipAPI`
- 绝对路径：`/home/codes/quoteNetzipRs/netzip_api_bin/NetzipAPI`

这个目录不是当前 Rust 项目生成的内容，而是网际风官方/历史样本/配套资源的参考资料集合。当前仓库中的很多结论、默认文件路径、请求串格式和运行依赖，都是从这个参考目录抽出来的。

## 参考目录的功能

`./netzip_api_bin/NetzipAPI` 目前承担以下角色：

### 1. 官方接口与调用规范参考

根目录下的这些文件是理解上层调用格式的直接来源：

- `股票接口调用规范.txt`
- `公式函数调用规范.txt`

它们用于确认：

- `Ask(...)` 的调用方式
- 明文请求串格式，例如 `股票数据?请求=登录...`
- 公式/指标类调用的参数组织方式

注意：这些文档的编码可能不是 UTF-8，直接读取时可能出现乱码，需要按 UTF-16LE、GBK 等方式判断。

### 2. 官方示例实现参考

参考目录里有三套示例：

- `StockC++`
- `StockC#`
- `StockPy`

它们的作用不是“可直接复用到 Rust”，而是提供官方调用模式、结构定义和运行时上下文。尤其要优先参考：

- `StockC++/OemStock.h`
- `StockC++/OemStock.cpp`
- `StockC#/OemStock.cs`
- `StockPy/OemStock.py`

这些文件用于确认：

- DLL 导出函数的签名
- 结构体定义
- 示例初始化顺序
- `Start / Ask / Stop` 的典型调用方法

### 3. 运行时资源与配套文件参考

参考目录中包含当前项目已确认会影响 DLL 运行的资源，例如：

- `StockC++/Stock64.dll`
- `StockC++/Stock.dll`
- `StockC++/Stock.dat`
- `StockC++/Stock.字典`
- `StockC++/升级配置.ini`
- `StockC++/用户/服务器列表.ini`
- `StockC++/用户/配置文件.ini`
- `StockC++/系统/Stockdrv.dll`

这些文件用于回答两类问题：

- DLL 运行时除了 `Stock64.dll` 之外还依赖什么
- 官方样本把服务器列表、升级配置、用户配置、代码表缓存放在哪里

### 4. 服务器与业务数据侧参考

参考目录中的这些子目录是协议分析的重要旁证：

- `StockC++/用户`
- `StockC++/数据`
- `StockC++/升级`
- `StockC++/系统`
- `StockC++/大智慧`

它们分别用于：

- `用户`：服务器列表、用户配置、代码表 CSV 等本地状态
- `数据`：财务、除权、实时缓存等业务文件样本
- `升级`：升级配置与升级说明
- `系统`：底层驱动或系统依赖
- `大智慧`：代码表等兼容/联动数据样本

### 5. 可执行程序与逆向入口参考

参考目录中还包含多个可执行文件，例如：

- `StockC++/网际风.exe`
- `StockC++/股票诊断模式.exe`
- `StockC#/网际风.exe`
- `StockPy/网际风.exe`

它们的用途主要是：

- 作为抓包目标进程
- 提取字符串、服务器域名、下载地址、端口等静态线索
- 配合 WinDbg/x64dbg/Frida 做运行时分析

## 当前仓库与参考目录的关联方式

当前 Rust 仓库与参考目录的关系可以概括为：

1. `README.md` 中关于 DLL 路径、运行前提的描述，来源于 `./netzip_api_bin/NetzipAPI`。
2. `PROTOCOL_NOTES.md` 中关于请求串、服务器端口、抓包结论、下载文件和配置文件的记录，部分直接来自参考目录中的文档、样本程序和资源文件。
3. `src/` 和 `examples/` 中的很多解析逻辑，是基于参考目录里的结构定义、官方示例行为和抓包样本逐步确认出来的。
4. Web UI 和 HTTP 服务只是当前 Rust 仓库的调试外壳，真正的业务语义来源仍然要回到参考目录与抓包样本一起交叉验证。

## 使用约定

在这个仓库里工作时，应遵守下面的约定：

1. 把 `./netzip_api_bin/NetzipAPI` 当成”参考资料目录”，不是当前仓库的实现目录。
2. 当前仓库优先修改 `/home/codes/quoteNetzipRs` 内的代码和文档；不要无意修改参考目录。
3. 遇到以下问题时，应优先回查参考目录：
   - DLL 导出签名不确定
   - 请求串字段含义不确定
   - 缺少运行时文件，不清楚放置位置
   - 不清楚某个抓包字段对应哪个官方文件或配置项
4. 遇到以下问题时，不能只依赖参考目录，还必须结合抓包或运行时分析：
   - 实际联网服务器地址是否已变更
   - 某个端口是否仍在使用
   - 某个字段在出网前是否又经过压缩、字典化或加密

## 建议阅读顺序

新接手这个仓库时，建议按下面顺序建立上下文：

1. 先看当前仓库的 `README.md`
2. 再看当前仓库的 `PROTOCOL_NOTES.md`
3. 然后回看参考目录：
   - `./netzip_api_bin/NetzipAPI/股票接口调用规范.txt`
   - `./netzip_api_bin/NetzipAPI/StockC++/OemStock.h`
   - `./netzip_api_bin/NetzipAPI/StockC++/用户/服务器列表.ini`
   - `./netzip_api_bin/NetzipAPI/StockC++/升级配置.ini`
4. 最后再结合 `captured_windows_traffic/`、`examples/` 和 `src/bin/netzip_service.rs` 看当前仓库已经落地了哪些能力

## 结论

`./netzip_api_bin/NetzipAPI` 的本质是：

- 官方接口规范来源
- 多语言示例来源
- 运行时依赖样本来源
- 抓包和逆向时的对照物来源

它不是本仓库的一部分实现，但它决定了本仓库很多分析工作的上下文和边界。
