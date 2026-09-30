# avdkit

avdkit 把 Android 虚拟设备的生命周期做成可编程接口。它不是 Android CLI 的参数转发器：调用官方工具之前，先用结构化数据回答本机能做什么、缺什么、调用方是否允许相应副作用。

单个官方工具完不成的操作会编译成多步计划，统一预检、执行和失败补偿。库自己的错误码、退出码和事件保持稳定，不把底层工具的不一致（例如 `android` 失败时仍返回退出码 0）暴露给调用方。

## 出口

同一套 `model` 类型出现在三个出口：

- Rust：依赖包 `avdkit`，入口是 `Kit`
- 命令行：可执行文件 `avdkit`，查询输出 JSON，长任务输出 NDJSON
- Swift：UniFFI 绑定，加上手写的异步与取消适配

## 当前能做什么

在 macOS arm64 上，公开出口已经贯通：

- 环境探测、能力查询与刷新
- Android CLI 预设机型
- 从 AVD 文件读取设备列表与详情
- 创建计划的编译与执行、删除与测试设备清扫
- 运行实例、开机状态、启动与停止
- CLI 与 Swift 绑定

公开类型里已经声明、但当前没有实现的入口返回 `capability_unavailable`，原因是 `not_implemented`。调用方不会看到空结果或挂起的任务。

## 明确不做

- 构建、APK 安装、UI 自动化
- 通用的 adb 设备交互；adb 只用于生命周期需要的查询
- 已废弃的 `tools/bin`
- 自行实现 SDK 仓库协议；下载与安装仍调用官方工具

## 怎么读

按角色选入口，不必按文件编号读完。

| 角色 | 从这里开始 |
| --- | --- |
| 第一次使用 | [快速开始](getting-started.md) |
| 集成 Rust / CLI / Swift | [概念](guide/concepts.md)、[策略](guide/policy.md)、[设备](guide/devices.md)、[创建计划](guide/plans.md)、[运行时](guide/runtime.md)、[命令行](guide/cli.md)、[Swift](guide/swift.md) |
| 查错误码、能力、JSON | [错误](reference/errors.md)、[能力](reference/capabilities.md) |
| 理解设计 | [架构](explanation/architecture.md)、[环境与路由](explanation/environment.md) |
| 改库 | [进程执行](internals/process.md)、[工具驱动](internals/drivers.md)、[测试](internals/testing.md) |

[findings/](findings/README.md) 只记录对官方工具的实测事实，不写设计决定。
