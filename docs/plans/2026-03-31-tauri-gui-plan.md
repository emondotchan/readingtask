# 2026-03-31 Tauri GUI 实施计划

## 1. 目标与结论

本计划的目标是为当前 `reading_task` Rust CLI 应用增加一个 `macOS` 优先的桌面 GUI，使日常使用不再依赖手动拼接命令参数，同时保留现有 CLI 作为兼容入口。

本次采用的明确方案如下：

- GUI 技术栈使用 `Tauri 2.x + React + Vite`
- 现有 CLI 保留，不迁移为 GUI 独占入口
- 核心业务逻辑继续保留在 Rust 中，CLI 与 GUI 复用同一套应用层
- 首版 GUI 范围限定为单页工作台，不做历史记录、任务模板、配置编辑器、自动重试
- 首版以 `macOS` 开发与打包可用为验收目标，Windows 兼容只做后续规划，不列入本轮交付

## 2. 当前状态

当前仓库已有：

- Rust CLI 入口，参数为 `s_course_id`、`s_manager_id`、`fc`、`count`、`shopcodes`
- 运行时配置文件位于 `config/`
- 已拆分出 `src/app.rs` 与 `src/cli.rs`
- 当前应用仍以“命令行文本输出”作为唯一交互方式

当前仓库尚未具备：

- Rust library target
- Tauri 工程
- 前端工程
- 结构化执行结果模型
- GUI 可消费的命令接口
- 桌面打包与资源路径方案

## 3. 目标架构

### 3.1 总体结构

推荐采用以下目录结构：

```text
.
├─ config/
├─ docs/
│  ├─ plans/
│  └─ taks/
├─ src/
│  ├─ lib.rs
│  ├─ main.rs
│  ├─ cli.rs
│  ├─ app.rs
│  └─ core/
│     ├─ mod.rs
│     ├─ model.rs
│     ├─ error.rs
│     ├─ loader.rs
│     ├─ executor.rs
│     └─ report.rs
└─ desktop/
   ├─ package.json
   ├─ index.html
   ├─ tsconfig.json
   ├─ vite.config.ts
   ├─ src/
   │  ├─ main.tsx
   │  ├─ App.tsx
   │  ├─ api/
   │  ├─ components/
   │  ├─ features/
   │  └─ types/
   └─ src-tauri/
      ├─ Cargo.toml
      ├─ build.rs
      ├─ tauri.conf.json
      ├─ capabilities/
      └─ src/
         ├─ lib.rs
         ├─ commands.rs
         ├─ bootstrap.rs
         └─ error.rs
```

### 3.2 结构选择理由

- 不在仓库根目录直接初始化前端，是为了避免与当前 Rust `src/` 冲突
- `desktop/` 作为独立 Tauri 子工程，便于隔离 Node/Vite/Tauri 配置
- `desktop/src-tauri` 通过 path dependency 依赖根目录 Rust library，避免复制业务逻辑
- 根目录 Rust crate 增加 `lib.rs` 后，CLI 和 Tauri 都可以复用同一应用层

## 4. 共享 Rust 应用层设计

### 4.1 共享能力边界

共享应用层负责以下职责：

- 请求输入校验
- 运行时配置路径解析
- `open_ids.toml`、`shop.toml`、`province.toml` 读取与解析
- OpenID 去重
- 门店筛选与随机抽样
- 请求顺序执行
- 单条执行结果与整体汇总建模
- 结构化错误返回

共享应用层不负责以下职责：

- `clap` 参数解析
- GUI 表单状态管理
- 前端展示文案和样式
- Tauri 生命周期和窗口配置

### 4.2 共享输入输出模型

建议统一为以下结构：

```rust
pub struct TaskRunRequest {
  pub s_course_id: String,
  pub s_manager_id: String,
  pub fc: String,
  pub count: usize,
  pub shopcodes: Vec<String>,
}

pub enum TaskItemOutcome {
  Success,
  RequestError,
  ResponseReadError,
}

pub struct TaskItemResult {
  pub index: usize,
  pub open_id: String,
  pub shop_code: String,
  pub province: String,
  pub city: String,
  pub http_status: Option<u16>,
  pub response_text: Option<String>,
  pub error_message: Option<String>,
  pub outcome: TaskItemOutcome,
}

pub struct TaskRunSummary {
  pub requested_count: usize,
  pub processed_count: usize,
  pub success_count: usize,
  pub failure_count: usize,
  pub started_at: String,
  pub finished_at: String,
  pub items: Vec<TaskItemResult>,
}
```

补充说明：

- `shopcodes` 在共享层使用 `Vec<String>`，由 CLI 和 GUI 各自完成原始输入拆分
- 当前执行逻辑保持顺序发送，不改为并发
- 首版不做取消执行，不做断点恢复

### 4.3 错误模型

共享层错误需至少可区分：

- `ValidationError`
- `ConfigReadError`
- `ConfigParseError`
- `ResourceUnavailableError`
- `ExecutionError`

CLI 可以继续渲染为人类可读文本，Tauri command 则需映射为可序列化错误响应。

## 5. 配置文件与资源路径方案

### 5.1 为什么必须改造

当前代码通过 `CARGO_MANIFEST_DIR/config` 读取配置，这只适用于源码运行环境，不适用于桌面打包后的 `.app`。

### 5.2 统一策略

共享应用层改为接收显式配置目录，不再自行写死目录位置。

建议引入 `AppPaths` 或等价结构：

```rust
pub struct AppPaths {
  pub config_dir: PathBuf,
}
```

调用侧职责：

- CLI：默认传入仓库根目录下的 `config/`
- Tauri：运行时使用应用可写目录中的 `config/`

### 5.3 Tauri 配置文件策略

GUI 首版采用“内置默认配置 + 首次运行复制到可写目录”的方式：

- 将仓库中的 `config/*.toml` 作为 Tauri bundle resources 打包
- Tauri 启动时检查 app data 目录下的 `config/`
- 若目标文件不存在，则从 bundle resources 复制默认版本
- 之后 GUI 运行时始终读取 app data 目录中的文件，而不是直接读取 bundle 内资源

该策略的好处：

- 打包后依然可运行
- 运维数据可在不重新打包 GUI 的情况下被替换
- 避免直接修改 `.app` 内部资源

首版不提供 GUI 内置配置编辑器，只在界面中展示当前配置目录路径和配置可用状态。

## 6. Tauri 桌面层设计

### 6.1 Tauri 子工程选择

`desktop/` 作为独立工程目录，包含：

- React/Vite 前端
- `src-tauri` Rust 桌面壳
- Tauri CLI 与前端构建脚本

### 6.2 Tauri commands

首版只定义两个明确命令：

#### `get_runtime_status`

用途：

- 返回当前 GUI 使用的配置目录
- 返回 `open_ids.toml`、`shop.toml`、`province.toml` 是否存在且可读取

返回结构建议：

```rust
pub struct RuntimeStatus {
  pub config_dir: String,
  pub open_ids_ready: bool,
  pub shop_ready: bool,
  pub province_ready: bool,
}
```

#### `run_reading_task`

用途：

- 接收结构化输入
- 调用共享 Rust 应用层执行任务
- 返回结构化汇总结果

参数命名统一使用 `camelCase` 暴露给前端，Rust 侧保持 `snake_case` 或通过 `rename_all` 对齐。

### 6.3 进度回传策略

首版采用“`invoke` 返回最终汇总 + Tauri event 推送中间进度”的混合方式。

具体事件：

- `reading-task://progress`
- `reading-task://completed`
- `reading-task://failed`

事件负载至少包含：

- 当前已完成数量
- 总数量
- 最近一条 `TaskItemResult`

选择事件而不是复杂通道的原因：

- 当前每次运行最多只会按 `count` 粒度回传
- 频率低，事件足够
- 前端实现更直接

## 7. React GUI 设计

### 7.1 页面范围

首版采用单窗口、单页工作台，包含四个区域：

- 顶部标题与说明
- 配置状态区
- 参数输入区
- 运行结果区

### 7.2 输入模型

GUI 表单字段与 CLI 保持一致：

- `课程 ID`
- `经理 ID`
- `FC`
- `数量`
- `门店代码`

输入规则：

- `count` 默认为 `1`
- `count` 必须为正整数
- `shopcodes` 输入框支持逗号或换行分隔
- 若 `shopcodes` 非空，则 GUI 仍要求 `fc` 输入，但实际执行逻辑以指定门店优先，与现有 CLI 保持一致

### 7.3 交互流程

1. 启动应用
2. 调用 `get_runtime_status`
3. 展示配置目录和可用状态
4. 用户填写参数
5. 前端做基础校验
6. 点击“开始执行”
7. 按钮进入禁用态，显示运行中状态
8. 调用 `run_reading_task`
9. 监听进度事件，增量更新结果列表
10. 执行结束后展示汇总信息

### 7.4 结果展示

结果区至少展示：

- 开始时间和结束时间
- 总请求数
- 成功数
- 失败数
- 每条结果的 OpenID、ShopCode、地区、HTTP 状态、响应文本或错误信息

### 7.5 首版明确不做

- 历史任务列表
- 参数模板保存
- 多任务并行运行
- 执行取消
- GUI 内编辑 TOML
- 图表统计

## 8. CLI 兼容策略

CLI 仍保留为一等入口，但职责简化为：

- 解析命令行参数
- 将输入转换为共享 `TaskRunRequest`
- 调用共享应用层
- 将 `TaskRunSummary` 渲染为当前风格的文本输出

兼容要求：

- 原有参数名不变
- 原有错误语义不变
- 逐条结果打印能力保留
- CLI 不依赖 Tauri 或前端代码

## 9. 实施顺序

### 阶段 1：共享核心重构

- 增加 `src/lib.rs`
- 将当前执行逻辑重构为共享服务
- 引入结构化结果类型与结构化错误类型
- 保证 CLI 回归可用

### 阶段 2：Tauri 壳工程搭建

- 初始化 `desktop/`
- 配置 React + Vite
- 配置 `desktop/src-tauri`
- 建立对根 Rust library 的 path dependency

### 阶段 3：资源与运行路径

- 配置 bundle resources
- 实现首次启动复制默认配置
- 实现 `get_runtime_status`

### 阶段 4：GUI 主工作台

- 完成表单、状态、结果列表
- 完成 `run_reading_task` 调用
- 接入进度事件

### 阶段 5：验证与打包

- 完成 Rust 单元测试与集成验证
- 完成 React 交互测试
- 完成 Tauri dev/build 冒烟
- 输出 macOS 本地运行与打包说明

## 10. 测试与验收标准

### 10.1 Rust 共享层

必须覆盖以下情况：

- `count == 0`
- `count` 超过可用 OpenID 数量
- `count` 超过可用门店数量
- 指定 `shopcodes` 但无匹配门店
- OpenID 去重后为空
- 配置文件缺失
- TOML 解析失败
- 结果汇总统计正确

### 10.2 CLI 回归

必须验证：

- 现有参数组合仍可运行
- 文本输出仍包含逐条执行结果
- 错误输出仍清晰可读

### 10.3 Tauri 层

必须验证：

- `get_runtime_status` 返回正确路径与状态
- `run_reading_task` 能返回结构化汇总
- 进度事件顺序正确
- 命令错误能正确映射到前端

### 10.4 React 层

必须验证：

- 默认值正确
- 输入校验正确
- 执行中按钮禁用
- 成功、部分失败、整体失败三种结果可展示
- 结果列表能在运行中增量更新

### 10.5 macOS 验收

以以下结果作为本轮完成标志：

- `desktop` 开发模式可以启动 GUI
- 可以通过 GUI 完成一次真实任务执行
- 可以生成 macOS 应用包
- 打包后的应用能够读取 app data 目录中的配置文件

## 11. 风险与控制措施

### 风险 1：根 Rust crate 与 Tauri 子工程耦合不清

控制：

- 明确要求根 crate 提供 library target
- `desktop/src-tauri` 只依赖 library，不依赖 CLI bin

### 风险 2：配置路径在打包后失效

控制：

- 禁止在共享层继续写死 `CARGO_MANIFEST_DIR`
- 强制由调用侧注入配置目录
- 使用 bundle resources + 首次复制机制

### 风险 3：GUI 直接复写业务导致逻辑分叉

控制：

- 禁止前端重写抽样与提交逻辑
- 统一由 Rust 共享层执行

### 风险 4：GUI 运行过程无反馈导致用户误判卡死

控制：

- 强制接入进度事件
- 显示当前完成数和最近一条结果

## 12. 任务拆解文件

本计划拆分为以下独立任务文档：

- `docs/taks/2026-03-31-tauri-gui-task-01-core-library.md`
- `docs/taks/2026-03-31-tauri-gui-task-02-cli-adapter.md`
- `docs/taks/2026-03-31-tauri-gui-task-03-desktop-shell.md`
- `docs/taks/2026-03-31-tauri-gui-task-04-runtime-config.md`
- `docs/taks/2026-03-31-tauri-gui-task-05-gui-workbench.md`
- `docs/taks/2026-03-31-tauri-gui-task-06-verification-and-packaging.md`

## 13. 实施默认值与明确假设

- 当前日期按 `2026-03-31` 生成计划与任务文件名
- `docs/taks/` 沿用仓库现有目录名，不在本轮顺手改为 `tasks`
- Tauri 使用当前稳定 `2.x` 主线；具体 patch 版本在实施当天统一锁定为最新稳定版本
- React 使用当前稳定版本，配合 Vite
- GUI 首版只做中文界面
- GUI 首版不新增鉴权、代理、自定义请求头配置项

## 14. 外部参考

- Tauri Create Project: https://v2.tauri.app/start/create-project/
- Tauri Calling Rust from the Frontend: https://v2.tauri.app/develop/calling-rust/
- Tauri Embedding Additional Files: https://v2.tauri.app/develop/resources/
- Tauri Prerequisites: https://v2.tauri.app/start/prerequisites/
