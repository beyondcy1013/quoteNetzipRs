# NetzipRs 根目录迁移

## 共识

- 目标：将 `netzipapi-rust-demo` 独立仓库扁平化到 `/home/codes/stock/quoteNetzipRs`，使该目录成为唯一工程根。
- 范围：保留现有 `src/`、`scripts/`、`docs/`、`deploy/`、`crates/`、`netzip_api_bin/`、`webgui/` 等内部目录；迁移 `.git`、Cargo 清单和项目文档。
- 外层现有 `NetzipAPI.rar` 保留；外层含凭据的 `AGENTS.MD` 不作为项目入口，改用项目版 `AGENTS.MD` 并移除明文凭据。
- 需要同步修订：旧绝对路径、父级 `stock/QUOTE.MD` 链接、webClx/Cargo/部署文档中的项目身份。

## 验收

- `/home/codes/stock/quoteNetzipRs/.git` 存在，旧 `netzipapi-rust-demo/` 不再作为源码入口。
- 构建清单、部署脚本和文档不再依赖原 `netzipapi-rust-demo/` 嵌套源码入口。
- `cargo metadata --no-deps` 成功，迁移后的 Git 差异边界可审计。

## 风险与未完成项

- webClx 已用项目标识 `quoteNetzipRs`、工作目录 `/home/codes/stock/quoteNetzipRs` 完成 release 编译；请求 `162028-18c8f2f31ead11cc`。
- 已部署 systemd 服务仍沿用既有运行二进制和配置；本次目录迁移不触发部署或重启。
- Windows 抓包脚本中的 `Z:\netzipapi-rust-demo` 是外部映射盘路径，不自动改写。
