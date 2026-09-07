# `netzip-fullpull` Crate 提取与解耦迁移方案

## 1. 目标与背景

参考已迁移到 `../crates/netzip-fullpull` 的模块化设计，将本项目中的官方全推（Official Full-Push 6100/7100 认证控制链 + 5188 实时传输与协议交互）集中到共享 crate。

保持严格边界：
- **`netzip-fullpull` 核心定位**：专属于官方真全推链路（6100/7100 认证协商、`Stock.字典` 解密与解压、5188 长连接会话 `Official5188Session`、8 字节应用帧解析、交织初始化 Stage `Login/Abk/Ack`、候选帧提取）。
- **`netzip-supplement` / 主服务**：7709 补数、0547 轮询、F10、除权、历史 K 线等逻辑归属于 supplement/业务层，不进入 fullpull crate。

---

## 2. 使用共享 `../crates/netzip-fullpull` 的结构设计

在 `../netzip_win` 中，`netzip-fullpull` 的结构非常精炼：
```text
netzip-fullpull/
├── Cargo.toml
├── assets/
│   └── Stock.字典           # 1000 字节 UTF-16LE 认证字典
└── src/
    ├── lib.rs              # 5188 传输/帧定义/会话管理 (Official5188Session, FrameReassembler 等)
    └── auth_7100.rs        # 6100/7100 完整认证流与交织握手控制 (Auth7100ControlSession 等)
```

### 依赖关系（轻量化）：
- `thiserror` (2.0)
- `zstd` (0.13)
- `crc32fast` (1)
- 零额外重量级依赖，不依赖 `tokio`、`axum` 或 7709 业务库，具备极致的编译速度与跨平台复用能力。

---

## 3. 本项目现有结构与冲突分析

### 现状：
1. 本项目目前根目录 `Cargo.toml` 引用了 `netzip-fullpull = { path = "../crates/netzip-fullpull" }`。
2. 上级目录 `../crates/netzip-fullpull` 历史残留了大量的 `tdx7709.rs`、`tdx_0547.rs`、`tdx_fin.rs` 等补数过渡代码。
3. 另一终端 `quoteNetzipRs_10_main` 正在对本项目进行编辑。
4. 如果直接修改根目录 `Cargo.toml` 指向本地 `crates/netzip-fullpull`，且 `../crates/netzip-supplement` 的 `Cargo.toml` 仍指向 `../netzip-fullpull`，会导致 Cargo lockfile 出现同名 Package 路径碰撞（`package collision in the lockfile`）。

---

## 4. 详细实施计划（待主终端编辑就绪后执行）

### 阶段一：建立本地 `crates/netzip-fullpull` 文件架构（已准备就绪）
- [x] 创建 `crates/netzip-fullpull/assets/Stock.字典`（SHA256 校验：`8f44f49cbf10c8d203d9cabbda256da37c1f7d43e7f99b60c08d077c47b2bc68`）。
- [x] 创建 `crates/netzip-fullpull/src/lib.rs` 与 `crates/netzip-fullpull/src/auth_7100.rs`。
- [x] 创建 `crates/netzip-fullpull/Cargo.toml`。

### 阶段二：解决多 Workspace 依赖路径一致性
在主终端编辑完成后，统一处理引用路径：
1. **更新 `crates/netzip-fullpull/Cargo.toml`**：确保 `edition = "2024"`，依赖版本一致。
2. **更新项目根 `Cargo.toml`**：
   ```toml
   [workspace]
   members = [
       ".",
       "crates/tuwenca-codec",
       "crates/stockdrv-compat",
       "crates/netzip-fullpull",
   ]

   [dependencies]
   netzip-fullpull = { path = "crates/netzip-fullpull" }
   ```
3. **针对 `netzip-supplement` 的协同调整**：
   若根项目继续引用 `netzip-supplement`，确保其依赖的 `netzip-fullpull` 也对齐为同版本/路径，或者将 `netzip-supplement` 中的 fullpull 耦合彻底解耦。

### 阶段三：主项目代码适配与验证
1. 将 `src/auth_7100_client.rs` 中的底层 7100/5188 协议流迁移调用 `netzip_fullpull::auth_7100` 与 `netzip_fullpull::*`。
2. 运行单元测试集：
   ```bash
   cargo test --manifest-path crates/netzip-fullpull/Cargo.toml
   ```
3. 运行全项目集成检查：
   ```bash
   cargo check --workspace
   cargo test
   ```

---

## 5. 变更前后的安全守则
- **绝不破坏当前终端 `quoteNetzipRs_10_main` 的工作流**：在用户明确通知该终端编辑保存前，保持根目录 `Cargo.toml` 与 `Cargo.lock` 原样。
- **凭证安全**：所有测试与代码中不得硬编码真实账号密码，使用运行时注入与 Mock 验证。
- **全推与补数边界清晰**：5188 仅保留真全推，7709 严格保持在补数模块。
