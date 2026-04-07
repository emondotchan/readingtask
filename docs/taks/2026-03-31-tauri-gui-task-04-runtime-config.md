# 任务 04：运行时配置目录与 Tauri Command 桥接

## 目标

解决 GUI 打包后的配置文件可用性问题，并提供前端可调用的结构化 Tauri commands。

## 背景

当前应用直接读取仓库 `config/`。这在桌面打包后会失效，因此必须引入资源复制与运行时配置目录。与此同时，前端需要通过 Tauri command 与 Rust 共享层交互。

## 任务范围

- 将默认 `config/*.toml` 打入 Tauri bundle resources
- 在 Tauri 启动阶段检查并初始化 app data 配置目录
- 新增 `get_runtime_status`
- 新增 `run_reading_task`
- 定义 Tauri 错误到前端错误消息的映射
- 实现执行进度事件

## 不在范围内

- GUI 页面布局
- TOML 可视化编辑器
- 历史记录存储

## 关键实现要求

- GUI 运行时只读取 app data 中的配置，不直接使用 bundle 内原始文件
- 首次启动若配置不存在，则从 bundle resources 复制默认文件
- `get_runtime_status` 必须返回配置目录和配置就绪状态
- `run_reading_task` 必须返回最终汇总，并在执行过程中发出进度事件

## 事件协议

建议事件名：

- `reading-task://progress`
- `reading-task://completed`
- `reading-task://failed`

建议进度负载：

```json
{
  "processedCount": 3,
  "requestedCount": 5,
  "latestItem": {}
}
```

## 交付物

- bundle resources 配置
- app data 初始化逻辑
- Tauri commands
- 进度事件协议

## 验收标准

- 初次启动可自动生成运行时配置目录
- 缺失配置时前端可以明确感知
- 前端可成功调用 `get_runtime_status`
- 前端可成功调用 `run_reading_task`
- 执行过程中可以收到进度事件

## 依赖关系

- 依赖任务 01 和任务 03
- 为任务 05 提供数据与事件桥接
