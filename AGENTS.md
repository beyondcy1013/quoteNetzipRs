# AGENTS.md

Linux 上的 Grok / Grok Bot 按大小写精确查找 `AGENTS.md`。本文件是发现入口。

完整项目规则以同目录 `AGENTS.MD` 为准，并同时加载：

- `.grok/rules/quote-netzip-rs.md` — Grok Bot 项目入口与能力边界
- `.agents/skills/quoteNetzipRs-ops/SKILL.md` — 工作路由
- `.agents/skills/quote-netzip-rs-fullpull-replication/SKILL.md` — 官方全推复刻

禁止输出真实账号或密码。凭据只存在于受保护运行时配置。
