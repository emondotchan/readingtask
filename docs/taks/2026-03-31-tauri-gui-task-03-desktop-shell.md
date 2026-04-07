# 任务 03：Tauri + React 桌面壳工程搭建

## 目标

创建独立的 `desktop/` 子工程，包含 React/Vite 前端和 `src-tauri` 桌面壳，并能成功依赖根目录 Rust library。

## 背景

当前仓库没有前端和桌面工程。需要先把 Tauri 与 React 的基础工程搭起来，后续命令桥接和 GUI 页面才有落点。

## 任务范围

- 新建 `desktop/`
- 初始化 React + Vite
- 初始化 `desktop/src-tauri`
- 配置开发脚本和构建脚本
- 让 `desktop/src-tauri` 通过 path dependency 引用根目录 Rust library

## 不在范围内

- 完整 GUI 表单与结果页
- 配置文件复制逻辑
- 业务 command 细节

## 关键实现要求

- `desktop` 内的 Node 依赖与 Rust 壳配置要清晰分层
- `desktop/src-tauri` 不能依赖根 crate 的 CLI bin，只能依赖 library
- 开发启动方式统一为在 `desktop/` 目录内执行脚本
- 构建方式统一为在 `desktop/` 目录内执行 Tauri build

## 最低交付能力

- 能启动一个空白 React 窗口
- 能完成 Tauri dev 构建
- 能编译通过 path dependency

## 交付物

- `desktop/package.json`
- `desktop/src-tauri/Cargo.toml`
- `desktop/src-tauri/tauri.conf.json`
- 最小可运行桌面应用骨架

## 验收标准

- `desktop` 开发模式可以启动
- 空白窗口可见
- 根 library 依赖解析成功

## 依赖关系

- 依赖任务 01 完成共享 library
- 可与任务 02 并行
- 为任务 04、05 提供运行容器
