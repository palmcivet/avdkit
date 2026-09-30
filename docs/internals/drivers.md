# 工具驱动

`drivers` 把不稳定的官方工具行为隔离起来，使 `core` 只面对可靠的数据和错误。

## 边界

当前接入三个官方可执行文件：

- `android`：SDK 包管理、六个预设机型以及部分 AVD 操作
- `emulator`：解析 AVD 列表并由运行时执行器直接启动
- `adb`：运行实例、开机状态和停止控制

每个驱动负责单条命令的构造和解释，不负责判断领域操作应该选哪个工具、组合多个命令和文件步骤、应用调用方策略，或把工具原始结构直接暴露给公共 API。这些决策留在 `core`。因此同一个驱动解析器可以被环境探测、查询和组合计划复用。

## 从领域请求到解析结果

```text
core 选择实现
  → Invocation 构造命令
  → ToolEnvironment 注入路径
  → Runner 执行
  → NormalizedOutput 清洗
  → 命令专用解析器验证成功或分类错误
  → core 映射为 model 类型
```

解析器返回工具层数据。`core` 再补上公共模型的字段语义，例如把机型 ID 变成 `Profile`。

## 命令构造

`Invocation` 保存工具种类、已探测到的可执行文件、独立参数和本次调用的环境变量。

Android CLI 调用始终显式加入 `--sdk=<snapshot.paths.sdk_root>`。这使环境快照成为唯一 SDK 来源，避免用户 `~/.androidrc` 中的参数把命令重定向到另一个 SDK。只有策略值为 `no_metrics` 时才加入 `--no-metrics`。

`ToolEnvironment` 从 `PlatformPaths` 构造，并覆盖子进程中的 `ANDROID_HOME`、`ANDROID_SDK_ROOT`（两者都指向选中的 SDK）、`ANDROID_USER_HOME`、`ANDROID_AVD_HOME` 和可选 `JAVA_HOME`。它不修改当前进程或用户配置。查询调用使用 `execute`；属于长任务的调用使用 `execute_cancellable`。

## 为什么不能相信退出码

Android CLI 1.0.x 已实测在 AVD 不存在、AVD 名称冲突和 SDK 包不存在时仍返回 0。错误有时写入 stdout，有时写入 stderr。因此统一规则是：

1. 先在两个输出流中匹配已知错误特征
2. 再验证这条命令自己的成功格式
3. 两者都不匹配时返回 `tool_output_unrecognized`

不能编写“退出码为 0 就返回空列表”的解析器。空列表必须是该命令明确定义且能够识别的成功输出。

已知错误映射到稳定错误码，例如 `Package … not found.` → `package_not_found`，`AVD … already exists` → `name_conflict`，`Device … doesn't exist` → `precondition_failed`。原始文本保留在 `Diagnostic`，但不会成为调用方必须解析的契约。

## 输出规范化

工具原始输出先转换为 `NormalizedOutput`。规范化只处理跨命令共有的噪声：同时按 CR 和 LF 拆分、删除 ANSI 颜色序列、删除 `Warning:` 行、删除 Android CLI 更新提示、去除空行和每行两端空白，并仍然区分 stdout 与 stderr。

命令专用信息不能在这一阶段删除。解析失败时，诊断使用未清洗的原始输出，便于新增 fixture 和定位工具版本变化。

## 当前解析覆盖

### Android CLI

已实现版本号、`emulator create --list-profiles` 的六个预设机型，以及 AVD 不存在、名称冲突、SDK 包不存在的错误识别。预设机型已经接入 `Kit.profiles()`。版本解析和错误分类目前只在驱动测试中使用。

### emulator

已实现 `emulator -list-avds` 的名称解析。解析器允许空列表，并过滤可能混入 stdout 的 `INFO`、`WARNING` 和 `ERROR` 日志行。它只返回 AVD ID，不从输出猜测详情。设备列表与详情由 `avdfs` 读取，见[设备](../guide/devices.md)。

### adb

已实现 `adb devices -l` 以及运行时探针：保留 serial 与原始状态，将 `product:`、`model:`、`transport_id:` 等列保存为键值字段，解析 `getprop` 或 emulator console 返回的 AVD ID，判断 `sys.boot_completed` 与 `pm path android`，验证 `adb emu kill` 的确认输出。

驱动不会丢弃离线设备。哪些状态应进入公共运行实例，由 `core` 决定。

## 为什么不解析 Android CLI 的对齐表格

`android emulator list --long` 使用空格对齐列。CJK 显示名称的终端宽度与字符数不同，会让后续列错位。项目因此采用更稳定的数据源：设备列表和详情读取 AVD 目录，serial 与运行状态组合 adb 和运行发现文件。Android CLI 表格只作为必要时的备选，不作为主数据源。

## Fixture 是驱动契约

解析测试不在 CI 中安装 Android SDK，而是回放真实输出：

```text
fixtures/<平台>/<工具>/<版本>/<场景>.json
```

一个 fixture 同时记录命令、退出状态、stdout、stderr、采集日期和脱敏项。写入仓库前必须用 `<SDK>`、`<HOME>` 等占位符替换机器路径，用测试前缀替换 AVD ID，记录每项脱敏，保留 CR、LF 和输出流归属，并且不“修正”工具原本不合理的退出码。

当前 fixture 覆盖 macOS arm64 上的 Android CLI 1.0.15985488、emulator 37.1.11 和 adb 37.0.1。

## 为驱动增加一条命令

1. 在测试前缀和只读优先的约束下录制真实成功与失败输出
2. 构造独立参数和完整 `ToolEnvironment`
3. 只做必要的公共规范化
4. 先识别已知错误，再验证成功格式
5. 未知格式返回 `tool_output_unrecognized` 和完整诊断
6. 用 fixture 覆盖解析和错误分类
7. 最后在 `core` 中映射为公共模型并开放能力
