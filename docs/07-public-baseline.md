# 公开基线与契约测试

公开基线同时约束编译器版本、Rust API、序列化格式、命令行行为和异步执行边界。它们共同保证调用方升级代码时，不会因为内部实现变化而静默得到不同的契约。

## Rust 版本与发布边界

workspace 的最低支持 Rust 版本是 1.80。常规 CI 在 macOS、Linux 和 Windows 上使用稳定版工具链执行格式检查、Clippy 和单元测试；独立的 Linux 作业使用 Rust 1.80 和锁文件运行整个 workspace 测试。这样既验证当前工具链，也防止依赖或语法无意中抬高最低版本。

所有 package 都从 workspace 继承 `publish = false`。在正式名称确定前，`cargo publish` 因此不能把内部 crate 或出口误发到公共注册表；调用方只通过 Git 依赖使用仓库。

## Rust API 文档

Rust 调用方只依赖 `avdkit` 包。它重新导出的公共模型在 `model` crate 定义，因此 `core` 和 `model` 都启用 `missing_docs` 拒绝编译：

- 类型说明其在稳定契约中的职责；
- 字段说明值的来源和语义；
- 枚举变体说明状态或操作的含义；
- 方法说明阻塞、异步、取消和返回值行为。

其他 crate 是内部实现层，不构成支持调用方直接依赖的 Rust API。

## JSON golden 契约

公共模型的 golden 测试把确定的模型值格式化为 JSON，再和版本化文件逐字节比较。当前覆盖：

- `CapabilityMatrix` 的可用与不可用状态；
- `Plan`、步骤和补偿；
- `Error`、失败步骤和底层诊断；
- CLI 使用 `Envelope<Error>` 输出的完整响应。

golden 文件会捕获字段改名、标签方式、字段顺序和结构层级变化。修改公共格式时必须显式更新 fixture，并同时判断是否需要调整 `SCHEMA_VERSION`。

## CLI 进程级测试

CLI 集成测试启动真实 `avdkit` 可执行文件，而不是只调用内部函数。测试覆盖：

- 嵌套命令解析和成功 JSON 响应；
- Clap 用法错误返回退出码 `2`；
- 领域输入错误返回退出码 `2`；
- 计划创建、展示和显式审批；
- 长任务最终错误作为 NDJSON 最后一行输出。

查询成功响应写入 stdout，查询错误和诊断写入 stderr。长任务的事件和最终成功或错误全部写入 stdout，脚本只读取一条 NDJSON 流即可取得完整生命周期。

## Blocking 边界

环境探测和 AVD 文件扫描都包含同步文件系统调用。异步入口不直接在 Tokio worker 上执行这些调用：

- `Kit::new_async()` 和 `Kit::refresh()` 通过异步进程层执行 shell 与工具探针，并用 blocking pool 完成平台和 SDK 文件探测；
- `Kit::list_devices()` 和 `Kit::get_device()` 使用 blocking pool 扫描和读取 AVD 文件；
- blocking 任务无法完成时统一映射为 `internal`。

同步的 `Kit::new()` 仍可供没有异步运行时的调用方使用，并明确在当前线程完成探测。异步应用和 CLI 使用 `Kit::new_async()`。
