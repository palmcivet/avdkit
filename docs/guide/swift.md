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

## 构建

`swift/` 是 Swift 出口的目录，放着本地清单、手写源码、测试、绑定生成器和构建脚本。绑定生成器在 `swift/bindgen/`。它是独立的 Cargo 包，不加入主 workspace，也不链进静态库；包本身只包装 UniFFI 命令行入口，UniFFI 版本与 `avdkit-ffi` 相同。

`swift/build.sh` 先构建这个生成器，再根据 `avdkit-ffi` 的动态库生成 Swift 绑定，并创建只包含 macOS arm64 切片的 `AvdkitFFI.xcframework`。生成的 `swift/Sources/Avdkit/Generated.swift` 与 XCFramework 都留在本机，不进入版本控制。脚本固定构建 `aarch64-apple-darwin` 静态库，用 `lipo` 拒绝包含其他架构的产物，再执行 Swift 包测试和取消链验证。

未设置 `DEVELOPER_DIR` 时，构建、打包和发布脚本选择本机 Swift 工具链不低于 6.0 的 Xcode。macOS 14 的 GitHub runner 把默认 `Xcode.app` 指到 15.4（Swift 5.10）；Swift 6 在同镜像并列安装的 Xcode 16 里。

`swift/Package.swift` 把 binary target 指到本地 `swift/Artifacts/AvdkitFFI.xcframework`，只给这次构建和持续集成用。

## 发布与引入

`swift/release.sh <version>` 调用 `swift/build.sh` 和 `swift/package.sh`。zip 地址由 `GITHUB_REPOSITORY` 或 `origin` 组成：`https://github.com/<owner>/<repo>/releases/download/<version>/AvdkitFFI.xcframework.zip`。`package.sh` 把该地址和校验和写入 `target/swift-package/`，其中只有清单、生成的绑定和手写的取消适配。

加上 `--publish` 时，脚本用临时索引把这棵树提交到 `release/swift`。分支尚不存在时，这次提交没有父提交；否则父提交是该分支的远端顶端。提交打上与版本号相同的附注标签，并快进推送。zip 和校验和文件挂到该标签的 GitHub Release。推送后脚本下载 Release 里的 zip，复核校验和，再用这个标签完成一次 Swift 构建。

版本号是 `MAJOR.MINOR.PATCH`。同名标签已存在时脚本拒绝覆盖。开发分支的 `swift/Package.swift` 仍指向本地 XCFramework，URL 不写入开发分支。

GitHub Actions 工作流 Swift release 在 macOS 14 上手动执行 `swift/release.sh <version> --publish`。

调用方依赖本仓库的版本标签。`package` 是仓库名的小写形式，产品名是 `Avdkit`：

```swift
dependencies: [
    .package(url: "https://github.com/<owner>/<repo>", from: "0.1.0"),
],
targets: [
    .target(
        name: "App",
        dependencies: [
            .product(name: "Avdkit", package: "<仓库名>"),
        ]
    ),
]
```

```swift
import Avdkit

let kit = try Kit(config: defaultKitConfig())
let report = try await kit.environment()
```
