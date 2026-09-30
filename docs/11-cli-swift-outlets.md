# CLI 与 Swift 出口

Rust 门面、CLI JSON 和 Swift 绑定共享 `model` 中的稳定语义。出口只负责参数、呈现和语言运行时适配，不重新解释 Android 工具输出。

## CLI 命令

查询命令包括环境、能力、刷新、设备、机型和运行状态。生命周期命令位于：

```text
avdkit runtime running
avdkit runtime boot-status <id>
avdkit runtime start <id>
avdkit runtime stop <id>
```

删除同样要求显式审批；测试设备清扫只匹配由品牌常量派生的测试前缀：

```text
avdkit devices delete <id> --approve
avdkit devices cleanup-tests --approve
```

创建计划通过 `plan create` 编译。`--output <path>` 保存可再次读取的完整 JSON 计划；`plan show <path>` 展示计划；执行必须显式提供：

```text
avdkit plan execute <path> --approve
```

执行器仍会重新编译并校验计划，`--approve` 只表达调用方已经审阅副作用，不绕过能力、策略或预检。

## JSON 与 NDJSON

短查询使用一个格式化的 `Envelope<T>` JSON 文档。长任务使用 NDJSON：每个 `Event` 是独立的一行 `Envelope<Event>`，最后一行是成功的 `OperationResult` 或结构化 `Error`。

长任务错误的最终 JSON 写入 stdout，使只读取 NDJSON 流的调用方不会丢失终态；此时 stderr 保持为空。非 JSON 模式仍把最终错误写入 stderr。

CLI 捕获 Ctrl-C 后调用 `Operation.cancel()`，继续排空事件并等待结构化终态；取消错误因此仍是 NDJSON 最后一行，并使用退出码 `5`。

## 稳定退出码

CLI 不暴露 Android 工具退出码：

- `0`：成功；
- `1`：执行失败、未知工具输出或内部错误；
- `2`：Clap 用法错误或无效输入；
- `3`：能力、工具或平台不可用；
- `4`：请求预检失败，包括对象缺失、名称冲突和设备运行中；
- `5`：取消或超时。

## UniFFI 边界

`avdkit-ffi` 生成静态库、动态库和 Rust `rlib`。UniFFI 导出：

- `Kit` 及同步构造、异步查询和领域方法；
- 环境、能力、设备、包、计划、事件和结果记录与枚举；
- 稳定错误码对应的 `BindingError` 变体；
- `Operation.nextEvent()`、`cancel()` 和 `result()`。

标识符在 Swift 边界使用字符串，进入 `core` 前仍经过 Rust 构造器校验。计划记录同时保留供 Swift 展示的步骤和完整序列化 JSON，执行时回到同一 Rust `Plan` 校验路径。

## Swift 并发易用层

Swift 包在生成绑定上增加两个接口：

- `Operation.events`：`AsyncThrowingStream<Event, Error>`；
- `Operation.value()`：等待最终结果。

两者在 Swift 任务或流终止时调用幂等的 `Operation.cancel()`。`value()` 使用 `withTaskCancellationHandler`，并在 Rust future 返回后调用 `Task.checkCancellation()`，因此调用方看到标准 `CancellationError`。

验证程序使用临时 SDK 和真实 shell 进程模拟 Android CLI 创建命令。它通过生成的 `Kit` 和 `Operation` 发起计划、取消 Swift Task，并确认 Rust 进程组中的 shell 与子进程都退出，不只依赖独立的 UniFFI 实验。

## XCFramework 与 SwiftPM

`scripts/build-swift.sh` 固定构建 `aarch64-apple-darwin` 静态库，生成 Swift 绑定并创建单切片 `AvdkitFFI.xcframework`。脚本用 `lipo` 拒绝包含其他架构的产物，再执行 Swift 包测试和取消链验证。

`swift/Package.swift` 的 binary target 使用本地 `swift/Artifacts/AvdkitFFI.xcframework`。产物不进入版本控制；正式名称和发布地址确定前不引用公共制品仓库。
