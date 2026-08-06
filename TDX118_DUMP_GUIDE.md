# TDX 0x118 Dump Guide

目标：在 Windows 上拿到一对可直接比对的样本。

- 明文：`0x10066b60` 组出的 `0x118` body
- 密文：首个 `0x7b00` 包出网前 `payload[2..]` 的 `0x118` body

拿到后直接喂给：

```bash
cargo run --example blob_compare -- plain_0118.bin cipher_0118.bin --len 280 --block-size 8
```

或者服务接口：

```bash
curl -fsS -X POST http://127.0.0.1:16893/api/debug/blob-compare \
  -H 'Content-Type: application/json' \
  -d '{"left_path":"plain_0118.bin","right_path":"cipher_0118.bin","compare_len":280,"block_size":8}'
```

## 最小结论

当前最稳的判断是：

- 首帧是 `10B client10 头 + 282B payload`
- `payload[0..2] = 0x000b`
- `payload[2..282] = 0x118 = 280` 字节主体
- 这 `0x118` 主体更像 `0x10066b60` 产出的明文经过 `Tdx_Encrypt` 一类固定块处理后的等长结果

## 关键地址

来自现有静态逆向笔记：

- `0x1005ce67`
- `0x10066b60`
- `0x1007b7c0`
- `0x1002bf90`
- `0x10064c40`
- `0x10115218` -> `中信证券`
- `0x10115224` -> `NetCardMac`
- `0x10103258` -> `Tdx_Encrypt`

模块文件：

- [Stock.dat](/home/codes/quoteNetzipRs/netzip_api_bin/NetzipAPI/StockC%23/Stock.dat)

## Frida 最短方案

优先目标不是完整逆算法，而是先 dump 样本。

1. 挂到目标进程。
2. 在 `Stock.dat + 0x10066b60` 返回后，找承载 `0x118` 明文 body 的缓冲区。
3. 在 `Stock.dat + 0x1007b7c0` 或 `Stock.dat + 0x1002bf90` 进入发送前，找 `0x7b00` 的 `payload[2..]`。
4. 各写一份二进制文件。

建议输出文件名：

- `plain_0118.bin`
- `cipher_0118.bin`
- 可选：`frame_7b00_full.bin`

最小 Frida 观察点：

- `0x10066b60`
  - 目标：确认哪块内存是刚组好的 `0x118` 明文
- `0x1007b7c0`
  - 目标：确认进入加密/固定块处理前后的输入输出
- `0x1002bf90`
  - 目标：确认出网前看到的 `0x118` 密文和 `0x7b00 payload[2..]` 一致

Frida 实操建议：

- 先只打印参数指针和前 `0x20` 字节，不要一上来大范围 dump
- 一旦看到长度稳定为 `0x118` 的缓冲区，再单次 `Memory.readByteArray(ptr, 0x118)`
- 文件写出时保留触发顺序编号，例如：
  - `plain_0118_step1.bin`
  - `cipher_0118_step2.bin`

## x64dbg 最短方案

如果不用 Frida，x64dbg 也够。

1. 加载目标模块，确认 `Stock.dat` 基址。
2. 在下列 RVA 下断：
   - `0x10066b60`
   - `0x1007b7c0`
   - `0x1002bf90`
3. 跑到首个 `0x7b00` 会话。
4. 在寄存器和栈里找指向 `0x118` 缓冲区的指针。
5. 用 dump 窗口各保存两份：
   - 明文 `0x118`
   - 密文 `0x118`

保存文件名同样建议：

- `plain_0118.bin`
- `cipher_0118.bin`

## WinDbg 备选

如果已经在 WinDbg 里：

- 先确认模块基址
- 用 `bp`/`bu` 下到上面 3 个点
- 命中后先看寄存器是否直接给出 `buffer + len`
- 用 `.writemem` 导出 `0x118` 字节

## 成功标准

本轮不要求马上解算法，只要满足下面任意一条就算推进成功：

- 拿到一对 `plain_0118.bin / cipher_0118.bin`
- 证明 `cipher_0118.bin == 线上首帧 payload[2..]`
- 证明 `plain_0118.bin` 中确实还能看到 `中信证券 / NetCardMac / 账号 / 密码 / 服务器 IP` 对应明文痕迹

## 喂回本仓库

如果 dump 文件放在本机：

```bash
cargo run --example blob_compare -- plain_0118.bin cipher_0118.bin --len 280 --block-size 8
```

或者：

```bash
curl -fsS -X POST http://127.0.0.1:16893/api/debug/blob-compare \
  -H 'Content-Type: application/json' \
  -d '{"left_path":"plain_0118.bin","right_path":"cipher_0118.bin","compare_len":280,"block_size":8}'
```

然后再把结果记回 [PROTOCOL_NOTES.md](/home/codes/quoteNetzipRs/PROTOCOL_NOTES.md)。
