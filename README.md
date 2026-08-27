# reading_task

`reading_task` 是一个基于 Rust 的阅读任务工具，提供 Web 管理界面和后台 API。

项目同时包含任务执行逻辑、SQLite 数据管理、后端运行时初始化，以及用于打包分发的内置 SQLite 模板。

## 项目结构

```text
.
├─ src/                  # Rust Axum API 与核心业务逻辑
├─ config/               # 运行时数据文件
├─ web/                  # Vite + React + Tailwind + shadcn 前端
├─ scripts/              # 辅助脚本
└─ Cargo.toml            # Rust workspace 根配置
```

核心文件：

- `src/main.rs`：Web API 入口
- `src/lib.rs`：对外导出的核心能力
- `web/package.json`：前端脚本
- `web/src/styles.css`：Tailwind/shadcn 主题
- `web/src/api/commands.ts`：前端 API 适配层

## 环境要求

### Web UI

- Node.js 18+
- npm
- Rust stable

## 配置文件

以下文件属于运行时数据，不是源码模块：

- `config/open_ids.toml`
- `config/shop.toml`
- `config/province.toml`

请谨慎处理其中的真实业务数据，不要在公开描述里泄露真实标识符。

## Web 开发

安装前端依赖：

```bash
cd web
npm install
```

一键启动前后端并打开浏览器：

```bash
./scripts/dev-web.sh
```

如果只想单独启动后端 API：

```bash
cargo run
```

启动前端开发服务器：

```bash
cd web
npm run dev
```

构建前端资源：

```bash
cd web
npm run build
```

## Web 构建

```bash
cargo build --release
cd web && npm run build
```

Linux 构建会通过 `.cargo/config.toml` 使用 `clang` 加 `mold` 链接器。请先安装 `clang` 和 `mold`，macOS 本机构建不受该配置影响。

说明：

- 后端启动时会在用户主目录下管理 `~/.reading.sqlite`
- 如果未找到已配置的 SQLite 路径，后端会先把内置模板复制到默认位置

## 常用检查命令

Rust:

```bash
cargo check --workspace
cargo test
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

前端：

```bash
cd web
npm run build
```

## 数据库说明

后端首次启动时会在用户主目录下创建默认 SQLite 文件，并写入运行时设置。运行中使用的实际数据库不建议直接拿内置模板替换。

## 说明

当前仓库已经将 Axum API 与前端纳入同一个代码库，以复用核心业务逻辑并减少重复实现。
