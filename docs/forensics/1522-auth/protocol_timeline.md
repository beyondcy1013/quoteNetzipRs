# 7100 登录协议时序（成功登录基线）

本文件汇总 7100 认证链的已确认时序与帧清单。证据来源见 `evidence_inventory.md`。
所有偏移均为**帧内字节偏移**（0 基），长度为**线上字节数**。

> 重要前提：7100 是**两阶段**服务器，不是单次登录。
> - 阶段 1：`auth_probe`（认证测速）——小会话形态 `419 -> 345/340`。
>   这一层在旧样本见过 `6100`，也在 7100 完整 sample pcap 里见过
>   （`PROTOCOL_NOTES.md:236-239`）。
> - 阶段 2：`auth_login`（认证登录）——稳定确认 `611/271/333 -> 443/1540/395`。
>   本文以下时序只覆盖这一阶段。
>
> 来源：`PROTOCOL_NOTES.md:235-242`。

## 1. 连接四元组（已确认的真实会话）

- **服务端**：`39.108.103.69:7100`（抓包样本 `tmp/netzip_full_tcp.pcap`），
  另一处 native 实测 `121.41.70.217:7100`（`docs/codex/tasks/netzip-rs-client-progress.md:18-21`，
  `examples/auth_7100_login.rs:18-24` 默认 host）。
- **客户端**（抓包样本）：`192.168.3.38:2697`（登录会话）与 `192.168.3.38:2695`（测速会话）。
- **传输**：TCP，长连接，客户端先发。
- 四元组定位由 `src/auth_7100_flow_matrix.rs:207-225` 的 `discover_7100_endpoints`
  按"端口 == 7100"自动识别。

## 2. 一次完整成功登录的端到端时序

时间戳需要由实时抓包补齐（见 `forensic_brief.md` §A 的"必须记录的时间点"）。
下面给出**帧序列与方向**，这是离线已确认的。

| # | 方向 | 长度(B) | 角色 | 外层对象 | 关键特征 | 证据 |
|---|---|---|---|---|---|---|
| C1 | client→server | **611** | `auth_login` 请求 #1 | `认证 \| 请求=登录` | 标准 ZSTD 压缩内层；含 `账号/密码/分析软件/运营商/模块` 等明文 UTF-16 字段 | `PROTOCOL_NOTES.md:314-327` |
| S1 | server→client | **443** | `zstd_dictionary` 应答 #1 | `field44=12` 假 ZSTD 壳 | `28 b5 2f fd 60 6c 01 1d 08 00 d2 08 ...`；标准 zstd 解压失败 | `PROTOCOL_NOTES.md:414, 422-424` |
| C2 | client→server | **271** | 请求 #2（下载请求） | `压缩\|ZSTD字典` UTF-16 + ASCII `penc` | `28 b5 2f fd 20 82 c5 02 00 e4 03 0b ...`；与内存转储 `dump_271_*.bin` 字节级一致 | `PROTOCOL_NOTES.md:412, 425-437` |
| S2 | server→client | **1540** | `download_file` 应答 #2 | `数据 \| 下载文件` | **明文**，内嵌 `系统\通达信股票服务器.ini`；含 7709 服务器列表与 `账号/密码/主端口/次端口/客户ID/市场` | `PROTOCOL_NOTES.md:329-349` |
| C3 | client→server | **333** | 请求 #3（完成/握手收尾） | `压缩\|ZSTD字典` + `penc` | `28 b5 2f fd 60 ae 00 ad 04 00 52 06 ...`；与 `spawn_dump_333_*.bin` 一致 | `PROTOCOL_NOTES.md:413, 429-431` |
| S3 | server→client | **395** | `zstd_dictionary` 应答 #3 | `field44=12` 假 ZSTD 壳 | `28 b5 2f fd 60 d8 00 9d 06 00 c4 09 ...`；含 `Tdx_Encrypt` UTF-16 字样；标准 zstd 失败 | `PROTOCOL_NOTES.md:415, 407-410, 422-424` |

**成功判据**（native Rust 状态机定义）：服务端三响的角色序列**必须**为
`["zstd_dictionary", "download_file", "zstd_dictionary"]`（顺序敏感）。
来源：`src/auth_7100_client.rs:70`，`README.md:102-104`。

判定规则（`src/auth_7100_client.rs:158-178`）：
- `download_file`：`field20 == 1` 且 UTF-16 尾串含 `下载文件`；
- `zstd_dictionary`：`field20 == 4 && field44 == 12` 且尾串含 `ZSTD`。

## 3. 帧内通用结构：74 字节"网络包"前缀

每个 7100 帧都包在一个 74 字节的"网络包"对象头里（`网络包` = UTF-16LE `51 7f dc 7e`，
`PROTOCOL_NOTES.md:221`）。已确认的字段偏移：

| 偏移 | 长度 | 字段 | 已确认结论 | 证据 |
|---|---|---|---|---|
| 0..4 | 4 | 魔数 | `51 7f dc 7e` = UTF-16LE `网络包` | `PROTOCOL_NOTES.md:221` |
| 4..8 | 4 | 标志 | `05 53 00 00`（`0x5305`） | `tmp/flow_7100_2697_server.bin:0x00` |
| 20..24 | 4 | `field20`（对象类型 id） | `1`=数据/下载文件，`4`=认证/ZSTD字典 | `src/auth_7100_client.rs:158-178` |
| 32..36 | 4 | `packet_len`（u32 LE） | **整帧线上长度**（读包依据） | `src/auth_7100_client.rs:145-156` |
| 36..40 | 4 | `packet_len` 副本 | 与 32..36 重复 | `PROTOCOL_NOTES.md:388` |
| 40..44 | 4 | `field40` | 已观测 `6100/7100` 样本恒为 `2` | `PROTOCOL_NOTES.md:389` |
| 44..48 | 4 | `field44`（payload len hint） | `4`=认证请求/应答；`8`=标准 ZSTD；`12`=ZSTD字典壳 | `src/auth_7100_prefix.rs:28-36` |
| 52..56 | 4 | `field52`（attr_flags_hint） | 下载文件包里 `= 9`（正好对应 9 个子标签） | `PROTOCOL_NOTES.md:357-369, 1137` |
| 56..60 | 4 | object_span_len | `field56 - field44` 恒为 `28` | `PROTOCOL_NOTES.md:390` |
| 60..74 | 14 | UTF-16 尾串（对象名/标签） | 例：`认证`、`压缩\|ZSTD`、`数据\|下载文件` | `src/auth_7100_client.rs:158-178` |

读包函数：读 74 字节头 → `packet_len = u32 LE @ 32..36` → 继续读 `packet_len - 74`
字节作为剩余体（`src/auth_7100_client.rs:145-156`）。

## 4. C1（611B）内层明文结构（登录体）

C1 是**标准 ZSTD**，解压后是另一个 74 字节头的 `认证` 对象，内层是 UTF-16 明文字段：

| 字段 | 示例值（抓包样本） | 备注 |
|---|---|---|
| `账号` | `168`（旧样本）/ 真实账号 `1522`（native） | 长度字段在 `账号`/`密码` 前随内容变 |
| `密码` | （不记录） | 仅运行期注入，见 `auth_field_map.md` §1 |
| `分析软件` | `自定义` | |
| `运营商名称` | `泉州移动` | |
| `模块` | `股票客户端` | |
| 文件引用 | `网际风.exe` / `Stock.dll` / `Stock.字典` | 设备/版本侧通道，见 `auth_field_map.md` §3 |

native 实现把这些字段拼成 `INNER_PREFIX(74B 模板) + 账号字段 + 密码字段 + INNER_SUFFIX`，
再 `zstd::bulk::compress(inner, 3)`，最后包 `OUTER_PREFIX`（含尾 `penc`）。
来源：`src/auth_7100_client.rs:85-109`。

> 注意：native 的 C1 密码是 **UTF-16 明文 + ZSTD 压缩**，**没有** `Tdx_Encrypt`。
> 这是关键差异——真实客户端是否在 C1 就对密码做 `Tdx_Encrypt`，需要调用链取证确认
> （见 `forensic_brief.md` §D）。

## 5. S2（1540B）下载文件明文内容（后续 7709 的会话状态源）

S2 是整条链里**唯一**的明文服务端应答，也是后续 7709 行情连接的配置来源：

| 字段 | 抓包样本值 | 用途 |
|---|---|---|
| 文件名 | `系统\通达信股票服务器.ini` | |
| 来源 | `认证服务器` | |
| 压缩 | `无压缩` | 这就是它能直接当明文读的原因 |
| 账号 | `NetCardMac` | 占位/网卡 MAC 派生，**不是**用户 1522 |
| 密码 | `l123321` | **下载配置里的密码**，与用户登录密码不同 |
| 主端口 | `7709` | 后续行情主端口 |
| 次端口 | `7712` | |
| 客户ID | `1031` | |
| 市场 | `SH;SZ` | |
| 7709 服务器 IP | `124.70.183.173 / 124.71.163.106 / 180.101.48.175 / 120.195.71.160 / 122.96.107.241` | 后续行情连接目标 |

来源：`PROTOCOL_NOTES.md:330-349`；解析器：`src/auth_download.rs:111-184`。
样本文件：`tmp/flow_7100_2697_server.bin`（`src/auth_download.rs:7`）。

## 6. 待实时抓包补齐的时间点（模板）

下一次实时抓包必须记录以下时间戳（对应需求 A）：

```
T0  登录开始（点击/命令发起）
T1  TCP 连接到 7100 建立（SYN/SYN-ACK/ACK 完成）
T2  C1 发送完成
T3  S1 收到完成       <- 第一响 zstd_dictionary
T4  C2 发送完成
T5  S2 收到完成       <- 第二响 download_file
T6  C3 发送完成
T7  S3 收到完成       <- 第三响 zstd_dictionary；此时 native 判 authenticated=true
T8  股票登录完成（如果有单独的"股票登录"步骤）
T9  行情开始增长（第一条 7709 实时 tick）
```

同时记录：进程 PID、`网际风.exe` 与 `Stock.dll` 的加载基址与版本、认证服务器与行情服务器
连接四元组。完整覆盖窗口建议 **登录前 30 秒 ~ 登录后 120 秒**。
