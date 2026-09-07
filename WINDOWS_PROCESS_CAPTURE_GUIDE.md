# Windows Process Capture Guide

目标：先确认“网际风目标进程到底连了哪些 `ip:port`”，再只抓这些真实属于目标进程的流量。

这一步是为了排除之前“同时开着通达信，导致 `7709` 流量混进样本”的问题。

## 结论先说

这轮不要先假设端口是 `7709 / 7719 / 6100 / 14017` 中的哪一个。

先做两件事：

1. 只启动目标程序，确认它真实连出的远端地址和端口。
2. 只抓这些由目标进程实际建立的会话。

如果这一轮确认目标进程根本没有连 `7709`，那之前那批 `7709` 样本就整体降级成旁支参考，不能再当主证据。

## 你需要准备

- Windows 机器
- 目标程序
- Wireshark
- Sysinternals TCPView
- 管理员 PowerShell

可选但推荐：

- x64dbg
- Frida

## Step 0. 先清环境

这一步很重要。

1. 关掉所有证券软件：
   - 通达信
   - 大智慧
   - 同花顺
   - 任何自写行情工具
2. 最稳的做法是直接重启 Windows。
3. 重启后只打开：
   - Wireshark
   - TCPView
   - 管理员 PowerShell
   - 目标程序

不要先开通达信再开目标程序。

## Step 1. 找到目标进程

启动目标程序后，先确认：

- 进程名
- PID
- EXE 完整路径

PowerShell：

```powershell
Get-Process | Sort-Object ProcessName | Select-Object Id, ProcessName, Path
```

如果进程名你已经知道，例如 `NetzipDemo`，可以直接：

```powershell
Get-Process -Name NetzipDemo | Select-Object Id, ProcessName, Path
```

记下这个 PID，下面都用它。

## Step 2. 启动按 PID 的 TCP 连接记录

仓库里已经放了脚本：

[watch_process_tcp.ps1](/home/codes/stock/quoteNetzipRs/tools/windows/watch_process_tcp.ps1)

在管理员 PowerShell 里运行：

```powershell
cd C:\path\to\netzipapi-rust-demo
powershell -ExecutionPolicy Bypass -File .\tools\windows\watch_process_tcp.ps1 -Pid <PID> -LogPath .\captured_windows_traffic\process_tcp_log.csv
```

例如：

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\windows\watch_process_tcp.ps1 -Pid 12345 -LogPath .\captured_windows_traffic\process_tcp_log.csv
```

它会持续记录这个 PID 首次出现过的 TCP 连接：

- `timestamp`
- `state`
- `local_address`
- `local_port`
- `remote_address`
- `remote_port`
- `pid`

## Step 3. 同时打开 TCPView 做人工确认

打开 TCPView：

1. 找到目标 PID 对应的进程
2. 只盯它的连接
3. 重点看：
   - `Remote Address`
   - `Remote Port`
   - 连接出现的先后顺序

这一步的作用是：

- 脚本负责留痕
- TCPView 负责肉眼确认“是不是就是这个进程发起的连接”

建议你做一张截图，保留：

- 进程名
- PID
- 远端地址
- 远端端口

## Step 4. 开始 Wireshark 抓包

这一轮先不要用太窄的过滤器。

推荐直接抓整张网卡，然后事后按目标进程实际连出的地址过滤。

如果你一定要先加过滤，可以先只限定 TCP：

```text
tcp
```

不要一上来就只抓 `7709` 或 `6100`。

## Step 5. 只触发一个动作

每轮只做一个动作，避免混流。

推荐顺序：

1. 启动目标程序
2. 调一次 `Start`
3. 只发送一条最小 `Ask(...)`
4. 等几秒
5. `Stop`
6. 退出程序

不要一次把：

- 登录
- 初始化
- 查询
- 自动升级

全混在一轮里。

## Step 6. 这一轮建议的最小触发

优先挑“最短、最单一”的请求。

如果你能控制只发一条，就用：

```text
股票数据?请求=查询服务器列表&代码=SH000001&模块=认证&等待=3000&编号=10
```

如果这个接口不走或不可控，再退一步用：

```text
股票数据?请求=登录&模块=认证&账号=168&密码=168&自动升级=稳定版&版本=20221120&等待=3000&编号=0
```

关键不是内容，而是“一轮只发一条”。

## Step 7. 结束后先看 process_tcp_log.csv

这一轮最先交给我的不是 raw payload，而是这个文件：

```text
captured_windows_traffic\process_tcp_log.csv
```

我先要知道：

- 目标进程到底连了哪些远端
- 哪些连接是先出现的
- 有没有 `7709`
- 有没有 `6100`
- 有没有 `7719`
- 有没有 `14017`

如果里面没有 `7709`，那之前那批 `7709` 基本就可以从“主链证据”里降级掉。

## Step 8. 再回到 Wireshark 精确导出流

拿到 `process_tcp_log.csv` 以后，再在 Wireshark 里只看那几个真实出现过的连接。

例如日志里真出现了：

- `121.41.70.217:6100`
- `183.236.97.134:14017`

那就用显示过滤器：

```text
tcp.port == 6100 || tcp.port == 14017
```

如果你已经知道具体 IP，也可以更精确：

```text
(ip.addr == 121.41.70.217 && tcp.port == 6100) || (ip.addr == 183.236.97.134 && tcp.port == 14017)
```

然后对每条 TCP 会话：

1. `Follow -> TCP Stream`
2. 导出 `Raw`
3. 分别保存：
   - `client_to_server.raw`
   - `server_to_client.raw`

如果一轮里有多条连接，按连接拆开保存，文件名里带上 `ip_port`。

例如：

- `121.41.70.217_6100_client.raw`
- `121.41.70.217_6100_server.raw`
- `183.236.97.134_14017_client.raw`
- `183.236.97.134_14017_server.raw`

## Step 9. 这一轮最想要你回传什么

至少给我这 4 类东西：

1. `process_tcp_log.csv`
2. TCPView 截图
3. `capture_xxx.pcapng`
4. 你导出的 `raw` 流

如果只能先给一种，优先顺序是：

1. `process_tcp_log.csv`
2. `pcapng`
3. `raw`
4. 截图

## Step 10. 如果想再往前推进一层

在“确认真实端口”之后，再进入动态 dump。

对应文档：

[TDX118_DUMP_GUIDE.md](/home/codes/stock/quoteNetzipRs/TDX118_DUMP_GUIDE.md)

但这个 dump 只在“确认这条链确实属于目标进程”以后再做。

## 我建议你这次的最小交付

只做下面这一套就够：

1. 重启 Windows
2. 关闭所有证券软件，只开目标程序
3. 跑 `watch_process_tcp.ps1`
4. 开 Wireshark
5. 只触发一条最小 `Ask(...)`
6. 保存：
   - `process_tcp_log.csv`
   - `capture_clean_run.pcapng`
7. 把结果给我

只要这一步拿到，我就能先判断：

- 之前的 `7709` 是否应整体降级
- 真正该继续追的主端口到底是哪条
