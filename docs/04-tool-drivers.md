# 工具驱动

[上一章](03-process-execution.md)得到的是原始 stdout、stderr 和终止状态。本章说明 `drivers` 如何把这些不稳定的工具行为隔离起来，使 `core` 只面对可靠的数据和错误。

## 驱动在项目中的边界

P0 涉及三个官方可执行文件：

- `android`：SDK 包管理、六个预设机型以及部分 AVD 操作；
- `emulator`：当前解析 AVD 列表；直接启动尚未接入；
- `adb`：运行实例、开机状态和停止控制。

每个驱动负责单条命令的构造和解释，不负责：

- 判断领域操作应该选哪个工具；
- 组合多个命令和文件步骤；
- 应用调用方策略；
- 把工具原始结构直接暴露给公共 API。

这些决策留在 `core`。因此同一个驱动解析器可以被环境探测、查询和组合计划复用。

## 从领域请求到解析结果

一条工具调用在项目中的路径是：

```text
core 选择实现
  → Invocation 构造命令
  → ToolEnvironment 注入路径
  → Runner 执行
  → NormalizedOutput 清洗
  → 命令专用解析器验证成功或分类错误
  → core 映射为 model 类型
```

以预设机型为例：

```rust
let environment = ToolEnvironment::from(&snapshot.paths);
let invocation = Invocation::android(
    android_path,
    &snapshot.paths.sdk_root,
    policy.android_cli_metrics,
    ["emulator".into(), "create".into(), "--list-profiles".into()],
)
.with_environment(&environment);

let output = execute(&runner, invocation).await?;
let profile_ids = android::parse_profiles(&output)?;
```

解析器返回工具层的 `ProfileId` 列表。`core` 再补上公共 `Profile` 的字段语义。

## 命令构造

`Invocation` 保存：

- `Tool` 种类；
- 已探测到的可执行文件；
- 独立参数；
- 本次调用的环境变量。

Android CLI 调用始终显式加入：

```text
--sdk=<snapshot.paths.sdk_root>
```

这使环境快照成为唯一 SDK 来源，避免用户 `~/.androidrc` 中的参数把命令重定向到另一个 SDK。只有策略值为 `no_metrics` 时才加入 `--no-metrics`。

`ToolEnvironment` 从 `PlatformPaths` 构造，并覆盖子进程中的：

- `ANDROID_SDK_ROOT`；
- `ANDROID_USER_HOME`；
- `ANDROID_AVD_HOME`；
- 可选 `JAVA_HOME`。

它不修改当前进程或用户配置。查询调用使用 `execute`；属于长任务的调用使用 `execute_cancellable`。

## 为什么不能相信退出码

Android CLI 1.0.x 已实测在以下失败中仍返回 0：

- AVD 不存在；
- AVD 名称冲突；
- SDK 包不存在。

错误有时写入 stdout，有时写入 stderr。因此统一规则是：

1. 先在两个输出流中匹配已知错误特征；
2. 再验证这条命令自己的成功格式；
3. 两者都不匹配时返回 `tool_output_unrecognized`。

不能编写“退出码为 0 就返回空列表”的解析器。空列表必须是该命令明确定义且能够识别的成功输出。

已知错误映射到稳定错误码，例如：

- `Package … not found.` → `package_not_found`；
- `AVD … already exists` → `name_conflict`；
- `Device … doesn't exist` → `precondition_failed`。

原始文本保留在 `Diagnostic`，但不会成为调用方必须解析的契约。

## 输出规范化

工具原始输出先转换为 `NormalizedOutput`。规范化只处理跨命令共有的噪声：

- 同时按 CR 和 LF 拆分，兼容原地刷新的进度；
- 删除 ANSI 颜色序列；
- 删除 `Warning:` 行；
- 删除 Android CLI 更新提示；
- 去除空行和每行两端空白；
- 仍然区分 stdout 与 stderr。

命令专用信息不能在这一阶段删除。解析失败时，诊断使用未清洗的原始输出，便于新增 fixture 和定位工具版本变化。

## 当前解析覆盖

### Android CLI

已实现：

- 版本号；
- `emulator create --list-profiles` 的六个预设机型；
- AVD 不存在、名称冲突、SDK 包不存在的错误识别。

预设机型已经接入 `Kit.profiles()`。版本解析和错误分类目前只在驱动测试中使用。

### emulator

已实现 `emulator -list-avds` 的名称解析。解析器允许空列表，并过滤可能混入 stdout 的 `INFO`、`WARNING` 和 `ERROR` 日志行。

它只返回 AVD ID，不从输出猜测详情。设备列表与详情由 `avdfs` 读取 `<id>.ini` 和 `config.ini`，见[设备文件查询](06-device-files.md)。

### adb

已实现 `adb devices -l`：

- 保留 serial；
- 保留 `device`、`offline`、`unauthorized` 等原始状态；
- 将 `product:`、`model:`、`transport_id:` 等列保存为键值字段。

驱动不会丢弃离线设备。哪些状态应进入公共运行实例，由 `core` 决定。

## 为什么不解析 Android CLI 的对齐表格

`android emulator list --long` 使用空格对齐列。CJK 显示名称的终端宽度与字符数不同，会让后续列错位。

项目因此采用更稳定的数据源：

- 设备列表和详情读取 AVD 目录；
- serial 与运行状态组合 adb 和运行发现文件；
- Android CLI 表格只作为必要时的备选，不作为主数据源。

## Fixture 是驱动契约

解析测试不在 CI 中安装 Android SDK，而是回放真实输出：

```text
fixtures/<平台>/<工具>/<版本>/<场景>.json
```

一个 fixture 同时记录输入与结果：

```json
{
  "command": ["android", "--version"],
  "status": 0,
  "stdout": "1.0.15985488\n",
  "stderr": "",
  "captured_at": "2026-09-30",
  "redactions": []
}
```

写入仓库前必须：

- 用 `<SDK>`、`<HOME>` 等占位符替换机器路径；
- 用测试前缀替换 AVD ID；
- 记录每项脱敏；
- 保留 CR、LF 和输出流归属；
- 不“修正”工具原本不合理的退出码。

当前 fixture 覆盖 macOS arm64 上的 Android CLI 1.0.15985488、emulator 37.1.11 和 adb 37.0.1。

## 为驱动增加一条命令

贡献者应按同一顺序完成：

1. 在测试前缀和只读优先的约束下录制真实成功与失败输出；
2. 构造独立参数和完整 `ToolEnvironment`；
3. 只做必要的公共规范化；
4. 先识别已知错误，再验证成功格式；
5. 未知格式返回 `tool_output_unrecognized` 和完整诊断；
6. 用 fixture 覆盖解析和错误分类；
7. 最后在 `core` 中映射为公共模型并开放能力。

下一章用已经接入的[预设机型查询](05-read-only-queries.md)展示这条链路如何端到端工作。
