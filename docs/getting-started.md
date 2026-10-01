# 快速开始

本文带你从源码构建 CLI，完成本机环境、能力和预设机型的第一次查询。概念和架构见[概念](guide/concepts.md)与[架构](explanation/architecture.md)。

## 前置条件

当前可执行的领域操作只覆盖 **macOS arm64**。Linux 可以编译、探测环境并返回能力矩阵，但领域能力会包含 `platform_not_supported`。不支持 Windows。

本机需要：

- Rust 1.80 或更高
- Android CLI（`android`）
- Android Emulator（`emulator`）
- platform-tools（`adb`）

SDK 根目录按 `KitConfig.sdk_root`、`ANDROID_HOME`、`ANDROID_SDK_ROOT` 的固定优先级选择，详见[环境与路由](explanation/environment.md)。

测试与实验只能创建带测试前缀的 AVD。前缀由品牌常量派生，当前为 `avdkit_test_`。不要用本库操作没有该前缀的设备，除非你明确知道自己在管理真实环境。

## 构建

在仓库根目录：

```sh
cargo build -p avdkit-cli
```

可执行文件名为 `avdkit`。也可以不安装，直接：

```sh
cargo run -p avdkit-cli -- --help
```

Rust 调用方依赖 workspace 包 `avdkit`（crate 目录是 `crates/core`）。正式名称确定前不发布到 crates.io，只通过 Git 或路径依赖使用。

Swift 绑定需要另外构建 XCFramework，见 [Swift](guide/swift.md)。

## 第一次查询

先看本机环境和能力：

```sh
cargo run -p avdkit-cli -- environment
cargo run -p avdkit-cli -- capabilities
```

加上 `--json` 得到 `Envelope<T>`，字段与 Rust / Swift 共用同一套 `model`：

```sh
cargo run -p avdkit-cli -- --json devices profiles
```

成功时，`devices_profiles` 的能力状态为 `available`，实现名为 `android_cli`。Android CLI 不在 PATH 或 SDK 中时，该能力为 `unavailable`，原因是 `tool_not_found`，而不是 `not_implemented`。

等价的 Rust 调用：

```rust
use avdkit::{Kit, KitConfig};

let kit = Kit::new_async(KitConfig::default()).await?;
let profiles = kit.profiles().await?;
```

## 下一步

- 理解能力、策略和三种操作形态：[概念](guide/concepts.md)
- 列出已有设备：[设备](guide/devices.md)
- 创建、启动、停止：[创建计划](guide/plans.md)、[运行时](guide/runtime.md)
- 命令一览：[命令行](guide/cli.md)
