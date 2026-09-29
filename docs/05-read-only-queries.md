# 第一个纵向切片：预设机型查询

前五章已经分别介绍架构、策略、公共 API、进程执行和工具驱动。本章不再引入新的抽象，而是沿 `devices_profiles` 走完一次真实调用，说明这些部分如何协作。

这个查询适合作为第一个闭环，因为它：

- 使用真实 Android CLI；
- 经过公开 Rust API 和 CLI 出口；
- 有稳定的回放 fixture；
- 不创建、启动或修改 AVD；
- 不安装 SDK 包或接受许可。

## 调用前提

当前实现要求：

- 主机为 macOS arm64；
- 环境探测发现 Android CLI；
- `KitConfig` 使用当前支持的默认策略。

满足条件时，能力矩阵包含：

```json
{
  "id": "devices_profiles",
  "state": {
    "state": "available",
    "implementation": "android_cli"
  }
}
```

Android CLI 缺失时，该能力为 `unavailable`，原因包含 `tool_not_found`，不包含 `not_implemented`。这表示功能已经实现，只是当前环境缺依赖。

其他设备查询尚未接入时仍会包含 `not_implemented`。调用方可以据此区分“安装工具即可使用”和“当前版本尚未实现”。

## 从 Rust 调用

```rust
use avdkit::{Kit, KitConfig};

let kit = Kit::new(KitConfig::default())?;
let profiles = kit.profiles().await?;

for profile in profiles {
    println!("{}", profile.id);
}
```

本机 Android CLI 1.0.15985488 返回：

```text
large_desktop
medium_desktop
medium_phone
medium_tablet
small_desktop
small_phone
```

这些是 Android CLI P0 提供的固定档位，不是 `avdmanager list device` 的完整设备目录。

## 从命令行调用

人类可读输出：

```sh
avdkit devices profiles
```

结构化输出：

```sh
avdkit --json devices profiles
```

JSON 示例：

```json
{
  "schema_version": "0.1.0",
  "data": [
    {
      "id": "medium_phone",
      "display_name": {
        "state": "unavailable"
      }
    }
  ]
}
```

CLI 没有重新定义 DTO。它直接序列化 `Envelope<Vec<Profile>>`，所以 Rust、JSON 和 UniFFI 边界看到同一语义。

## 调用在项目内部怎样流动

### 1. `Kit` 检查能力

`Kit.profiles()` 首先要求 `CapabilityId::DevicesProfiles` 可用。即使调用方之前已经展示过能力矩阵，这里仍会检查，避免环境不支持时误执行。

### 2. 快照提供确定的路径

方法从 `EnvironmentSnapshot` 读取：

- Android CLI 可执行文件；
- SDK 根目录；
- Android 用户目录；
- AVD 目录。

查询期间不会重新读取 PATH 或环境变量，因此同一次调用不会突然切换 SDK。

### 3. 驱动构造调用

实际命令是：

```text
android --sdk=<SDK> emulator create --list-profiles
```

同时为子进程注入快照中的：

```text
ANDROID_SDK_ROOT
ANDROID_USER_HOME
ANDROID_AVD_HOME
```

默认指标策略为 `inherit`，所以不传 `--no-metrics`。

### 4. `Runner` 执行

进程执行层：

- stdin 连接空设备；
- 捕获 stdout 和 stderr；
- 应用默认超时；
- 为命令创建独立进程组；
- 在 future 被丢弃或超时时清理进程组。

这是短查询，没有公开 `Operation`；调用方通过取消包含它的 Rust future 停止等待。

### 5. 驱动验证输出

解析器先去除公共噪声，再要求：

- 退出状态为 0；
- 至少有一个输出行；
- 每一行都是不含空白的单个机型 ID。

已知工具错误会映射为稳定错误码。其他格式返回 `tool_output_unrecognized`，并在 `diagnostic` 中保留原始输出。

### 6. `core` 构造公共模型

驱动只返回 ID。Android CLI 这条命令没有提供可靠显示名称，因此 `core` 构造：

```text
Profile {
    id: "medium_phone",
    display_name: unavailable
}
```

avdkit 不把 `medium_phone` 猜成 `Medium Phone`，因为这种转换不是工具提供的事实。

## 如何诊断失败

先看稳定错误码：

- `capability_unavailable`：平台或 Android CLI 不可用；
- `tool_not_found`：快照中的可执行文件在执行前后失效；
- `timeout`：Android CLI 没有在预设时间内结束；
- `tool_output_unrecognized`：工具版本输出了尚未覆盖的格式；
- `internal`：启动或等待进程时发生其他 I/O 错误。

对于 `tool_output_unrecognized`，应把 `diagnostic` 中的 stdout、stderr 和退出状态连同本次已知调用参数录制为新的脱敏 fixture，再决定是工具升级还是新的已知错误。

## 这个切片证明了什么

预设机型查询验证了项目最小主链路：

```text
公开调用
  → 能力判断
  → 环境快照
  → 工具调用
  → 进程生命周期
  → 输出解析
  → 公共模型
  → Rust / JSON 出口
```

这个切片只证明了工具型查询。下一章的[设备文件查询](06-device-files.md)沿另一条路径读取 AVD 目录；运行实例发现和有副作用操作仍按各自能力状态返回结构化不可用结果。
