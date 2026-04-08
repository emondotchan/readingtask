# reading_task

`reading_task` 是一个基于 Rust 的阅读任务工具，提供两种使用方式：

- CLI：按课程、经理、FC 和门店范围发起阅读任务
- Desktop：基于 Tauri + React 的桌面管理界面

项目同时包含任务执行逻辑、SQLite 数据管理、桌面端运行时初始化，以及用于打包分发的内置 SQLite 模板。

## 项目结构

```text
.
├─ src/                  # Rust CLI 与核心业务逻辑
├─ config/               # 运行时数据文件
├─ desktop/              # Tauri + Vite + React 桌面端
│  └─ src-tauri/         # Tauri Rust 壳层与打包配置
├─ scripts/              # 辅助脚本
└─ Cargo.toml            # Rust workspace 根配置
```

核心文件：

- `src/main.rs`：CLI 入口
- `src/cli.rs`：命令行参数定义
- `src/lib.rs`：对外导出的核心能力
- `desktop/package.json`：桌面端前端脚本
- `desktop/src-tauri/tauri.conf.json`：Tauri 配置
- `desktop/src-tauri/resources/bundled.reading.sqlite`：内置 SQLite 模板

## 环境要求

### Rust / CLI

- Rust stable
- Cargo

### Desktop

- Node.js 18+
- npm
- Rust stable
- Tauri 2 对应平台依赖

Windows 打包通常需要：

- WebView2 Runtime
- NSIS
- MSVC Build Tools

macOS 打包通常需要：

- Xcode Command Line Tools

## 配置文件

以下文件属于运行时数据，不是源码模块：

- `config/open_ids.toml`
- `config/shop.toml`
- `config/province.toml`

请谨慎处理其中的真实业务数据，不要在公开描述里泄露真实标识符。

## CLI 使用

查看帮助：

```bash
cargo run -- --help
```

执行一次阅读任务：

```bash
cargo run -- -c <course_id> -m <manager_id> -f <fc> -n 5
```

指定门店编码列表：

```bash
cargo run -- -c <course_id> -m <manager_id> -f <fc> -n 2 -s SHOP001,SHOP002
```

当前支持的主要参数：

- `-c, --s-course-id`：课程 ID
- `-m, --s-manager-id`：经理 ID
- `-f, --fc`：FC 标识
- `-n, --count`：执行数量，默认 `1`
- `-s, --shopcodes`：门店编码列表，逗号分隔

## Desktop 开发

安装前端依赖：

```bash
cd desktop
npm install
```

启动桌面端开发模式：

```bash
npm run tauri:dev
```

仅启动前端开发服务器：

```bash
npm run dev
```

构建前端资源：

```bash
npm run build
```

## Desktop 打包

Windows:

```bash
cd desktop
npm run tauri:build:win
```

macOS:

```bash
cd desktop
npm run tauri:build:mac
```

通用构建：

```bash
cd desktop
npm run tauri:build
```

说明：

- Tauri 会把 `desktop/src-tauri/resources/bundled.reading.sqlite` 作为内置资源打包
- 如果本机存在 `HOME/.reading.sqlite` 或 `USERPROFILE/.reading.sqlite`，构建阶段会优先将其复制为打包模板
- 如果家目录下没有模板库，则会直接使用仓库内已有的 `bundled.reading.sqlite`

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
cd desktop
npm run build
```

## 数据库说明

桌面端首次启动时，会把内置模板数据库复制到应用数据目录作为默认 SQLite 存储文件。运行中使用的实际数据库不建议直接拿打包资源文件替代。

如果使用 Navicat 等工具打开 `desktop/src-tauri/resources/bundled.reading.sqlite`，请在打包前关闭占用，否则安装包构建阶段可能因为文件被锁定而失败。

## 说明

当前仓库已经将 CLI 与 `desktop/src-tauri` 纳入同一个 Cargo workspace，以复用构建产物并减少重复编译。
