# 命令行

Rust 门面、CLI JSON 和 Swift 绑定共享 `model` 中的稳定语义。CLI 只负责参数、呈现和进程信号，不重新解释 Android 工具输出。

构建与第一次查询见[快速开始](../getting-started.md)。

## 命令

全局选项 `--json` 适用于所有子命令。

查询：

```text
avdkit environment
avdkit capabilities
avdkit refresh
avdkit devices list
avdkit devices get <id>
avdkit devices profiles
avdkit runtime running
avdkit runtime boot-status <id>
```

生命周期：

```text
avdkit runtime start <id>
avdkit runtime stop <id>
```

删除要求显式审批。测试设备清扫只匹配由品牌常量派生的测试前缀：

```text
avdkit devices delete <id> --approve
avdkit devices cleanup-tests --approve
```

创建计划：

```text
avdkit plan create --id <id> --profile <profile> --image <image> [--display-name <name>] [--output <path>]
avdkit plan show <path>
avdkit plan execute <path> --approve
```

`--output` 保存可再次读取的完整 JSON 计划。执行器仍会重新编译并校验计划，`--approve` 只表达调用方已经审阅副作用，不绕过能力、策略或预检。

写出的计划文件和 JSON 响应一样使用 `Envelope<Plan>`，带有 `schema_version`。`plan show` 与 `plan execute` 只接受与当前构建相同 schema 版本的文件，否则返回 `invalid_input` 并说明文件的版本。

## JSON 与 NDJSON

短查询使用一个格式化的 `Envelope<T>` JSON 文档。长任务使用 NDJSON：每个 `Event` 是独立的一行 `Envelope<Event>`，最后一行是成功的 `OperationResult` 或结构化 `Error`。

长任务错误的最终 JSON 写入 stdout，使只读取 NDJSON 流的调用方不会丢失终态；此时 stderr 保持为空。非 JSON 模式仍把最终错误写入 stderr。

CLI 捕获 Ctrl-C 后调用 `Operation.cancel()`，继续排空事件并等待结构化终态；取消错误因此仍是 NDJSON 最后一行，并使用退出码 `5`。

信封字段见[错误](../reference/errors.md)。

## 稳定退出码

CLI 不暴露 Android 工具退出码：

| 退出码 | 含义 |
| --- | --- |
| `0` | 成功 |
| `1` | 执行失败、未知工具输出或内部错误 |
| `2` | Clap 用法错误或无效输入 |
| `3` | 能力、工具或平台不可用 |
| `4` | 请求预检失败，包括对象缺失、名称冲突和设备运行中 |
| `5` | 取消或超时 |
