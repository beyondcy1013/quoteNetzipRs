# 目标可执行文件指纹（静态取证）

采集环境：Windows (win32), `D:\Soft\_Stock\飞狐2020\`。
工具：`sha256sum`, `file` (file-5.46), `dumpbin` (MSVC 14.44.35220), GNU `strings` (binutils 2.39)。
采集日期：2026-08-06。

> 注：本目录另一处 `netzip_api_bin/NetzipAPI/StockC++/` 下还有一套历史样本
> `Stock64.dll / Stock.dll / 网际风.exe`，那是官方/历史参考资料，**与现场实际运行的
> 这一套不是同一文件**。取证必须以现场这套为准。

## 1. 现场关键文件

### `网际风.exe`
- **SHA-256**：`de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29`
- **大小**：2,067,968 字节
- **修改时间**：2025-05-31 22:23
- **PE 类型**：`PE32 executable for MS Windows 6.00 (GUI), Intel i386, 7 sections`
- **位数**：**32 位（i386）** —— 抓包/调试器/Frida 必须用 32 位目标。
- **依赖 DLL**（dumpbin `/dependents`）：`KERNEL32 USER32 ADVAPI32 SHELL32 WS2_32 IPHLPAPI dbghelp`
  - 含 `WS2_32` 与 `IPHLPAPI`：走 Winsock2 直连，DNS/路由解析自带，**不是只走 Stock.dll**。

### `Stock.dll`（现场真正被加载的行情 API DLL）
- **SHA-256**：`524f3d11ab53211d437ded738bb1a00d29b11a16aed8167518c9aa982a9084df`
- **大小**：753,152 字节
- **修改时间**：2025-02-28
- **PE 类型**：`PE32 executable for MS Windows 6.00 (DLL), Intel i386, 5 sections`
- **位数**：32 位。
- **导出函数**（dumpbin `/exports`，仅 3 个）：
  - `Start` @ RVA `0x000195A0`（ordinal 1）
  - `Stop`  @ RVA `0x00019620`（ordinal 2）
  - `Ask`   @ RVA `0x000196C0`（ordinal 3）
  - 即官方规范里的 `Start / Ask / Stop` 调用入口；**`Tdx_Encrypt` 不是导出函数**，
    是 `Stock.dll` 内部符号，只能用调试器/Frida 在内部调用点取证（见
    `forensic_brief.md` §D）。
- **加密相关字符串命中**（`Stock.dll` 内）：
  - ASCII：`penc`、`hypenc`、`c6e0Rpenc`、`epenc`、`lpenc`、`penc"`、`penc{`
  - UTF-16LE：`ZSTD`
  - **`Tdx_Encrypt`**（UTF-16LE）—— 证实算法名是真实内部符号，与 `PROTOCOL_NOTES.md:1326`
    记录的 `0x1002da10` 处把 `Tdx_Encrypt` 放进 0x1414 字节上下文的结论一致。

### `系统\Stockdrv.dll`（底层驱动/依赖）
- **PE 类型**：`PE32 executable for MS Windows 5.01 (DLL), Intel i386, 5 sections`
- 32 位；较老子系统（5.01）。本包不深入它。

## 2. 关键发现（对比既有笔记）

| 结论 | 证据 | 意义 |
|---|---|---|
| `penc` / `hypenc` 是 `Stock.dll` 内部真实字符串 | `strings Stock.dll` 命中 | 这两个 marker 由 DLL 在运行期发出，不是网络侧伪影 |
| `Tdx_Encrypt` 是 `Stock.dll` + `网际风.exe` 共有 UTF-16 符号 | `strings -e l` 双双命中 | 与 `PROTOCOL_NOTES.md:1321-1329` 的 DLL 调用链一致 |
| `Stock.dll` 只导出 `Start/Stop/Ask` | dumpbin `/exports` | 任何"明文→密文"取证必须在 DLL **内部** hook，不能通过导出表 |
| 现场是 32 位 i386 | `file` 全部命中 i386 | Frida/x64dbg 必须是 32 位实例；`Stock64.dll`（64 位）不在现场 |

## 3. 参考资料目录指纹（供回溯，**非**现场文件）

仓库 `netzip_api_bin/NetzipAPI/StockC++/` 下的样本是官方/历史参考（见 `AGENTS.MD`
"参考目录"章节），不应与现场混用。后续如需对参考样本取证，应单独建文件并标注
"参考样本，非现场"。
