# quoteNetzipRs — Grok Bot 项目入口

本仓库产品定位：`quoteNetzipRs = 官方全推 + 7709 补数据`。
Linux/Rust 复刻 `quoteNetzipWine` 的两类数据能力，二者不得混称。

## reverse-skill 包

`vendor/reverse-skill` 是 `zhaoxuya520/reverse-skill` 的 git submodule。
Grok 入口：`.grok/skills/reverse-skill/SKILL.md`。
克隆本仓请带 `--recurse-submodules`，或事后 `git submodule update --init vendor/reverse-skill`。
协议/PE 方法可参考 pack；官方全推证据与验收仍以本仓库 fullpull skill 和账本为准。
禁止把 pack 的渗透/利用模块用到未授权目标。

## 权威阅读顺序

1. `AGENTS.MD` — 项目最高目标、账号与证据约束
2. `docs/capability-boundaries.md` — 全推 vs 补数据术语
3. `.agents/skills/quoteNetzipRs-ops/SKILL.md` — 工作路由
4. `.agents/skills/quote-netzip-rs-fullpull-replication/SKILL.md` — 官方全推复刻
5. `docs/fullpull-replication-authority.md` — 全推复刻结果账本
6. `docs/codex/tasks/netzip-rs-client-progress.md` — 当前进度

## 能力边界

| 术语 | 含义 | 所有者 | 不是什么 |
|---|---|---|---|
| 官方全推 | 正式账号认证后的非 7709 厂商持续下发；当前证据指向 5188 | `netzip-fullpull` | 7709 全市场扫描、工作表轮询、服务名里的 `full-push` |
| 补数据 | 7709 代码表、快照、K 线、F10、财务等查询 | `netzip-supplement` | 官方全推完成证据 |
| 产品编排 | 组合两者并发布到 quoteGateway | `quoteNetzipRs` | 不把某个上游端口写进产品名 |

## 硬约束

- 凭据只存在于受保护运行时配置；禁止写入仓库、日志、状态响应或对话。
- 禁止输出真实账号。测试账号类别仅记 `test-168`，且不得当作生产/全推数据源。
- 编译、测试、Clippy、release、部署走 webClx 队列，禁止本地绕过。
- 修 bug 追上游逻辑，禁止在下游吞异常或伪装成功。
- `/compact` 不是证据；压缩前必须把命令、路径、哈希和下一步实验写入权威文档。
- 抓包、pcap、diagnostics、账号密码、DLL 运行时私有文件不要当知识库上传。

## 当前阶段

官方全推主链尚未闭环。`netzip-fullpull::official_5188` 已有帧边界/重组/方向分类，
不等于认证后 5188 初始化、内层对象解码或常驻回调等价。
`POST /api/hqw/push-worklist` 仍是 7709 过渡发布。
`quoteNetzipWine` 仍是行为基准。

## Wine 对照源

Grok Bot 自己从 http://www.dbszx.net/information.html 下载官方包
`Stock股票数据API.rar`，用宿主 Wine 11.13 win32 跑 `网际风.exe`。
官网 zip 与生产目录哈希可以不同（在线升级）；版本差不作为数据源分歧。
对照用现有宿主机实例，不要再开第二个 `网际风.exe`，也不要覆盖生产文件。

- 运行目录：`/home/codes/third_party/quoteNetzipWine`
- prefix：`/config-wine/.wine-quoteNetzipWine-host`
- 监督器：`quoteNetzipWine-wine-supervisor.service`
- 调试口：`http://127.0.0.1:28787/health`、`/api/v1/status`（状态不含凭据）
- 本地下载解压：`quoteNetzipRs/.tmp/nezip-vendor/extract/StockC++/`
