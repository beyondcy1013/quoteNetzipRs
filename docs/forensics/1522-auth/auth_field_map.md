# 7100 登录字段映射（账号 / 密码 / 设备 / 随机数 / 时间戳 / token）

本文件把 7100 登录链里**每个业务字段**的线上位置、编码方式、来源和证据列清楚，
并标注哪些已可由 Rust 复现、哪些仍需取证。

字段编码五档（与需求 C 对齐）：
- **明文 UTF-16**：`penc`/`hypenc` marker 之外的纯文本，直接可读。
- **标准 ZSTD**：`28 b5 2f fd` 开头且不带字典，标准解压可还原。
- **ZSTD 字典壳（field44=12）**：`28 b5 2f fd` 开头但标准 zstd 解压失败。
- **Tdx_Encrypt**：DLL 内部算法，输出特征是 `0x118` 字节体的 8 字节块变换痕迹。
- **penc/hypenc**：未知语义的 ASCII marker，总是贴在 payload 边界附近。

## 1. 账号（account）

| 项 | 结论 | 证据 |
|---|---|---|
| 出现位置 | C1（611B）解压后的内层 `认证` 对象 | `PROTOCOL_NOTES.md:318` |
| 字段名 | `账号`（UTF-16） | |
| 默认值 | `1522`（native 默认账号） | `src/auth_credentials.rs:3` `DEFAULT_ACCOUNT` |
| 注入方式 | 环境变量 `NETZIP_TDX_ACCOUNT`（缺省回落 `1522`） | `src/auth_credentials.rs:4, 15-30`；`examples/auth_7100_login.rs:14-17` |
| 编码 | 明文 UTF-16，拼在 `INNER_PREFIX` 之后（`append_string_field("账号", account)`） | `src/auth_7100_client.rs:90` |
| 外层保护 | 整个内层 `认证` 体经 `zstd::bulk::compress(inner, 3)` | `src/auth_7100_client.rs:97` |
| Rust 可复现 | **是**（C1 动态生成已验证） | `examples/auth_7100_login.rs`，`docs/.../netzip-rs-client-progress.md:18-21` |

注意：抓包样本里 C1 的 `账号` 值是 `168`，与 native 的 `1522` 不同——说明字段是动态的，
不是硬编码，**已经能证明账号字段可被 Rust 正确注入**。

## 2. 密码（password）

| 项 | 结论 | 证据 |
|---|---|---|
| 出现位置 | C1 内层，紧跟 `账号` 字段 | `PROTOCOL_NOTES.md:319`（字段名为 `密码`，长度 168） |
| 字段名 | `密码`（UTF-16） | |
| 注入方式 | 环境变量 `NETZIP_TDX_PASSWORD`（无文件、无硬编码） | `src/auth_credentials.rs:5, 15-23`；`examples/auth_7100_login.rs:19-21` |
| **native 当前编码** | **明文 UTF-16 + ZSTD 压缩**，`append_string_field("密码", password)` 后整体 zstd(3) | `src/auth_7100_client.rs:91, 97` |
| **未确认（关键缺口）** | 真实 `网际风.exe` 是否在 C1 内层对密码做 `Tdx_Encrypt` 再进 ZSTD？native 的"明文密码直接 ZSTD"是否就是真实协议？ | 需调用链取证（`forensic_brief.md` §D-1/D-2） |
| 安全处理 | 密码**绝不**写入源码/日志/pcap 命名/提交；只运行期注入 | `AGENTS.MD` 修复原则 + 本包安全约束 |

> 这是最需要调用链取证的一点：native 当前能登录成功，说明"明文密码+ZSTD"至少被服务端
> 接受了；但**是否完整等价于真实客户端**，取决于 C1 内层在进入 ZSTD 前是否还经过
> `Tdx_Encrypt`。如果真实客户端多了一层加密，native 的"成功"可能只是服务端对弱形态的
> 宽容，长期/异常路径下会分叉。

## 3. 设备 / 版本侧通道字段

C1 内层还携带一批"环境自描述"字段，目前当设备/版本指纹理解：

| 字段 | 抓包样本值 | 编码 | 证据 |
|---|---|---|---|
| `分析软件` | `自定义` | 明文 UTF-16 | `PROTOCOL_NOTES.md:320` |
| `运营商名称` | `泉州移动` | 明文 UTF-16 | `PROTOCOL_NOTES.md:321` |
| `模块` | `股票客户端` | 明文 UTF-16 | `PROTOCOL_NOTES.md:322` |
| `网际风.exe` | 文件名引用 | 明文 UTF-16 | `PROTOCOL_NOTES.md:323`；`src/auth_7100_client.rs:10` `INNER_SUFFIX_HEX` |
| `Stock.dll` | 文件名引用 | 明文 UTF-16 | 同上 |
| `Stock.字典` | 文件名引用 | 明文 UTF-16 | 同上 |
| 文件 hash 片段 | `f78b426c` / `ea819a5b` | 固定 4 字节，在 `INNER_SUFFIX` | `src/auth_7100_client.rs:10` |

native 当前把这些字段作为**固定 hex 模板** `INNER_SUFFIX_HEX` replay（`src/auth_7100_client.rs:10`），
不随现场 `Stock.dll` 的真实 hash 变化。**未确认**：服务端是否校验这些字段与真实文件 hash 一致。

## 4. penc / hypenc marker（未知语义，但位置已对齐）

`penc` 与 `hypenc` 是 ASCII marker，**已在 `Stock.dll` 字符串里命中**（见
`binary_fingerprints.md` §1），由 DLL 在运行期发出。

| marker | 已确认位置 | 含义 | 证据 |
|---|---|---|---|
| `penc`（首） | 下载文件包 payload 边界 `-8`；C1 外层尾 | 未定（疑似 payload 分隔） | `PROTOCOL_NOTES.md:124-127`；`src/auth_7100_client.rs:204` 断言 `packet[162..166]==b"penc"` |
| `penc`（次） | 下载文件包 `penc_offsets=[60, 416]` 的第二个 | 未定 | `src/auth_7100_flow_matrix.rs:866`；`PROTOCOL_NOTES.md:114-115` |
| `hypenc` | local-2000 桥 `hypenc@68` | 未定；与 7100 关系仍在研究 | `PROTOCOL_NOTES.md:115`；`README.md:190` |

**native 的处理**：`auth_7100_client.rs` **从不**生成或解析 `hypenc`；`penc` 只作为外层
`OUTER_PREFIX` 的固定尾字节出现。`hypenc`/次 `penc` 的语义是显式未决问题
（`PROTOCOL_NOTES.md:1128-1141`）。

## 5. 随机数 / 时间戳 / token

| 字段 | 现状 | 证据 |
|---|---|---|
| 随机数 | **未发现**独立的 challenge/random 字段。C1/外层未见显式 nonce；S1/S3 的 `field44=12` 壳内是否有随机量，需解壳后才能判断 | 无 |
| 时间戳 | 74 字节头**没有**时间戳字段；C1 内层未见 Unix 时间戳。可能藏在 S1/S3 加密壳里 | 无 |
| token / cookie / session id | **7100 链里没有任何被解析出来的 token**。`src/` 全量 grep `token/cookie/session`：7100 文件 0 命中（`session` 仅出现在结构体名 `Auth7100FlowSession`） | grep 结果；`auth_field_map` 无 token 字段 |

> 关于 7709：后续 7709 的 `0x0547` 请求项从 7B 扩成 11B，多出的 `u32` 像 token
> （`PROTOCOL_NOTES.md:850`），但**没有证据**这个 token 来自 7100 登录。
> 7709 有自己的 bootstrap（`0x7b00 -> 0x9400 -> 0x9900`，`README.md:1029`）。
> 结论：**当前不能断言"7100 登录产出的某字段被喂给 7709"**，需要调用链取证确认
> （`forensic_brief.md` §D-4）。

## 6. 会话状态：成功登录后 Rust 应保存什么

综合 S2 明文下载文件（`auth_field_map` §5 of `protocol_timeline.md`）与 7709 自举，
**Rust 端在 7100 登录成功后至少应保存**：

1. **服务器列表**：5 个 7709 IP + 主端口 7709 + 次端口 7712（来源 S2）。
2. **下载配置里的凭据**：`账号=NetCardMac`、`密码=l123321`、`客户ID=1031`
   （用于 7709 自举，与用户 1522 不同）。
3. **市场集合**：`SH;SZ`。
4. **登录元数据**：endpoint、account、`response_packet_lengths`、`response_roles`、时间戳。
   （native 已返回前 4 项，缺时间戳与会话句柄。）

**未确认需保留**：
- 如果 S1/S3 加密壳解开后含有 session token / 随机数 / 时间戳，必须一并保存并验证
  是否喂给 7709。这是 `crypto_transform_notes.md` 列的最高优先缺口。

来源：`src/auth_download.rs:21-53`（`DownloadedServerConfig` 结构已定义全部上述字段）。
