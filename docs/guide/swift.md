# Swift

Rust 门面、CLI JSON 和 Swift 绑定共享 `model` 中的稳定语义。Swift 出口只负责语言运行时适配，不重新解释 Android 工具输出。

## UniFFI 边界

`avdkit-ffi` 生成静态库、动态库和 Rust `rlib`。UniFFI 导出：

- `Kit` 及同步构造、异步查询和领域方法
- 环境、能力、设备、包、计划、事件和结果记录与枚举
- 稳定错误码对应的 `BindingError` 变体
- `Operation.nextEvent()`、`cancel()` 和 `result()`

标识符在 Swift 边界使用字符串，进入 `core` 前仍经过 Rust 构造器校验。计划记录同时保留供 Swift 展示的步骤和完整序列化 JSON，执行时回到同一 Rust `Plan` 校验路径。

Rust 核心中可扩展的枚举标注了 `#[non_exhaustive]`，而 Swift 枚举一旦生成就是封闭的。核心在次版本中新增的值按以下规则进入 Swift：

- 有合理归类的，映射到已有分支：错误码归为 `internal`，平台归为 `unsupported`，CPU 架构和包类型归为 `other`
- 其余枚举带有显式的 `unknown` 分支：`CapabilityId`、`ReasonCode`、`BootStatus`、`PlanKind` 和 `PlanStepKind`
- `Event` 与 `OperationResult` 的 `unknown` 分支携带该值的完整 JSON

当前核心的每个值都有对应的 Swift 分支，`unknown` 只用于承接更新版本核心新增的值。

## 并发易用层

Swift 包在生成绑定上增加两个接口：

- `Operation.events`：`AsyncThrowingStream<Event, Error>`
- `Operation.value()`：等待最终结果

两者在 Swift 任务或流终止时调用幂等的 `Operation.cancel()`。`value()` 使用 `withTaskCancellationHandler`，并在 Rust future 返回后调用 `Task.checkCancellation()`，因此调用方看到标准 `CancellationError`。

UniFFI 生成的 Swift async 桥接不会自动传播 `Task.cancel()`。必须由手写易用层把 Swift 取消接到 `Operation.cancel()`。实测记录见 [UniFFI 与 Swift 6](../findings/uniffi-swift.md)。

验证程序使用临时 SDK 和真实 shell 进程模拟 Android CLI 创建命令。它通过生成的 `Kit` 和 `Operation` 发起计划、取消 Swift Task，并确认 Rust 进程组中的 shell 与子进程都退出，不只依赖独立的 UniFFI 实验。

## XCFramework 与 SwiftPM

`scripts/build-swift.sh` 固定构建 `aarch64-apple-darwin` 静态库，生成 Swift 绑定并创建只包含 macOS arm64 切片的 `AvdkitFFI.xcframework`。脚本用 `lipo` 拒绝包含其他架构的产物，再执行 Swift 包测试和取消链验证。

`swift/Package.swift` 的 binary target 使用本地 `swift/Artifacts/AvdkitFFI.xcframework`。产物不进入版本控制；正式名称和发布地址确定前不引用公共制品仓库。
