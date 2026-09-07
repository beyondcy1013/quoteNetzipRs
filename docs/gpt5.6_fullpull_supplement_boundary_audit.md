# netzip-fullpull / netzip-supplement 边界核实审计

Updated: 2026-09-01
方法：只读静态审计（三路并行代码探索，未修改文件、未运行网络会话、未读取凭据值）。
本文档是 `docs/capability-boundaries.md` 所定目标边界的第一次全面核实，记录"目标所有者
与当前物理位置"的差距。行号为审计时点快照，后续迁移会使行号漂移，引用时以路径+符号名为准。

---

## 1. 权威边界（复核基准）

以 `docs/capability-boundaries.md` 为准：

| Crate | 唯一职责 | 端口边界 |
|---|---|---|
| `netzip-fullpull` | 正式账号认证后的官方全推：连接、初始化、持续接收、解码、重连、状态 | 非 7709；当前证据指向认证后的 5188 |
| `netzip-supplement` | 全部 7709 查询型补数据：代码表、快照、K 线、F10、财务及 OEM 映射 | 7709 |
| `quoteNetzipRs` | 产品编排，不重写底层协议算法 | 不把端口写进产品名 |

关键判别标准：**区分依据是"认证与否 + 是否 7709"，不是"是否持续收到数据"。**
7709 `0x0547` 的初始请求、续订（renewal）、unsolicited delivery、worklist 轮询，
全部属于 7709 查询/订阅型补数，即使它们表现为持续推送，也不是官方全推。

结论先行：**边界定义本身自洽合理；当前代码处于迁移中间态，实现归属与目标边界相反。**

---

## 2. 总体判断

### 2.1 核验状态

本审计的 P0/P1 结构性结论已由当前源码、依赖图和运行脚本复核，状态为
**confirmed**：`netzip-fullpull` 仍物理承载 7709，现名 full-push 服务仍执行
7709 工作表链，认证状态尚未绑定常驻 5188 数据会话，且 supplement 反向依赖
fullpull。关于 BJ 支持、隐式票据、动态端点和在线分时/逐笔的结论保持
**needs-verification**，不得当作已实现或已否定。整改顺序与验收标准是建议，
不是完成声明。新增 `official_5188` 仅证明帧边界/重组基础层已落地，不改变上述判断。

```text
目标：
netzip-fullpull    = 认证后非 7709 官方全推（当前仅 official_5188 帧边界已落地）
netzip-supplement  = 全部 7709 查询型补数据

现状：
netzip-fullpull    = official_5188 帧边界 + 全部 7709/0547/K线/F10/FIN 实现 + NativeSession(7709 包装)
netzip-supplement  = 在线仅 0x052d 三周期 K 线 + 本地 Wine 文件解析，且反向依赖 fullpull
运行链路           = "full-push" 服务实际执行 7709 查询/订阅发布，且与认证完全解耦
```

无 Cargo 循环依赖；问题是**所有权方向颠倒 + 运行链路语义错误 + 认证未闭环**。

---

## 3. 依赖拓扑现状（确定事实）

```text
quoteNetzipRs (root, members 仅 "." + tuwenca-codec + stockdrv-compat)
  ├─ path dep: ../crates/netzip-fullpull
  ├─ path dep: ../crates/netzip-supplement ──→ netzip-fullpull   ← 反向所有权边
  └─ root facade 重复导出 fullpull 的 7709 全部模块

tdxRs/crates/tdx-runtime ──→ stock-source-netzip ──→ netzip-fullpull (NativeSession, 实为 7709)
```

- 根 workspace 成员表：`Z:\stock\quoteNetzipRs\Cargo.toml:18-20`；外部 path 依赖 `:31-34`。
- `netzip-fullpull`、`netzip-supplement`、`stock-source-netzip` 各有独立 `Cargo.lock`，
  不受根 workspace resolver 管理，无法用单条 `cargo test --workspace` 验证整体边界。
- `netzip-supplement/Cargo.toml:6-11` 声明 `netzip-fullpull = { path = "../netzip-fullpull" }`；
  `netzip-supplement/src/lib.rs:7-10` 直接导入 `Tdx7709Config/Tdx7709Session/
  Tdx7709CodeTableRecord/Tdx7709KlineBar` 和 FIN 类型。
- tdxRs 侧解析：`tdx-runtime` 的 `../../../crates/netzip-fullpull` 解析为
  `Z:\stock\crates\netzip-fullpull`，路径存在；fullpull 自身对 `dllhqarrow-rs` 的
  `../../../crates/rustHq` 解析为 `Z:\crates\rustHq`，同样存在。

---

## 4. 发现清单（按严重度）

### P0-1 fullpull 物理承载并公开导出全部 7709 实现

- `Z:\stock\crates\netzip-fullpull\src\lib.rs:8-20` 自述从旧共享 crate（netzip-native）
  迁入 7709/0547/K线/F10/FIN，且全部 `pub mod` + re-export（`:22-59`）。
- `src\client.rs:7-8,78-90,101-188`：`NativeSession/NativeDriverConfig/NativeInstrument`
  等公共 API 实际包装 `Tdx7709Session::open_quote_only`，注释直接写明 quote-only 7709。
- `src\tdx7709.rs:19-20` `DEFAULT_PORT = 7709`、默认 host 为空；`:47-65` 配置仅
  host/port/超时，无任何认证上下文字段；`:193-209` open/open_quote_only；
  `:219-268` 0547 请求与 renewal；`:341-465` K线/F10 查询；`:469-517` 公开 fetch API；
  `:767-856` 直连 socket、固定 bootstrap（`0x010c/0x020c/0x030c`），可选同步代码表。
- 影响：任何 fullpull 的 consumer 都能直接执行 7709 查询，边界无法由依赖结构防护。

### P0-2 运行时 "full-push" 主链实为 7709 查询/订阅发布

- `Z:\stock\quoteNetzipRs\src\bin\netzip_service.rs:7415-7540`
  `execute_hqw_push_worklist`：读 quoteGateway 工作表 → 建 `Tdx7709Session` →
  代码表 → `request_live_quotes` → 收 delivery → 周期 renewal → 发布。
- `:7823-7980` `publish_full_push_stage` / worker 仍以 `Tdx7709Config`（7709）发布；
  `:7979,8042` 错误字符串明示 7709。
- `:1873-1875` 暴露为 `POST /api/hqw/push-worklist`；`:2149-2173` capabilities 把该
  路径列为 public service contract——不是孤立命名遗留，而是可生产调用的数据源分类错误。
- `scripts\run-full-push.sh:10-14,96-108` 默认 `NETZIP_FULL_PUSH_MODE=push`，
  调用上述 7709 接口；`deploy\quote-netzip-rs-full-push.service:1-11` 服务描述为
  "native Rust full-market publisher"，且 `Requires=quote-netzip-rs-supplement.service
  quoteTdx.service`——依赖补数链，反证其并非独立官方全推。
- `scripts\install-service.sh:49-51` `NETZIP_INSTALL_ENABLE_FULL_PUSH:-1`，
  安装即默认启用该错误命名的常驻服务。
- 按边界：这条链应称"7709 补数/过渡发布路径"（capability-boundaries.md:46-47 已如此定性），
  运维默认行为与文档定性冲突。

### P0-3 认证结果未绑定任何数据链

- 认证入口 `netzip_service.rs:2066-2096`（6100/7100 认证），结果仅写入状态
  `:2129-2140`；`auth_7100_client.rs:63-83,120-208,265-383` 完成账号/密码 UTF-16
  字段、认证链响应角色校验（zstd_dictionary/download_file）并从下载配置选择 5188 与 7709。
- 但 7709 发布链从独立环境变量 `NETZIP_TRANSITION_7709_ENDPOINTS` 取端点
  （`netzip_service.rs:7144-7202`），随后直接建 session（`:7435-7449,:7531-7534`）。
- 不存在"authenticated=true 才可建全推连接""认证失效即停全推""fullpull 只连认证后
  非 7709 端点"的任何 gate；7709 也不受认证状态约束。
- fullpull 配置结构（`tdx7709.rs:47-65`）无凭据/认证字段，认证上下文无法传入。
- 补数 handler 普遍自行构造 `Tdx7709Config` 或回退 `Tdx7709Config::default()`
  （host 为空），与"认证成功后端点自动供补数使用"的文档含义不符。
- 推断（needs-verification）：7709 固定 bootstrap 是否被服务端视为隐式票据未知；
  代码层面确认"未执行任何认证"。

### P0-4 supplement 反向依赖 fullpull（所有权方向颠倒）

- `Z:\stock\crates\netzip-supplement\Cargo.toml:6-11`；`src\lib.rs:7-10,1047-1104`
  直接以 fullpull 的 `Tdx7709Session` 执行补数；README（`:2-5`）自认 compatibility dependency。
- 后果：未来"把 7709 移出 fullpull"会变成结构性迁移；当前 supplement 无法独立于
  fullpull 编译，其"7709 唯一所有者"的声明不能由依赖图成立。
- 若两侧需要共享帧编解码，应拆中立 `netzip-protocol`/codec crate，而非 supplement→fullpull。

### P1-1 supplement 未实际拥有"全部 7709 查询型补数据"

当前在线执行器仅有 `0x052d` K 线（日线/5 分钟/1 分钟）：
`Z:\stock\crates\netzip-supplement\src\lib.rs:800-835,856-867,1028-1164,1166-1233,1235-1283`。

仍留在 fullpull / 根 facade 的 7709 能力：

| 能力 | 目标所有者 | 当前物理位置 |
|---|---|---|
| 7709 transport / bootstrap | supplement | fullpull `tdx7709.rs:767-856` |
| 代码表同步 | supplement | fullpull `tdx7709.rs:809-847`；根服务多处每次全量同步再本地过滤 |
| 0547 查询 / delivery / renewal / token | supplement | fullpull `tdx7709.rs:219-268` + `tdx_0547*` + scheduler/coalescer |
| K 线（0x052d） | supplement | supplement 已有（经 fullpull transport）；通用 0-11 类解析在 fullpull/root |
| F10 分类/内容 | supplement | fullpull `tdx7709.rs:376-467`；根服务直调（`netzip_service.rs:4359-4534`） |
| FIN | supplement | fullpull FIN parser |
| 实时快照编排 | supplement | 根服务聚合接口（K线+F10+live quote，`netzip_service.rs:2799-3011`），非独立 wire 协议 |
| 在线分时序列 | supplement | **无在线执行器**（仅 Wine `实时.dat` 本地解析，`netzip-supplement/src/lib.rs:544-741`） |
| 在线逐笔 | supplement | **无在线执行器** |
| 官方 5188 全推 | fullpull | 仅 `official_5188` 帧边界/TCP 重组/方向分类；无认证后业务闭环 |

- 超声明：`Z:\stock\crates\netzip-supplement\README.md:3-22` 宣称拥有全部 7709 补数并
  包含 0x0547 delivery，实际在线执行器仅三周期 K 线。
- UI 超声明：`webgui\index.html:184-190` 提供可勾选的"补分时/笔"，无对应在线执行器；
  `:156-229` 其余补数选项也未绑定任务执行器；`:237-239` 认证面板反而注明真实 DLL 登录
  尚未封装。
- BJ/市场 2：代码表同步只覆盖 bucket 0/1 且按前 23 块映射 SZ、其余映射 SH
  （fullpull `tdx7709.rs` 同步段）；对外 symbol 归一化接受 BJ（`netzip_service.rs:6782-6822`）
  属于"输入格式可接受"，不等于代码表/数据链支持 BJ（needs-verification）。
- 实时查询 symbols 少于 3 个时自动补 seed symbols（`netzip_service.rs:6762-6779`）：
  响应可能包含调用方未请求的内部 seed，需确认契约与过滤。
- "全部股票"范围：规划器要求调用方解析好 instruments，"补全部股票"选项无对应
  任务执行器；全量同步代码表再本地过滤的做法在根服务多处重复，存在延迟与远端压力问题。

### P1-2 适配器把 7709 查询包装成 fullpull 身份（跨产品来源标签错误）

- `Z:\stock\crates\stock-source-netzip\src\lib.rs:2,7-12,23-26,35-60`：
  `SOURCE_ID: &str = "netzip-fullpull"`，内部用 `NativeSession`（7709 quote-only 包装）。
- `Z:\stock\tdxRs\crates\tdx-runtime\src\source_plugins.rs:12-16,78-89,273-283` 默认
  启用该来源。
- 影响：运行时优先级、监控、状态和数据审计会把 7709 轮询结果记成官方全推来源。
- 应改为 `netzip-supplement`（或更细的 `netzip-supplement-7709-0547`）语义。

### P1-3 根 crate facade 重复导出，迁移面被放大

- `Z:\stock\quoteNetzipRs\src\lib.rs:19-26` 重新声明 tdx7709/0547/FIN/push 模块；
  `:104-147` 再导出全部类型与函数；桥模块 `src\tdx7709.rs:1`、`src\tdx_0547.rs:1` 等
  直接 `pub use netzip_fullpull::*`。
- `src\bin\netzip_service.rs:8-36` 直接从根 facade 导入 7709 API。
- 结果：`fullpull 实现 → root facade → service 端点`，迁移时必须同时处理底层 crate、
  根 facade、service import、适配器与测试归属（7709/0547/push 测试现位于 fullpull，
  根 `tests\tdx_push_poll_policy.rs:1-61` 又有一份）。

### P1-4 服务监听与请求面缺防护（独立于 crate 边界的安全问题）

- `Z:\stock\quoteNetzipRs\README.md:21-29` 明示 `0.0.0.0:16893`、无鉴权、勿映射公网；
  绑定于 `netzip_service.rs:1963-1965`。
- 路由 `:1861-1961` 混合了认证、7709 查询、补数、诊断探针、文件解析、quoteGateway 发布。
- 多个请求允许调用方提交目标 host/port（如 snapshot/K线/supplement `:3843-3955`），
  形成 SSRF/内网 TCP 探测面；部分补数/文件端点允许本地输入路径与 CSV 输出路径
  （`:11128-11147` 等），形成本地文件读写面。
- 认证 login 反而默认关闭、需 env opt-in（`:1988-1995`）——上游账号认证与
  HTTP 调用方鉴权是两个边界，当前都未闭合。
- systemd 以 root 运行（`deploy\quote-netzip-rs-supplement.service:9-13`）放大影响面。

### P1-5 下载配置完整解析器公开凭据字段

- `Z:\stock\quoteNetzipRs\src\auth_download.rs:29-52,67-107,110-120,136-166`：
  `DownloadedServerConfig` 公开 `account/password/raw_text`；完整 parser 把配置中的
  账号/密码读入结构体。窄 parser 避免了 live auth result 暴露，但 full parser 未收口。
- 状态接口 `netzip_service.rs:373-415` 暴露 `account`/`password_source`；在无鉴权监听下
  构成账号元数据泄露（密码值未序列化，`auth_credentials.rs:13-39`；测试 `:429-439`
  仅验证密码不序列化）。
- 代码内存在默认账号硬编码常量（值为真实账号，本文档不复述）；`webgui\index.html:68-71`
  与 `AGENTS.MD`（217 行起）含 credential-like 默认值/敏感材料（值不复述）。
  应移除硬编码、轮换已暴露凭据、改部署时注入。

### P2-1 命名残留系统性误导

- 脚本/服务/环境变量/测试沿用 full-push：`run-full-push.sh`、
  `enable-native-push-mode.sh:11-31`、`resume-full-push.sh`、`pause-legacy-full-push.sh`、
  `set-fullpull-endpoints.sh:21-41`（把 7709 过渡端点写进
  `NETZIP_TRANSITION_7709_ENDPOINTS` 并启停 full-push 服务）、
  `test-run-full-push-http-error.sh:22,41-51`（断言"7709 fallback full-push"文案）。
- fullpull 内旧 `Native*` 命名与"原生全推"注释（`client.rs` 多处；README `:9-18`）。
- 隐式 BJ 轮询：`netzip_service.rs:7427-7485` 分拆 BJ 并进入 `execute_bj_poll_loop`，
  与"全推=服务端推送"定义冲突，应归 7709 查询补数命名。

### P2-2 测试缺口（无法证明新边界）

已覆盖较好：Wine worklist/PWR/FIN/实时文件解析、K 线分页与校验、OEM 编码、
0547 delivery 机制、scheduler/poll policy 部分（fullpull `tdx_0547*`、
`netzip-supplement/src/lib.rs:1285+`、根 `tests\tdx_push_poll_policy.rs`）。

缺失的关键契约测试：

1. 未认证时禁止建立 fullpull 数据连接；认证失效即停止全推。
2. fullpull 不允许出现 7709 端点/会话；7709 只能经 supplement。
3. 7709 查询结果不得以 `netzip-fullpull` source ID 发布（适配器负例）。
4. supplement 拥有 0547 查询/delivery/renewal、代码表、F10 的独立执行与测试
   （当前 0x0547 相关测试主体在 fullpull）。
5. BJ/市场 2 代码表真实同步测试。
6. HTTP 调用方鉴权、host/port allowlist、任意本地路径拒绝。
7. 任务模型：取消、重试、恢复、持久化（当前 K 线补数为同步 RPC，无任务 ID/取消/恢复）。
8. "7709 订阅不得标记为 official full push"的命名/契约测试。

### P2-3 其他需要澄清的表述

- "快照"是编排接口（K线+F10+live quote），不是独立 7709 snapshot wire 协议
  （`netzip_service.rs:2799-3011`）；对外文档不得暗示独立协议能力。
- README 能力清单（code table/realtime/tick/intraday/K线周期/split/finance/F10/depth，
  `README.md:382-390,408-455,593-600`）是解析器/兼容能力概览，不应被读作
  supplement 在线 API 全集；`/api/supplement/kline` 实际限定三周期。
- `netzip-supplement` 非持久任务系统：同步阻塞执行、无任务 ID/队列/状态查询/取消/
  恢复；`continue_on_error` 是单请求内选项，不是可恢复任务机制。

---

## 5. 整改顺序建议

1. **拆中立协议层**：将 7709 帧编解码、Server16、bootstrap、0547/代码表/K线/F10 的
   协议与公共类型迁入 `netzip-supplement`（或先拆 `netzip-protocol` 中立 crate，
   fullpull 与 supplement 共同依赖它）。
2. **fullpull 收口**：只保留 official_5188 及其上的认证后全推会话 API；
   删除/隔离 `Native*` 与全部 7709 公共导出；配置结构纳入认证上下文。
3. **运行链正名**：`/api/hqw/push-worklist`、runner、脚本、systemd、环境变量、日志
   统一改为 `7709 supplement active-query/publisher` 语义；安装默认不再启用它
   作为"full-push"。
4. **适配器正名**：`stock-source-netzip` 的 SOURCE_ID 与实现改为 supplement 语义；
   tdx-runtime 插件描述同步更新。
5. **认证闭环**：认证成功 → 绑定 5188 端点 → fullpull 建链；认证失效停链；
   补数统一使用已发现端点或显式配置，移除空 host 默认值；未认证拒绝 fullpull。
6. **服务防护**：HTTP 调用方鉴权（默认开启）、host/port allowlist、文件路径沙箱、
   非 root 运行。
7. **凭据治理**：移除代码/前端/指令文件中的硬编码默认凭据，轮换暴露值，
   下载配置 parser 收敛凭据字段，状态接口去除账号元数据。
8. **workspace 收口**：将 `../crates/netzip-fullpull`、`netzip-supplement`、
   `stock-source-netzip` 纳入统一 workspace（或建立跨 crate 集成验证 workspace），
   消除独立 Cargo.lock 分裂。
9. **契约测试**：按 §4 P2-2 清单补齐，作为迁移验收门槛。

## 6. 验收标准（与 capability-boundaries.md 对齐）

官方全推完成需同时证明：运行时正式账号认证（凭据不落源码/文档/日志/状态响应）、
认证后非 7709 连接与 Wine 一致初始化、回调式全市场接收（不得用工作表轮询冒充）、
quoteGateway 收到独立 Rust 来源实时批次且可恢复、7709 故障不影响全推主链。

补数验收围绕 7709 查询的分页、去重、节流、字段一致性、分项失败与取消展开，
不得使用"全推完成"作为结论。分时/逐笔在在线执行器落地前，UI 与文档必须标注
"未实现（本地 Wine 文件解析除外）"。

## 7. 脱敏声明

本文档为审计记录，遵守项目凭据边界：不包含真实账号、密码或任何可登录凭据值；
涉及默认凭据常量、嵌入凭据样例的位置仅以路径+行号指认，值一律不复述。
