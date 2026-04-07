# 任务 01：共享 Rust Core Library

## 目标

把当前 CLI 中可复用的业务流程提炼成根 crate 的 library target，为 CLI 和 Tauri 共用。

## 背景

当前仓库只有 CLI 入口和应用逻辑模块，没有可供外部依赖的库接口。Tauri 子工程若要复用业务逻辑，必须先有稳定的 Rust library 接口。

## 任务范围

- 新增 `src/lib.rs`
- 将运行输入、执行结果、汇总结果、错误类型抽象成可序列化结构
- 将配置读取、抽样、请求发送、结果汇总整理为共享服务
- 去掉共享层对 `CARGO_MANIFEST_DIR` 的硬编码依赖，改为显式路径注入

## 不在范围内

- `clap` 参数定义调整
- Tauri command 实现
- React 页面实现

## 关键实现要求

- 共享层接口必须可以从 CLI 和 Tauri 两端调用
- 共享层返回结构化结果，不能直接 `println!`
- 执行顺序保持与当前实现一致，不改为并发
- 当前错误语义必须保留，允许内部重构但不允许业务行为漂移

## 建议接口

```rust
pub struct AppPaths {
  pub config_dir: PathBuf,
}

pub struct TaskRunRequest {
  pub s_course_id: String,
  pub s_manager_id: String,
  pub fc: String,
  pub count: usize,
  pub shopcodes: Vec<String>,
}

pub async fn run_task(paths: &AppPaths, request: TaskRunRequest) -> Result<TaskRunSummary, AppError>
```

## 交付物

- 根 crate 可被其他 Rust crate 作为 library 引用
- 共享输入输出模型
- 共享错误模型
- 可复用执行入口

## 验收标准

- CLI 可以通过 library 接口继续工作
- 单元测试覆盖关键校验和加载逻辑
- `cargo test` 通过
- `cargo clippy --all-targets --all-features -- -D warnings` 通过

## 依赖关系

- 无前置依赖
- 为任务 02、03、04、05 提供基础接口
