---
name: reverse-skill
description: >-
  Load the zhaoxuya520/reverse-skill router (git submodule) for authorized
  reverse engineering, protocol recovery, PE/DLL analysis, and PCAP work. Use
  when the user mentions reverse-skill, IDA/Ghidra, 协议逆向, 抓包还原, or asks Grok
  to follow that GitHub pack. Not for unauthorized scanning, exploits, or
  publishing attack procedures.
---

# reverse-skill（Grok 入口）

Pack 以 git submodule 进仓，Grok Bot `git clone --recurse-submodules` 后即可读到。

- 路径：`vendor/reverse-skill`
- 上游：https://github.com/zhaoxuya520/reverse-skill
- 当前钉住：`main` `7e2097fd90d25c2f976f6eba26d6c00aa88051df`

克隆后若 `vendor/reverse-skill` 为空，执行：

```bash
git submodule update --init --recursive vendor/reverse-skill
```

## 本仓库用法

quoteNetzipRs 的官方全推复刻仍走项目 skill，不要被 reverse-skill 的渗透/利用模块带走：

1. 产品路由：`.agents/skills/quoteNetzipRs-ops/SKILL.md`
2. 5188/Wine 全推：`.agents/skills/quote-netzip-rs-fullpull-replication/SKILL.md`
3. 需要通用协议/PE 方法时，再读 pack 内：
   - `vendor/reverse-skill/skills/SKILL.md`
   - `vendor/reverse-skill/skills/MASTER-ROUTING.md`
   - `vendor/reverse-skill/skills/protocol-reverse/SKILL.md`
   - `vendor/reverse-skill/skills/ida-reverse/SKILL.md` 或 `ghidra-reverse/`（分析已有 `网际风.exe` / `Stock.dll`）

## 硬门

- 只分析本机已有、已授权的厂商客户端与自有抓包。
- 禁止对未授权目标扫描、利用、EDR 绕过、攻击链编排。
- 禁止写出 exploit / PoC / 攻击步骤。
- 凭据不进仓库、日志、状态接口或对话。
- 不要再开第二份 `网际风.exe`；对照用现有 Wine 宿主。

## 发现后立刻做

读 pack 的 `skills/SKILL.md` 与 `MASTER-ROUTING.md`，把任务分到 `protocol-reverse` 或二进制模块，然后回到 quoteNetzipRs 的全推账本写证据。不要把 pack 的 `pwn-chain` / `pentest-tools` / `attack-chain` 当成本产品默认路径。
