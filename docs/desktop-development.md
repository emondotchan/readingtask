# 桌面应用开发与打包指南

## 前置条件

在开始之前，请确保已安装以下工具：

- **Rust** (≥ 1.85) — 通过 [rustup](https://rustup.rs/) 安装
- **Node.js** (≥ 18) 与 npm
- **Tauri 2 系统依赖** — 各平台详见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)

### macOS 打包环境

- **Xcode Command Line Tools** — 运行 `xcode-select --install`
- 建议使用 Apple Silicon 或 Intel 原生 macOS 主机执行打包

### Windows 打包环境

- **Visual Studio Build Tools 2022**，至少包含 C++ 构建工具和 Windows SDK
- **Microsoft Edge WebView2 Runtime**
- 建议使用原生 Windows 主机执行打包

> 当前仓库按“各平台各自构建”维护。不建议在 macOS 上直接产出 Windows 安装包，也不建议在 Windows 上直接产出 macOS 安装包。

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
npm run tauri:build:mac
```

构建产物位于：

```
desktop/src-tauri/target/release/bundle/
├── macos/          # .app 应用包
└── dmg/            # .dmg 安装镜像
```

### 构建 Windows 应用

```bash
cd desktop
npm run tauri:build:win
```

构建产物位于：

```text
desktop/src-tauri/target/release/bundle/
└── nsis/           # Windows 安装器 .exe
```

### 通用打包命令

```bash
cd desktop
npm run tauri:build
```

该命令会按 `tauri.conf.json` 中的 `bundle.targets` 为当前宿主平台生成对应安装包：

- macOS 主机：`.app` 与 `.dmg`
- Windows 主机：`.exe`（NSIS 安装器）

> 当前阶段以内部测试分发为目标，不包含 macOS 公证、Developer ID 签名或 Windows 代码签名。
> 未签名安装包在首次运行时出现系统安全提示属于预期行为。

## 配置说明

应用使用三个 TOML 配置文件：`open_ids.toml`、`shop.toml`、`province.toml`。

### 开发模式

开发模式下，CLI 工具直接读取项目根目录 `config/` 下的配置文件。

### 打包后的应用

打包后的应用在首次启动时，会将 bundle 资源中的配置文件复制到用户数据目录。

macOS 默认目录：

```text
~/Library/Application Support/cn.eau-thermale-avene.reading-task/config/
```

Windows 默认目录：

```text
%APPDATA%\cn.eau-thermale-avene.reading-task\config\
```

之后应用始终从该目录读取配置。用户可以直接编辑该目录下的 TOML 文件来更新配置，**无需重新打包应用**。

> **注意**: 只有当目标文件不存在时才会从 bundle 复制，已有的配置不会被覆盖。

## 验收建议

每次打包后至少验证以下项目：

- 安装包可以正常生成
- 应用可以正常启动
- 首次启动后配置文件已复制到用户数据目录
- FC、月度计划、执行记录等核心页面可以正常打开

## CLI 命令行工具

桌面应用之外，CLI 工具可独立使用：

```bash
# 查看帮助
cargo run -- --help

# 运行示例：为指定 FC 随机选取 5 家门店执行任务
cargo run -- -c <course_id> -m <manager_id> -f <fc> -n 5
```

CLI 与桌面应用共享相同的核心逻辑（`reading_task` 库），区别仅在于界面交互方式。
