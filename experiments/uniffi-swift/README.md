# UniFFI 与 Swift 6 验证

这个独立实验验证 P0 Swift 绑定中风险最高的链路：

- UniFFI 生成代码能在 Swift 6 严格并发模式下通过编译；
- Swift `Task` 取消能够通过显式适配层通知 Rust；
- Rust 能终止工具进程及其整个进程组；
- macOS arm64 静态库能够打包为 XCFramework，并测量分发体积。

运行：

```sh
./validate.sh
```

脚本需要 Rust、Swift 6 和完整 Xcode。产物写入忽略版本控制的 `target/`。实验不调用 Android 工具，也不创建或修改 AVD。

这里保留的 `CancellationProbe` 只用于验证跨语言运行时行为，不是公开 API 设计。实测结论见 [`docs/findings/uniffi-swift.md`](../../docs/findings/uniffi-swift.md)。
