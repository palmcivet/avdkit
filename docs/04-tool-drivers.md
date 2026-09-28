# 工具驱动

`drivers` crate 封装官方 Android 命令行工具的调用和输出语义，不决定设备生命周期或操作路由。

## 调用

`Invocation` 保存工具种类、可执行文件、参数和本次调用的环境变量。Android CLI 调用始终在参数中显式指定 SDK 根目录；指标策略要求时才加入 `--no-metrics`。

`ToolEnvironment` 从平台路径构造，并为每次命令注入：

- `ANDROID_SDK_ROOT`；
- `ANDROID_USER_HOME`；
- `ANDROID_AVD_HOME`；
- 可选的 `JAVA_HOME`。

这些值只覆盖子进程环境，不修改调用方的全局设置。普通查询通过 `execute` 运行；可取消调用通过 `execute_cancellable` 连接到进程执行层的取消令牌。

## 输出规范化

工具原始 stdout、stderr 和终止状态首先保留在 `process::Output`。解析前生成 `NormalizedOutput`：

- 按 CR 和 LF 拆分，兼容原地刷新的进度行；
- 删除 ANSI 控制序列；
- 删除 `Warning:` 行；
- 删除 Android CLI 更新提示；
- 去除空行和每行两端空白；
- stdout 与 stderr 仍然分开保存。

无法识别的输出返回 `tool_output_unrecognized`，并把未清洗的 stdout、stderr 和退出状态放入诊断信息。

## 当前解析器

Android CLI 解析器支持版本和六个预设机型，并识别以下已实测错误：

- AVD 不存在；
- AVD 名称冲突；
- SDK 包不存在。

错误识别不依赖退出码。每个具体命令的解析器还必须验证自己的成功特征，不能把退出码 0 单独视为成功。

emulator 解析器读取 `-list-avds` 的名称列表，并过滤混入 stdout 的日志行。adb 解析器读取 `devices -l`，保留 serial、连接状态和其余键值字段；离线或未授权设备不会在驱动层丢弃。

Android CLI 的对齐表格尚未作为数据源。设备详情优先读取 AVD 文件；运行实例由 adb 和发现文件组合，避免 CJK 显示宽度破坏列解析。

## 回放样本

真实输出按以下路径保存：

```text
fixtures/<平台>/<工具>/<版本>/<场景>.json
```

每个样本包含命令参数、stdout、stderr、退出状态、采集日期和脱敏记录。SDK、用户目录、AVD ID 等机器相关内容在写入仓库前替换为占位符；用于 AVD 的示例名称使用测试前缀。

当前样本覆盖 macOS arm64 上的 Android CLI 1.0.15985488、emulator 37.1.11 和 adb 37.0.1。解析测试直接回放这些文件，不要求 CI 安装 Android SDK。
