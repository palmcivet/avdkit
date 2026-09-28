- [UniFFI 与 Swift 6](#uniffi-与-swift-6)
  - [Swift 6](#swift-6)
  - [取消](#取消)
  - [体积](#体积)

# UniFFI 与 Swift 6

验证环境：

- macOS 15.7.7 arm64；
- Xcode 26.3，Swift 6.1.2；
- Rust 1.98.1；
- UniFFI 0.32.2。

验证代码位于 `experiments/uniffi-swift/`，通过 `validate.sh` 重复执行。实验导出一个同步函数和一个持有取消信号的异步对象；异步方法启动独立进程组，组内包含一个 shell 和一个子进程。

## Swift 6

UniFFI 生成的 Swift 代码与验证程序在以下设置下编译通过：

- Swift 语言版本 6；
- 完整严格并发检查；
- 警告视为错误。

生成的对象协议继承 `Sendable`，生成的对象类标记为 `@unchecked Sendable`。因此对象可以被 Swift 任务和取消处理器捕获，不需要修改生成代码。

使用 Tokio API 的导出异步方法必须标记 `async_runtime = "tokio"`。只启用 UniFFI 的 `tokio` Cargo feature 不够；省略该标记时，方法会在创建 Tokio 子进程时因没有 reactor 而 panic。

## 取消

直接取消等待 UniFFI async 方法的 Swift `Task`，不会取消 Rust future。生成的异步桥接使用 continuation 轮询 Rust future，但没有安装 Swift task cancellation handler。取消后等待 250 毫秒，实验中的 shell 和子进程仍然存活。

可工作的链路是：

1. Rust 对象公开同步的 `cancel()` 和异步的 `wait()`。
2. Swift 易用层用 `withTaskCancellationHandler` 包装 `wait()`。
3. `onCancel` 调用 Rust 对象的 `cancel()`。
4. Rust 收到信号后结束等待，并终止为该调用创建的整个进程组。
5. Swift 在等待返回后调用 `Task.checkCancellation()`，向调用方抛出 `CancellationError`。

实验确认该链路会结束 shell 和 shell 创建的子进程；本次运行从调用 `Task.cancel()` 到两个 PID 都消失用了 29 毫秒。仅依赖 Tokio `Child.kill_on_drop(true)` 不能表达“终止进程组”；实现需要在启动时创建独立进程组，并在取消保护器中向负的进程组 ID 发送终止信号。

因此，Swift `Task` 取消不能只依赖 UniFFI 生成代码。Swift 包必须提供手写易用层，Rust 的长任务对象必须保留显式、幂等的取消入口。

## 体积

最小实验包含 UniFFI、Tokio runtime、进程支持和取消链路。绑定生成器通过 Cargo feature 与分发库隔离，不计入产物。macOS arm64 release 构建的实测结果：

- 未剥离的 Rust 静态库：19,849,984 bytes；
- 剥离符号后的静态库：13,167,416 bytes；
- 单切片 XCFramework：约 12.6 MiB；
- XCFramework ZIP：4,593,408 bytes；
- 链接并执行验证逻辑的 Swift 可执行文件：1,036,136 bytes。

这些数字是最小验证程序的基线，不代表完整 avdkit 的最终大小。加入领域模型、驱动和其他架构切片后需要重新测量。
