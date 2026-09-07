# 加密 / 压缩变换笔记（penc / ZSTD字典 / Tdx_Encrypt / hypenc）

本文件把 7100 登录链涉及的每个变换按"已可逆 / 部分理解 / 完全未还原"三档归类，
记录输入输出长度、固定头、可重复样本，并指出还原它还需要什么证据。

## 0. 总览（一句话现状）

- **标准 ZSTD**（C1 登录体、C1' 测速体）：✅ 已可逆，native 能动态生成并 round-trip。
- **下载文件明文**（S2, 1540B）：✅ 无加密，直接可读。
- **ZSTD 字典壳（field44=12）**（C2/C3/S1/S3）：❌ 标准解压报 `Data corruption detected`，
  壳内结构像 ZSTD 帧头但 body 不可解。**这是纯 Rust 复现的最大阻塞**。
- **Tdx_Encrypt**：❌ 算法名已在二进制确认，调用链已定位，但**算法本身未还原**。
  证据是 0x118 字节体的 8 字节块变换痕迹。
- **penc / hypenc**：⚠️ marker 位置已对齐，**语义未定**。

## 1. 标准 ZSTD —— ✅ 已可逆

| 项 | 值 | 证据 |
|---|---|---|
| 帧头魔数 | `28 b5 2f fd` | `src/auth_7100_flow_matrix.rs:13` `ZSTD_MAGIC` |
| 用于哪些包 | C1（611B 登录体内层）、测速 C1'（419B） | `PROTOCOL_NOTES.md:315, 259-260` |
| 压缩级别 | level 3（native） | `src/auth_7100_client.rs:97` `zstd::bulk::compress(&inner, 3)` |
| 可逆验证 | native 构建 C1 后用 `zstd::stream::decode_all` round-trip 回原始内层 | `src/auth_7100_client.rs:212-213` |
| 419B 测速样本 | 单帧压缩长 249B，解压长 434B | `PROTOCOL_NOTES.md:259-260` |

结论：标准 ZSTD 这一层**已经完整可逆**，Rust 复现无障碍。

## 2. 下载文件明文 —— ✅ 无加密

S2（1540B）整包是明文 `数据|下载文件`，含 UTF-16 INI 文本。
- 定位方法：找 UTF-16LE BOM + `[`（`ff fe 5b 00`）作为嵌入文本起点
  （`src/auth_download.rs:234, 271`）。
- 已实现解析器：`src/auth_download.rs:111-184`。
- 这是后续 7709 会话状态的唯一明文来源（见 `auth_field_map.md` §6）。

## 3. ZSTD 字典壳（field44=12）—— ❌ 未还原（核心阻塞）

这是纯 Rust 复现的**最大障碍**。特征：

| 包 | 长度 | 帧头字节（含 ZSTD 魔数） | 标准 zstd 结果 | 证据 |
|---|---|---|---|---|
| C2 | 271 | `28 b5 2f fd 20 82 c5 02 00 e4 03 0b ...` | 失败 | `PROTOCOL_NOTES.md:412, 422-424` |
| C3 | 333 | `28 b5 2f fd 60 ae 00 ad 04 00 52 06 ...` | 失败 | `PROTOCOL_NOTES.md:413` |
| S1 | 443 | `28 b5 2f fd 60 6c 01 1d 08 00 d2 08 ...` | 失败 | `PROTOCOL_NOTES.md:414` |
| S3 | 395 | `28 b5 2f fd 60 d8 00 9d 06 00 c4 09 ...` | 失败 | `PROTOCOL_NOTES.md:415` |

帧头解析（native 已实现 `summarize_zstd_frame`，`src/auth_7100_flow_matrix.rs:393-489`）：
- S1：`frame_content_size = 364`，`block_size = 259`（`PROTOCOL_NOTES.md:417-421`）
- S3：`frame_content_size = 216`，`block_size = 211`
- 即**帧头能解析，block body 不能解压**。报错：`Data corruption detected`。

关键判断：
> "这几包更像是 `penc + 字典化/加密` 壳，不是前面 419/345/611 那种标准 Zstd 帧"
> —— `PROTOCOL_NOTES.md:424`

证据强度：
- `auth_flow_sample.rs:413-418` 的测试**显式断言**样本里的 field44=12 帧 `!decode_ok && decode_error.is_some()`。
- 即"解不开"是**被回归测试钉死的事实**，不是没试过。

native 的当前应对：把 C2/C3（271B/333B）作为**硬编码 hex 常量** replay
（`src/auth_7100_client.rs:12-13` `FOLLOWUP_DOWNLOAD_HEX` / `FOLLOWUP_FINISH_HEX`）。
**这只能复用抓到的那一次会话的字节，无法为新会话/新随机量重新生成。**

## 4. Tdx_Encrypt —— ❌ 算法未还原

### 4.1 二进制侧已确认
- `Tdx_Encrypt` 字符串同时存在于 `网际风.exe` 与 `Stock.dll`（UTF-16LE）。
  见 `binary_fingerprints.md` §1。
- DLL 调用链（来自 `PROTOCOL_NOTES.md:1321-1329`）：
  - 入口疑似 `0x7b7c0 -> 0x1002bf90`
  - `0x1002da10` 初始化一个 `0x1414` 字节上下文，并把算法名 `Tdx_Encrypt` 放进去
- `Tdx_Encrypt` **不是导出函数**（`Stock.dll` 仅导出 `Start/Stop/Ask`），只能内部 hook。

### 4.2 输出特征（部分理解）
0x118（280）字节体是 Tdx_Encrypt 的疑似输出载体：
- 7709 首帧 292B = `0x010c / 0x7b00 / tag=0x000b / payload=282`，其中 `payload = 2 + 0x118`
  （`PROTOCOL_NOTES.md:1332-1334`）。
- 0x118 体呈现"8 字节粒度的变换/加密迹象"——存在重复 8 字节块模式。
- **保守结论**："存在 8 字节块变换，但不能仅凭重复块断定是简单 XOR"
  （`PROTOCOL_NOTES.md:1337-1340`）。即**算法未定**，只观测到块结构。

### 4.3 取证样本是否已就绪
- 已有疑似样本对：`plain_0118.bin` / `cipher_0118.bin`（各 280B）
  —— `TDX118_DUMP_GUIDE.md` 的目标产物。
- 成功判据（`TDX118_DUMP_GUIDE.md:114-115`）：`plain_0118.bin` 里应能看到
  `中信证券 / NetCardMac / 账号 / 密码 / 服务器 IP` 的明文痕迹。
- **当前状态**：`plain_0118.bin` 是否已真正 hook 到明文侧，需要复核；
  `cipher_0118.bin` 应与线上首帧 `payload[2..]` 字节级一致（`TDX118_DUMP_GUIDE.md:113`）。
  见 `forensic_brief.md` §D-3 的复核清单。

### 4.4 还原 Tdx_Encrypt 还需要什么
1. **可靠的明文/密文对**：确认 `plain_0118.bin` 是 hook 在 `Stock.dat + 0x10066b60`
   （加密前明文）取得的，`cipher_0118.bin` 是 hook 在 `0x1007b7c0`/`0x1002bf90`
   （发出前密文）取得的。
2. **多组样本**：至少 2 组不同输入的明文/密文对，才能区分"固定 XOR/置换"与"带密钥"。
3. **算法上下文 dump**：`0x1414` 字节上下文的内容（可能含密钥/字典）。
4. **是否与 field44=12 的 ZSTD 字典壳同源**：C2/C3 是否就是 `Tdx_Encrypt` 后再套 ZSTD 帧头？
   这是把第 3、4 节缺口合并的关键假设。

## 5. penc / hypenc —— ⚠️ 位置已对齐，语义未定

### penc
- `Stock.dll` 内真实 ASCII 字符串（`binary_fingerprints.md` §1）。
- 首个 `penc` 位置稳定：下载文件包 payload 边界 `-8`；C1 外层尾（native 断言
  `packet[162..166]==b"penc"`，`src/auth_7100_client.rs:204`）。
- 下载文件包有两个 penc：`penc_offsets=[60, 416]`（`src/auth_7100_flow_matrix.rs:866`）。
- **未确认**：是分隔符？长度前缀？还是触发某变换的开关？

### hypenc
- `Stock.dll` 内真实 ASCII 字符串。
- 仅在 local-2000 桥路径观测到（`hypenc@68`），**7100 侧暂未观测到**
  （`PROTOCOL_NOTES.md:114`）。
- native `auth_7100_client.rs` **从不**生成或解析 `hypenc`（grep 0 命中）。
- **未确认**：`hypenc` 是否是 7709/本地桥专用，7100 链是否真的没有。

`PROTOCOL_NOTES.md:1128-1141` 与 `README.md:190-192` 把这两项列为显式未决问题。

## 6. 变换链总结图

```
真实客户端（待证实）：
  明文(账号/密码/环境) 
    -> [?] Tdx_Encrypt(0x118体, 8字节块变换)      ❌ 未还原
    -> ZSTD字典壳(field44=12, penc wrapper)        ❌ 标准zstd失败
    -> 74B网络包头                                  ✅ 已理解
    -> TCP 7100

native Rust 当前：
  明文(账号/密码/环境) 
    -> (不做 Tdx_Encrypt)                          
    -> 标准ZSTD(level3)                            ✅ 可逆
    -> 74B网络包头 + 固定penc尾                      ✅ 可逆
    -> TCP 7100
  + C2/C3 用硬编码 replay（含未还原的 ZSTD字典壳）   ⚠️ 仅限单次会话
```

中间的差距就是"是否经过 Tdx_Encrypt + ZSTD字典壳"。native 靠"明文+标准ZSTD"能登录成功，
说明服务端当前接受这种弱形态，但**不等价于真实客户端**，长期/异常路径可能分叉。

## 7. 对 Rust 复现的影响（结论）

1. **能复现**：一次性的 7100 登录（C1 动态 + C2/C3 replay）——已验证可成功
   （`docs/.../netzip-rs-client-progress.md:18-21`）。
2. **不能复现**：
   - 为**新**会话生成 C2/C3（依赖未还原的 ZSTD字典壳）。
   - 解密 S1/S3（同上），所以也无法验证里面是否藏 session token / 随机数。
   - 完整 `Tdx_Encrypt` 等价实现。
3. **必须先解的缺口**（优先级）：
   - P0：还原 field44=12 ZSTD字典壳（C2/C3/S1/S3）——决定能否动态生成与解密。
   - P0：确认 Tdx_Encrypt 是否与该壳同源（即壳 body 是否就是 Tdx_Encrypt 输出）。
   - P1：penc/hypenc 语义——影响字段对齐，但不阻塞登录本身。
4. **最小可行 Rust 实现**（在 P0 解决前）：
   - 维持"C1 动态 + C2/C3 replay"现状，但把 replay 字节与 session 绑定，
     并在登录成功后**立即**把 S2 明文下载配置解析为 7709 连接参数（`src/auth_download.rs`
     已有解析器，只差把它接进 `login_auth_7100` 的成功分支）。
   - 这是"可工作的窄复现"，不是"协议级等价复现"。
