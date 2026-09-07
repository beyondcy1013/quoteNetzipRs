# 证据清单（SHA-256 + 大小 + 用途 + 脱敏状态）

所有哈希于 2026-08-06 在 `Z:\quoteNetzipRs` 计算（`sha256sum`）。
"脱敏状态"列说明该文件是否已抹除密码：本仓库现有证据文件**均未做密码区脱敏**，
但它们不含明文用户密码（密码只在运行期注入），含的是下载配置里的次级密码 `l123321`。

## 1. 网络抓包与流样本

| 文件 | 大小 | SHA-256 | 用途 | 脱敏 |
|---|---|---|---|---|
| `captured_windows_traffic/capture_netzip_full.pcapng` | 16,405,420 | `2a033d8c4663bdab642cd0791c053873c306cd989f69b97a11a8def377f9c70d` | 完整 payload 抓包（pktmon `--pkt-size 0`），88 包/20 唯一/0 截断，7100 时序主证据 | 未脱敏（无用户明文密码） |
| `captured_windows_traffic/capture_netzip.pcapng` | 2,884,812 | `6921dd5d262196d1c38f37119f3cfa44b07a07556ce36d5f44a7064827a85470` | 早期截断抓包（仅 74B 可见前缀） | 未脱敏 |
| `tmp/netzip_full_tcp.pcap` | 16,045,116 | `5b5a5d39ef707ae77b4472a269fa3f31b00e9831fb8a20b17125e8403b01b345` | Rust 端 7100 流矩阵分析输入（`auth_7100_flow_matrix.rs:14` 默认路径） | 未脱敏 |
| `tmp/flow_7100_2697_server.bin` | 2,378 | `aca9526d7226b23679585f8fa14a4b66d1f143ca551d309a4075d13492d1bdc8` | 会话 `192.168.3.38:2697 <-> 39.108.103.69:7100` 的服务端流；1540B 下载文件解析来源（`auth_download.rs:7`） | 未脱敏 |

## 2. 已提取方向流（raw）

| 文件 | 大小 | SHA-256 | 用途 |
|---|---|---|---|
| `captured_windows_traffic/client_to_server_full.raw` | 3,352 | `54eeb6c9a55356b4ad281f4248020d982293f73deebc7158fbc36254f439c196` | 8×419B 测速请求 |
| `captured_windows_traffic/server_to_client_full.raw` | 2,760 | `237d52a55627af455d54891e254c7d6a9b03b9f5327dc5290ad9e188c3aa0099` | 8×345B 测速应答 |
| `captured_windows_traffic/client_to_server.raw` | 1,184 | `05f435183ae6ff28204021e3f466dc0b798dd724825b7118fdc7b97958736416` | 早期截断提取 |
| `captured_windows_traffic/server_to_client.raw` | 319,088 | `f7d30da05e43378f7dbef1a0e59c996eeb497b9d1477dcc849a867bcbafb327c` | 早期截断提取 |
| `captured_windows_traffic/CAPTURE_REPORT.md` | 2,319 | `20bf97a2adcb1fbb1505dc3669effcd7236b9c9d2d6e60ee6ef6b6cfcae7f10f` | pktmon 全 payload 抓包方法记录 |

## 3. 加密样本对（0x118 体）

| 文件 | 大小 | SHA-256 | 用途 |
|---|---|---|---|
| `plain_0118.bin` | 280 | `d3bf34cd48c20192ba3e511c96b8a4395fbf5bb494fe5ed8b78c6c604d87faee` | 疑似 Tdx_Encrypt 明文输入（待复核 hook 点正确性，见 `crypto_transform_notes.md` §4.3） |
| `cipher_0118.bin` | 280 | `fe4bf159175aa193e5756ba1e5988f58b03766d023ab262993ca123f018a4190` | 疑似 Tdx_Encrypt 密文输出；应与线上首帧 `payload[2..]` 一致 |

## 4. 协议文档

| 文件 | SHA-256 | 用途 |
|---|---|---|
| `PROTOCOL_NOTES.md` | `848aaf6a991b271d0c4091d1b091f3588485ac346453a188be1a499c5e0efac0` | 111KB 协议主笔记（7100/7709/0547） |
| `README.md` | `f0c9aec63d94627ad1caa51bd241d3e5562646c19b383576ea5950fdad8162f2` | 项目主文档 |
| `docs/codex/tasks/netzip-rs-client-progress.md` | `bfcc810392b817f37f388b337899f7c4b2f4a9ac64dab62b8686814ed9407c8b` | native 7100 进度与验收项 |
| `TDX118_DUMP_GUIDE.md` | `ebb5a7d9704658fe3e791efcf2b1b7801031b34c815b74a8cadc043f2bcc60cd` | 0x118 明文/密文对取证指南 |
| `WINDOWS_PROCESS_CAPTURE_GUIDE.md` | `e0a37e1b0940f11bf29d76bb6f5a82563542aadf68c62f0b3f1ea18f94eff833` | Windows 进程抓包指南 |
| `CAPTURE_GUIDE.md` | `4b155b8e0ab580f8a2dc2fe17ad9e5a3fb2fa079c2dec37f06cb0d74527f0cbb` | 抓包方法指南 |

## 5. 关键 Rust 源（实现指纹）

| 文件 | SHA-256 | 用途 |
|---|---|---|
| `src/auth_7100_client.rs` | `6264978678f6a7cd7a96039641019c79849e49f87ba2717379e51cff1ac87aaa` | live 登录状态机 + C1 动态构造 + C2/C3 硬编码 replay |
| `src/auth_credentials.rs` | `9672cee9e6293601d3003e293dfcde30900dc26f1a99c5e32eec367683fe873f` | 账号/密码环境变量加载（`1522` 默认） |
| `src/auth_download.rs` | `0d9665effa73abd055ae740bb2d0f7b7a2b66f68844103fad3bd38e7640bda1b` | 1540B 下载文件明文解析器 |
| `src/auth_7100_flow_matrix.rs` | `9551c84b136c53258982bd2d72b37041c2377e917621ccad45bed1b239a38090` | 离线 pcap→会话→netpacket 分析器（ZSTD 帧头解析、penc/hypenc 偏移） |
| `src/auth_flow_sample.rs` | `12e677479978149c4008078db0271700355e6beaec6e06f53b0406f94f840d2d` | 离线 .bin 流分析；含"ZSTD 解压失败"回归测试 |
| `examples/auth_7100_login.rs` | `44fd8f2124af5cb369187d627fcc952ff8299a1ec5e1111923850837caeda9aa` | native 7100 登录 CLI（环境变量凭据） |

## 6. 目标可执行文件（见 `binary_fingerprints.md`）

| 文件 | SHA-256 | 备注 |
|---|---|---|
| `D:\Soft\_Stock\飞狐2020\网际风.exe` | `de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29` | 32 位 GUI 主程序 |
| `D:\Soft\_Stock\飞狐2020\Stock.dll` | `524f3d11ab53211d437ded738bb1a00d29b11a16aed8167518c9aa982a9084df` | 32 位 API DLL，导出 `Start/Stop/Ask` |
| `D:\Soft\_Stock\飞狐2020\系统\Stockdrv.dll` | （未算） | 32 位底层驱动依赖 |

## 7. 还需产出（待实时取证，模板见 `forensic_brief.md`）

| 计划文件 | 状态 | 说明 |
|---|---|---|
| `sanitized_login_success.pcapng` | **待产出** | 一次完整成功登录的脱敏 pcap（覆盖登录前 30s ~ 登录后 120s） |
| `sanitized_login_reconnect.pcapng` | **待产出** | 断线自动重登/备用服务器切换的脱敏 pcap |
| 失败对照（认证服务器不可达） | **待产出** | 错误码/响应长度/字段偏移/状态（无密码） |

> 安全重申：实时抓包的原始 pcap 只在独立测试目录短时存在；分析后用固定长度掩码替换密码区，
> 只保留脱敏副本与长度/哈希。密码不进 pcap 命名、不进日志、不进聊天。
