# 环境探测与领域路由

环境快照把一次领域调用需要依赖的主机、路径、工具和已安装包固定下来。领域路由只根据这份快照选择实现，不在操作过程中重新读取 PATH 或切换 SDK。

## 环境来源

探测首先读取当前进程环境。macOS 还启动用户的登录 shell 并读取其环境，用于覆盖 GUI 应用没有继承终端配置的场景。

PATH 的合并规则是保留进程路径顺序，再追加登录 shell 中尚未出现的目录。其他变量由当前进程优先；只在进程没有该值时采用登录 shell。公开快照只记录与 Android 和平台路径有关的变量，不暴露其他进程环境。

每个记录值带有来源：

- `process_environment`：当前进程；
- `login_shell`：macOS 登录 shell；
- `caller_override`：`KitConfig`；
- `android_cli`：Android CLI 查询；
- `platform_default`：平台预设值。

进程和登录 shell 对同一个 Android 环境变量给出不同值时，探测仍按优先级继续，但会生成 `source_conflict` 诊断。

## SDK 根目录选择

SDK 根目录使用固定优先级：

1. `KitConfig.sdk_root`；
2. `ANDROID_SDK_ROOT`；
3. `ANDROID_HOME`；
4. `android info sdk`；
5. 平台默认路径。

选中的路径和来源分别进入 `paths.sdk_root` 与 `sdk_root_source`。所有较低优先级的不同路径都保留在冲突诊断中，因此调用方能解释“为什么使用这个 SDK”，而不需要复现库内选择逻辑。

Android CLI 从合并后的 PATH 查找。探测执行 `android --version`，只有命令成功且版本可解析时才将工具标记为 `available`。服务条款或首次运行提示映射为 `tool_not_ready`；其他异常输出和进程失败也不会把工具误判为可用。`android info sdk` 失败是非致命诊断，SDK 仍可从其他来源选择。

## SDK 文件扫描

选定 SDK 后扫描以下位置：

- `emulator/source.properties` 与 emulator 可执行文件；
- `platform-tools/source.properties` 与 adb；
- `cmdline-tools/*/source.properties` 及对应的 sdkmanager、avdmanager；
- `system-images/**/source.properties`。

`Pkg.Revision` 转换为统一的 `Revision`。部分官方包不写 `Pkg.Path`，此时从 `source.properties` 的 SDK 相对目录推导包 ID。已安装工具包和系统镜像进入 `installed_packages`，`Kit::list_installed()` 直接返回这份快照数据。

`cmdline-tools` 在当前实现中只进入环境报告，不参与领域路由，也不检查 Java。`tools/bin` 下的旧工具单独进入 `legacy_tools`，状态为 `report_only`，不会成为任何操作的实现。

## 主机 ABI

平台层在运行时读取主机架构，而不是在其他 crate 中按编译目标分支。已知架构转换为 Android ABI：

- Arm64 对应 `arm64-v8a`；
- x86_64 对应 `x86_64`；
- 未识别架构将 ABI 标记为 `unavailable`。

平台支持状态仍由运行时平台与架构共同决定。未实现平台可以完成环境探测并返回能力矩阵，但领域能力包含 `platform_not_supported`。

## 固定领域路由

`core` 中的 `Router` 是不持有状态的领域路由器。当前固定选择为：

- 环境、能力与刷新使用平台探测；
- 设备列表和详情使用 AVD 文件；
- 预设机型使用 Android CLI；
- 已安装包使用 SDK 文件扫描；
- 运行发现使用 adb 与平台发现文件实现；
- 启动使用 emulator；
- 停止优先使用 `adb emu kill`，Android CLI 为备选；
- 包管理使用 Android CLI；
- 创建请求先使用计划编译器，计划中的实际创建工具为 Android CLI。

路由表与可用性判断分开：路由可以声明未来操作应使用哪个实现，而能力仍会在实现尚未接入时返回 `not_implemented`。运行发现与开机状态要求 adb；启动同时要求 emulator 和 adb；停止优先要求 adb，缺失时可选择 Android CLI。工具存在但未就绪时返回 `tool_not_ready`，工具不存在时返回 `tool_not_found`。

能力矩阵只使用平台支持状态、工具状态与版本、路由和实现接入状态。AVD 是否运行、名称是否冲突、镜像是否满足某次创建请求等动态条件不进入矩阵，由具体操作的预检判断。

## 异步边界

登录 shell 和 Android CLI 通过统一进程执行层运行，受固定命令超时和进程组清理约束。平台描述、SDK 目录扫描和 `source.properties` 读取进入 Tokio blocking pool，不占用异步 worker。

`Kit::new_async()` 和 `refresh()` 使用完整异步探测。同步的 `Kit::new()` 在独立运行时完成同一条探测路径，因此同步与异步构造不会生成语义不同的快照。
