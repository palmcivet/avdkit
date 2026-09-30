# 策略

当 `Kit` 收到操作请求时，能力、策略和预检共同决定“可以做”以及“是否应该做”。三者不能混为一谈，概念摘要见[概念](concepts.md)。

本文说明 `KitConfig` 的当前校验规则、默认策略，以及不能关闭的安全契约。

## `KitConfig` 的当前用法

调用方在创建 `Kit` 时一次性提交配置：

```rust
use avdkit::{Kit, KitConfig};

let mut config = KitConfig::default();
config.sdk_root = Some("/opt/android-sdk".into());
let kit = Kit::new(config)?;
```

SDK 根目录覆盖已经可用。策略、工具偏好、超时和备份目录虽然已经进入公共类型，目前只接受默认值。公开类型里保留这些字段，传入其他值时返回 `capability_unavailable`，原因是 `not_implemented`。这样可以先稳定跨语言数据结构，同时避免配置被静默忽略。

配置在环境探测之前整体校验。任一不支持的值都会使 `Kit::new` 立即返回：

- 错误码 `capability_unavailable`
- 原因码 `not_implemented`
- 指向具体字段的消息，例如 `policy.license_acceptance`

不会出现部分字段生效、部分字段被忽略的状态。

## 默认策略

默认策略针对库和自动化调用设计：不做隐藏下载，不自动接受协议，但允许调用方明确请求包管理和 AVD 文件操作。

### 不允许隐式安装

`implicit_install` 默认为 `false`。

Android CLI 在空 SDK 中创建设备时，可能顺带下载 emulator、platform-tools 和系统镜像，还可能写入许可文件。avdkit 不允许底层工具隐藏这些副作用。正确行为是预检报告缺失依赖，由调用方决定是否另行执行显式安装。

它与 `explicit_install` 相互独立：禁止“创建时顺便安装”，不等于禁止调用方明确发起安装。

### 允许显式包管理

`explicit_install` 默认为 `true`，表示调用方可以明确请求安装、更新或删除 SDK 包。

这只是权限。操作仍需要相应能力和预检通过。包管理执行入口当前返回 `capability_unavailable`，原因是 `not_implemented`，因此默认允许也不会触发安装。

### 从不代为接受许可

`license_acceptance` 默认为 `never`。公共枚举还保留 `allow`，但当前配置为 `allow` 会在创建 `Kit` 时被拒绝。

已接受状态通过 `<sdk>/licenses/<license-id>` 中是否存在预期哈希判断：

- 文件缺失或不可读：未接受
- 内容格式无效：未接受
- 没有匹配哈希：未接受

avdkit 不解析工具的交互提示，因为提示文本会随 Android 工具版本变化。

### 允许 AVD 文件操作

`file_operations` 默认为 `true`，为修改 ID、改写配置和移动目录保留权限。

创建计划执行会通过这些 INI 原语移动 AVD、备份文件并原子改写配置，同时保留未知行、键顺序和值中的 `=`。关闭文件操作会在配置校验时返回 `not_implemented`。

### 沿用 Android CLI 指标设置

`android_cli_metrics` 默认为 `inherit`，调用时不传 `--no-metrics`，沿用用户已有设置。

公共枚举保留 `no_metrics`，但当前只接受默认值。无论采用哪种指标策略，Android CLI 调用都会显式传入 `--sdk=<path>`。

## 工具偏好不是副作用策略

`KitConfig.tool_preference` 描述一个操作优先走哪个实现，例如 Android CLI、emulator 二进制或文件读取。它不会放宽副作用权限，因此单独放在 `Policy` 之外。

当前只接受空覆盖表，由内置路由选择实现。自定义条目会像其他未实现配置一样在 `Kit::new` 时被拒绝。

## 不能关闭的安全规则

以下行为是 avdkit 的执行契约，不属于可配置策略：

- Android CLI 始终显式使用快照中的 SDK 路径，避免 `~/.androidrc` 改变目标
- 许可状态只根据 SDK 许可哈希文件判断
- 不删除或改写用户全局文件来绕过认证
- 不把底层工具的退出码直接暴露为公共成功语义
- 取消和超时要清理本次调用拥有的整个进程组
