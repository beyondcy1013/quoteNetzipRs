# 实时抓包与调用链取证清单（待人工执行）

本文件是"为什么 AI 不能自己跑这一步、以及谁来做时该怎么做"的清单。
本取证包的其他文件（`protocol_timeline.md` / `auth_field_map.md` /
`crypto_transform_notes.md` / `binary_fingerprints.md`）已经把**离线可得的全部证据**
整理完毕。剩下的必须实时取证，因为：

1. **需要真实账号 1522 的密码**——密码不可进聊天/日志/临时文件，AI 无法持有。
2. **可能与生产 Wine 会话冲突**——反复登录可能踢掉现有行情。
3. **需要在真实 `网际风.exe` 进程内 hook**——`Tdx_Encrypt` 是 `Stock.dll` 内部符号，
   不是导出函数，必须用调试器/Frida 在内部调用点取证。

所以本文件是交给"持有凭据、且能控制 Windows 进程"的人的执行清单。

## 前置：现场关键事实（别重新推导）

- 目标主程序：`D:\Soft\_Stock\飞狐2020\网际风.exe`，**32 位 i386**，
  SHA-256 `de712a8d…8509bd29`。
- 行情 DLL：同目录 `Stock.dll`，32 位，SHA-256 `524f3d11…2a9084df`，
  导出仅 `Start/Stop/Ask`。`Tdx_Encrypt`/`penc`/`hypenc`/`ZSTD` 均为内部字符串。
- 登录服务器：实测 `121.41.70.217:7100`（native 默认，`examples/auth_7100_login.rs:18-24`），
  抓包样本另见 `39.108.103.69:7100`。
- 7100 是两阶段：先 `auth_probe`（测速，419→345），再 `auth_login`（611/271/333→443/1540/395）。
- 已确认阻塞点：C2/C3（271/333B）与 S1/S3（443/395B）是 `field44=12` 的 ZSTD字典壳，
  **标准 zstd 解压失败**；C2/C3 在 native 里靠硬编码 replay。
- 工具限制：本机有 `dumpbin/strings/objdump/nm/file/python3.14`，**没有 tshark/wireshark CLI**；
  抓包建议用 `pktmon`（已用于 `capture_netzip_full.pcapng`，见 `CAPTURE_REPORT.md`）。

## A. Wine/原生成功登录基线（实时抓包）

目标：产出一次完整成功登录的脱敏 pcap + 时间线。

**安全约束**：
- 与生产 Wine 错峰；先确认此刻没有正在跑的行情会话。
- 密码只用环境变量或登录框临时输入；**不要**让 pcap 文件名、日志、截图出现密码。
- 抓包在独立目录，完成后用脚本抹除密码区，只留脱敏副本。

**执行**：
1. 启动 `网际风.exe`，用真实账号 1522 登录。
2. 抓包覆盖 **登录前 30 秒 ~ 登录后 120 秒**（pktmon `start -c --pkt-size 0`，参考
   `CAPTURE_REPORT.md` / `CAPTURE_GUIDE.md`）。
3. 记录 `protocol_timeline.md` §6 列的全部时间戳（T0..T9）。
4. 记录进程 PID、`网际风.exe`/`Stock.dll` 加载基址、认证服务器与行情服务器四元组。

**交付**：`sanitized_login_success.pcapng` + 时间线表 + SHA-256。

## B. 失败与重连对照（实时抓包）

目标：在不泄露密码的前提下，捕获 4 种情形，只报错误码/响应长度/字段偏移/状态。

| 情形 | 方法 | 关注点 |
|---|---|---|
| 1. 正常登录 | 见 §A | 基线 |
| 2. 认证服务器不可达 | 改 host 或断网后登录 | 客户端重试间隔、超时错误码、是否切换备用 |
| 3. 登录超时/断线后自动重登 | 登录成功后拔网/防火墙阻断 7100 | 重登是否重新发 C1..C3，还是复用旧 token；有无私有握手包 |
| 4. 备用服务器切换 | 触发主服务器失败 | 备用 IP 来源（是否来自 S2 下载文件列表） |

**交付**：`sanitized_login_reconnect.pcapng` + 各情形的帧清单（方向/长度/偏移/错误码，无密码）。

## C. 协议字段定位（可部分离线完成）

对 §A/§B 的 pcap，标出**所有** 7100 请求/响应帧：
`direction, timestamp, src/dst ip:port, length, frame offset`。

`protocol_timeline.md` §2-3 已给出离线已知的帧结构与偏移；实时部分需补时间戳与四元组。

字段范围定位（`auth_field_map.md` 已列已知项）：
- 账号/密码：C1 内层 UTF-16（已知）。
- 设备/版本：C1 内层 `分析软件/运营商/模块/文件引用`（已知）。
- 随机数/时间戳/token：**未发现**——需解 S1/S3 加密壳后才能确认有无（依赖 §D）。

**重点验证**（直接对应需求 C）：
1. `Tdx_Encrypt` 是否真在 7100 登录链里出现（目前只在 7709 首帧 0x118 体有证据）。
2. `penc` 在 C2/C3 之外是否还出现在 S1/S3。
3. ZSTD 字典（`field44=12`）是否真的用了字典——如果是，字典从哪来（S1？客户端内置？）。

## D. 调用链取证（解决加密缺口的关键，必须 hook）

这是"网络包看不懂时"的取证路径，也是把 `crypto_transform_notes.md` §3-4 的
❌ 推进到 ✅ 的唯一办法。**目标可执行文件是 32 位**，Frida/x64dbg 必须用 32 位实例。

### D-1. 登录请求进入 DLL 前的明文缓冲区
- Hook 点候选：`Ask`（`Stock.dll` 导出，RVA `0x000196C0`）入口。
- 记录：明文请求串（`股票数据?请求=登录…` 形式，见 `AGENTS.MD` 参考目录的调用规范）
  及其长度/哈希。
- 目的：确认"账号/密码/环境字段"在进 ZSTD 前的真实明文形态，验证 native 的
  `INNER_PREFIX+账号+密码+INNER_SUFFIX` 拼接是否与之字节级一致。

### D-2. 加密函数输入输出（Tdx_Encrypt）
- Hook 点（来自 `PROTOCOL_NOTES.md:1321-1329`）：
  - 入口疑似 `0x7b7c0 -> 0x1002bf90`
  - 上下文初始化 `0x1002da10`（写 `0x1414` 字节 + `Tdx_Encrypt` 名）
- 记录：每次调用的输入缓冲区（长度/哈希/前 64 字节）、输出缓冲区（同上）、
  `0x1414` 字节上下文内容（可能含密钥/字典）。
- 脱敏：密码区域用**固定长度掩码**替换，保留长度与偏移。
- 目的：还原 `Tdx_Encrypt` 算法，或至少拿到多组明文/密文对用于差分。

### D-3. DLL 发出的最终网络缓冲区
- Hook 点：`WS2_32.send`/`WSASend`（`网际风.exe` 直接依赖 WS2_32，见
  `binary_fingerprints.md` §1）。
- 记录：每次 send 的字节、目标四元组、调用栈前 5 帧。
- 目的：把"DLL 内部缓冲区 → 线上帧"的边界钉死，验证 `cipher_0118.bin == 线上首帧 payload[2..]`
  （`TDX118_DUMP_GUIDE.md:113`）。
- 同时复核 `plain_0118.bin`/`cipher_0118.bin` 的 hook 点是否正确（见
  `crypto_transform_notes.md` §4.3）。

### D-4. 收到响应后的解码结果
- Hook 点：`WS2_32.recv` 返回后，跟踪缓冲区进入 DLL 解码路径。
- 记录：S1/S3（443/395B）解壳后的明文（如果能解）；重点找 session token / 随机数 / 时间戳。
- 目的：回答 `auth_field_map.md` §5 的核心未决——7100 登录是否产出喂给 7709 的 token。
- 每个缓冲区只留脱敏版本 + 长度/哈希。

## E. 交付物清单（对应需求 E）

| 交付物 | 当前状态 | 负责 |
|---|---|---|
| `sanitized_login_success.pcapng` | 待产出（§A） | 持凭据的 Windows 操作者 |
| `sanitized_login_reconnect.pcapng` | 待产出（§B） | 同上 |
| `protocol_timeline.md` | **已完成离线部分**；待补实时时间戳（§A） | 本包已写 |
| `auth_field_map.md` | **已完成已知项**；随机数/token 待 §D-4 | 本包已写 |
| `crypto_transform_notes.md` | **已完成已知项**；Tdx_Encrypt/字典壳待 §D-2 | 本包已写 |
| 每个文件的 SHA-256 | 已完成（`evidence_inventory.md`）；新增文件需补 | 本包已写 |

## F. 最终结论（本包已可回答的 4 个问题）

> 需求 E 列了 4 个最终结论。基于现有离线证据，现在就能回答 3 个；第 2 个部分依赖 §D。

1. **是否足以在 Rust 中复现 7100 登录？**
   - **窄义够，广义不够。** 一次性登录（C1 动态 + C2/C3 replay）已验证可成功
     （`docs/codex/tasks/netzip-rs-client-progress.md:18-21`）。
     但无法为新会话动态生成 C2/C3，也无法解密 S1/S3，**不是协议级等价复现**。

2. **还缺哪些字段或算法？**
   - 缺：`field44=12` ZSTD字典壳的可逆实现（C2/C3 生成、S1/S3 解密）。
   - 缺：`Tdx_Encrypt` 算法本体（疑似与上述壳同源，待 §D-2 证实）。
   - 缺：penc/hypenc 语义（不阻塞登录，阻塞字段对齐）。
   - 待证：C1 内层密码是否需要先过 `Tdx_Encrypt`（native 当前是明文+ZSTD）。

3. **登录成功后 Rust 应保存哪些会话状态？**
   - 见 `auth_field_map.md` §6：S2 下载文件里的 7709 服务器列表、主/次端口、
     `账号=NetCardMac`/`密码=l123321`/`客户ID=1031`、市场 `SH;SZ`。
   - 待证：S1/S3 内是否藏 token/随机数喂给 7709（依赖 §D-4）。

4. **最小可行 Rust 实现步骤？**
   - 见 `crypto_transform_notes.md` §7-4：在 P0（字典壳/Tdx_Encrypt）解决前，
     维持"C1 动态 + C2/C3 replay"，但在 `login_auth_7100` 成功分支里**接上**
     `auth_download.rs` 的 S2 解析器，把下载配置转成 7709 连接参数。
     这是"可工作的窄复现"，不是"协议级等价复现"。

## G. 工作顺序（重申，来自原始需求）

1. 先抓一次成功登录的完整时序（§A）；
2. 再定位 DLL 内"明文登录体 → 最终网络帧"的边界（§D-1/D-2/D-3）；
3. 最后才分析加密算法与 Rust 实现（§D-2 多组样本 → 还原 → 接进 Rust）。

不要只交付"抓包文件"。真正需要的是"抓包 + 调用链 + 字段偏移 + 成功/失败对照"。
