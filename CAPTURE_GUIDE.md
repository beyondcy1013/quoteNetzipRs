# Windows Capture Guide

目标：抓到 `Stock64.dll` 在调用 `Ask("股票数据?...")` 时，真正发往 `121.41.70.217:6100` 的原始字节流。

## 最短路线

优先用 Wireshark 或 RawCap。不要优先依赖 `pktmon` 默认导出，因为它当前样本里只保留了每个 TCP 包前 `128` 字节，落到应用层只剩 `74` 字节 payload。

## 1. 启动抓包

在 Windows 上打开 Wireshark，选择当前联网网卡，设置显示过滤器：

```text
tcp.port == 6100 && ip.addr == 121.41.70.217
```

如果连的是备用服务器，也可以改成：

```text
tcp.port == 6100 && (ip.addr == 121.41.70.217 || ip.addr == 39.108.103.69)
```

## 2. 触发最小请求

尽量只做一个动作，避免噪音：

1. 启动你的 Windows 版 Rust demo 或官方 C++/C# 示例
2. 只发送一条请求，例如：

```text
股票数据?请求=查询服务器列表&代码=SH000001&模块=认证&等待=3000&编号=10
```

或者：

```text
股票数据?请求=登录&模块=认证&账号=168&密码=168&自动升级=稳定版&版本=20221120&等待=3000&编号=0
```

如果一次就做登录 + 初始化 + 查询，流量会混在一起，不利于定位首包。

## 3. 导出原始流

在 Wireshark 中：

1. 找到对应 TCP 会话
2. 右键 `Follow` -> `TCP Stream`
3. 右下角把显示格式切成：
   - `Raw`
   - 或 `Hex Dump`
4. 保存为文件

建议保存两份：

- `client_to_server.raw`
- `server_to_client.raw`

如果 Wireshark 只能导出完整流，也可以接受。

抓完以后先自检一次，不要直接开始逆向：

```bash
tcpdump -r capture.pcapng -w capture.pcap 'tcp port 6100'
cargo run --example pcap_summary -- capture.pcap
```

如果输出里出现大量：

- `truncated-unique-packets`
- `warning: capture file does not contain full on-wire packet bytes`

说明这份抓包仍然不够，继续分析只会看到每包前缀。

## 4. 拿到 Linux 分析

把导出的文件放到当前仓库后：

原始二进制：

```bash
cargo run --example stream_analyze -- client_to_server.raw --bin
```

如果是十六进制文本：

```bash
cargo run --example stream_analyze -- client_to_server.txt --hex
```

## 5. 优先关注什么

优先关注第一条客户端发包：

- 是否以固定长度头开头
- 是否带 `u32/u16` 长度
- 是否出现 gzip 头 `1f 8b`
- 是否能直接看出 UTF-16LE 请求串
- 是否先有一条握手包，再发真正请求

## 6. 如果没有 Wireshark

备选方案：

1. 用 RawCap 抓 `pcap`
2. 再把 `pcap` 放回 Linux，用 `tshark` 导出特定 TCP 流

例如：

```bash
tshark -r capture.pcapng -q -z follow,tcp,raw,0
```

把导出的十六进制文本再喂给：

```bash
cargo run --example stream_analyze -- follow_tcp_raw.txt --hex
```

## 当前判断

根据已经做过的 Linux 探测：

- `6100` 是对外开放 TCP 端口
- 不是标准 TLS
- 不是直接 HTTP
- 不是把 `股票数据?...` 明文直接发过去

所以只要拿到 Windows 侧第一条真实出站流，基本就能把协议继续推进。

补充：

- 如果抓包后发现每个大包只有 `74` 字节应用层数据可见，说明不是协议只有 `74` 字节，而是抓包被截断了。
- 目前我们已经确认 `51 7f dc 7e 05 53` 就是 UTF-16LE 的 `网络包`，所以重新抓包时仍然优先看这个前缀之后的完整正文。
