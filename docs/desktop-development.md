# 桌面应用开发与打包指南

## 前置条件

在开始之前，请确保已安装以下工具：

- **Rust** (≥ 1.85) — 通过 [rustup](https://rustup.rs/) 安装
- **Node.js** (≥ 18) 与 npm
- **Xcode Command Line Tools** — 运行 `xcode-select --install`
- **Tauri 2 系统依赖** — macOS 上通常只需 Xcode CLT；详见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)

## 开发

### 安装前端依赖

```bash
cd desktop
npm install
```

### 启动开发模式

```bash
cd desktop
npm run tauri:dev
```

此命令会同时启动 Vite 前端开发服务器（端口 1420）和 Tauri 后端，支持前端热更新。

## 打包

### 构建 macOS 应用

```bash
cd desktop
npm run tauri:build
```

构建产物位于：

```
desktop/src-tauri/target/release/bundle/
├── macos/          # .app 应用包
└── dmg/            # .dmg 安装镜像
```

> **提示**: 如需发布到 App Store 或分发给其他用户，需要对 `.app` 进行代码签名。
> 开发阶段可直接运行未签名的 `.app`。

## 配置说明

应用使用三个 TOML 配置文件：`open_ids.toml`、`shop.toml`、`province.toml`。

### 开发模式

开发模式下，CLI 工具直接读取项目根目录 `config/` 下的配置文件。

### 打包后的应用

打包后的应用在首次启动时，会将 bundle 资源中的配置文件复制到用户数据目录：

```
~/Library/Application Support/cn.eau-thermale-avene.reading-task/config/
```

之后应用始终从该目录读取配置。用户可以直接编辑该目录下的 TOML 文件来更新配置，**无需重新打包应用**。

> **注意**: 只有当目标文件不存在时才会从 bundle 复制，已有的配置不会被覆盖。

## CLI 命令行工具

桌面应用之外，CLI 工具可独立使用：

```bash
# 查看帮助
cargo run -- --help

# 运行示例：为指定 FC 随机选取 5 家门店执行任务
cargo run -- -c <course_id> -m <manager_id> -f <fc> -n 5
```

CLI 与桌面应用共享相同的核心逻辑（`reading_task` 库），区别仅在于界面交互方式。
