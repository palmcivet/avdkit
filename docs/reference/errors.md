# 错误与 JSON

所有出口共享 `model` 中的 `Error`、`Envelope` 和字段命名。调用方应匹配稳定码，而不是解析底层工具文本。

## `Error`

重要字段：

- `code`：调用方应依赖的稳定错误码
- `message`：用于展示或日志
- `reasons`：能力不可用的结构化原因
- `failed_step`：组合计划失败的位置
- `compensations`：原始失败后各项补偿的成功状态和消息
- `diagnostic`：可选的底层命令、stdout、stderr 和退出状态

底层工具文本只进入诊断，不作为公共控制流。调用方应匹配 `code` 和 `reasons`。

`diagnostic` 在 Rust 中装箱存放，使 `Error` 保持在 128 字节以内，可以按值放入 `Result` 返回；JSON 形态不受影响。

## 错误码

| 码 | 含义 |
| --- | --- |
| `capability_unavailable` | 平台、工具或实现不可用 |
| `invalid_input` | 公开模型或参数无效 |
| `device_not_found` | 请求的 AVD 不存在 |
| `package_not_found` | 请求的 SDK 包不存在 |
| `precondition_failed` | 本次请求不满足条件 |
| `name_conflict` | 名称已被占用 |
| `device_running` | 目标 AVD 必须先停止 |
| `tool_not_found` | 所需可执行文件不存在 |
| `tool_output_unrecognized` | 工具运行了，但输出不符合已知契约 |
| `launch_failed` | emulator 在启动过程中失败 |
| `timeout` | 执行没有在时限内完成 |
| `cancelled` | 调用方取消了操作 |
| `platform_not_supported` | 当前主机没有实现 |
| `internal` | 库内部或底层 I/O 失败 |

`ErrorCode` 标注 `#[non_exhaustive]`。Rust 调用方匹配时需要通配分支；Swift 把未知错误码映射为 `internal`，见 [Swift](../guide/swift.md)。

## JSON 信封

公开模型都可以序列化，字段统一使用 `snake_case`。CLI 的完整 JSON 响应使用：

```json
{
  "schema_version": "0.1.0",
  "data": {}
}
```

`Envelope<T>` 的版本属于数据契约，不是 Android 工具版本。Rust API、CLI JSON 和 UniFFI 边界共用同一套 `model` 类型。

计划文件同样包在信封里。`plan show` 与 `plan execute` 只接受与当前构建相同 `schema_version` 的文件。

## 可扩展枚举

会随新工具、平台或操作增长的公开枚举标注为 `#[non_exhaustive]`，包括 `ErrorCode`、`ReasonCode`、`CapabilityId`、`Event`、`OperationResult`、`BootStatus`、`PlanKind`、`PlanIntent`、`PlanStepKind`、`PackageKind`、`Platform`、`CpuArchitecture`、`EnvironmentDiagnosticCode` 和 `RemedyKind`。库在次版本中新增变体不构成破坏性变更。`Field`、`LogStream`、`CapabilityState` 等封闭集合保持穷尽匹配。
