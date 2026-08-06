# 认证链变换笔记

## 总结

- `penc` 确实在 7100/6100 登录线上出现。
- 标准 ZSTD 用于 7100 测速、7100 响应和 6100 正式登录请求。
- `field44=12` 不是无法解释的伪 ZSTD：现场 `Stock.字典` 作为 raw-content dictionary 可解开本次全部 66 个样本。
- `Tdx_Encrypt` 没有出现在本次网络字节中；现有证据只把它定位到 Stock.dll 的 7709 bootstrap 调用链。
- `hypenc` 在本次原始 pcapng 中未出现。

## 1. 标准 ZSTD

| 帧 | 输入长度 | ZSTD offset | 输出长度 | 输出 SHA-256 | 角色 |
|---:|---:|---:|---:|---|---|
| 478 | 427 | 168 | 446 | `f225c2803fa0ac5e33dc98565c0acac9d7e97a7c8cfeaed11f650af5688eba68` | 7100 `认证|测速` |
| 482 | 345 | 168 | 328 | `ada49dcca65a01be391288a8fbd0f297a79616623aada3f49a81577ab310cfbf` | 7100 测速响应 |
| 493 | 633 | 168 | 854 | `6dba83b10584c80a84a7bf062748d1e413050815ef35028f103daf8d41f869a8` | 6100 `认证|登录` |

这些帧的 `field44=8`，ZSTD magic 均为 `28 b5 2f fd`。账号和密码在解压内层中是 UTF-16LE；未发现额外 XOR 或 `Tdx_Encrypt` 层。

## 2. Stock.字典路径

字典文件：`D:\Soft\_Stock\飞狐2020\Stock.字典`，长度 1000 B，SHA-256 `8f44f49cbf10c8d203d9cabbda256da37c1f7d43e7f99b60c08d077c47b2bc68`。

验证方式：对 `field44=12` 帧从 ZSTD magic 开始解压；无字典解压失败，`DICT_TYPE_RAWCONTENT` 和 auto 模式成功，full-dictionary 模式失败。全部 66 个本次样本都能用相同字典解开，0 个失败。

代表样本：

| direction | 外层长度 | 字典输入长度 | 输出长度 | 帧 / raw offset | 输出 SHA-256 |
|---|---:|---:|---:|---:|---|
| inbound | 439 | 267 | 620 | 494 / 137444 | `ec650efed623657140ea3a79d02cdfdd6e0a01d490592e4562f68b76b1a5ec37` |
| outbound | 267 | 95 | 130 | 499 / 138376 | `4a161f1788e4eb602b4e30ce4754383ef28dc825b4a00d4c9ed5209399fadd3f` |
| outbound | 378 | 206 | 460 | 545 / 144960 | `3ca4d28f52b4cff72efac1c4a8b5fe99e518077cb9d5c7376b5c65b7c09ed81e` |
| inbound | 385 | 213 | 289 | 546 / 145424 | `a0ce464d0a369badb46c68657acc28d8d9dde81d2b17188fba4a7e344f6baaab` |
| outbound | 977 | 805 | 1689 | 1605 / 1178100 | `222c1a933802af961a2057f76a9fa95a8caa7ad4c11a76c68a71c592d6ebc990` |
| inbound | 326 | 154 | 308 | 10464 / 9454936 | `8bec669b85f8065145604c44cdb2d65a101a00a68122bac5b020bc63260ae41c` |

这修正了旧结论：阻塞点不再是“找不到字典”，而是 Rust 端需要对齐 raw dictionary 压缩参数、对象模板和动态字段，验证生成字节能被服务端接受。

## 3. penc

- `field44=8` 的 427/633/345 B 帧：`penc @ 162`，ZSTD magic `@168`。
- `field44=12` 的登录后帧：`penc @ 166`，ZSTD magic `@172`。
- 1892 B 下载对象：`penc @ 60` 和 `@416`，UTF-16 配置 BOM `@422`。

`penc` 当前可确定是对象边界 marker；它是否还有校验或算法选择语义，网络证据仍不足。

## 4. Tdx_Encrypt 调用链

本次原始 pcapng 对 ASCII/UTF-16 `Tdx_Encrypt` 均为 0 命中。已确认的二进制调用链来自 `PROTOCOL_NOTES.md`：

1. `0x10066b60` 生成 0x118 B 明文体。
2. `0x1005cea9` 压入 UTF-16 `Tdx_Encrypt`。
3. `0x1007b7c0 -> 0x1002bf90 -> 0x1002da10` 初始化 0x1414 B 算法上下文。
4. `0x10064c40` 发送最终网络缓冲区。

该链对应 7709 首个 292 B `0x7b00` bootstrap 帧。当前网际风 PID `40504` 只建立 6100/7100/5188，没有 7709，因此不能声称 `Tdx_Encrypt` 参与了本次 7100/6100 登录。

## 5. 调用链取证缺口

本轮没有进行 API hook，原因是成功网络包已经直接解释账号/密码和 ZSTD 字典边界，且继续注入调试器会扩大真实凭据暴露面。若要完成协议级等价实现，仍需在一次隔离会话中记录以下脱敏缓冲区：

1. `0x10066b60` 的 0x118 B 明文输出，密码区域固定掩码。
2. `0x1007b7c0` 输入和 `0x1002bf90` 输出的长度/hash。
3. `0x10064c40` 最终 7709 网络缓冲区与 pcap 帧的字节级 hash 对应。
4. 7709 三阶段 `0x7b00 -> 0x9400 -> 0x9900` 响应解码结果。

## 6. Rust 最小实现顺序

1. 实现 427 B `认证|测速`，并支持同时探测 6100/7100 候选。
2. 实现 633 B `认证|登录` 的动态字段和标准 ZSTD level 3。
3. 把 `Stock.字典` 作为受版本/hash 管理的 raw-content ZSTD dictionary，实现 `field44=12` 解压。
4. 用代表帧回归字典输出长度和 SHA-256，再实现对应压缩端并和现场请求 hash/结构对照。
5. 解析 1892 B 下载对象和 5188 server list，保存选中 endpoint 与响应状态。
6. 把 7709/0547 作为独立 bootstrap 实现；在获得 hook 证据前，不把 6100/7100 字段臆测为 7709 token。

结论：已有证据足以实现真实账号字段、7100 测速、6100 正式登录和字典响应解码；还不足以证明完整 7709 `Tdx_Encrypt` 会话等价性。
