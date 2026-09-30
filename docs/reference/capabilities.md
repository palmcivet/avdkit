# 能力与原因

`capabilities()` 返回所有已声明能力，而不只是当前可用项。每项带有实现名，或一组结构化原因和可选补救。调用方法仍会再次检查能力。

能力矩阵只使用平台支持状态、工具状态与版本、路由和实现接入状态。AVD 是否运行、名称是否冲突、镜像是否满足某次创建请求等动态条件不进入矩阵，由具体操作的预检判断。

## 原因码

| 码 | 含义 |
| --- | --- |
| `not_implemented` | 库声明了入口，但当前没有实现 |
| `tool_not_found` | 所需可执行文件不存在 |
| `tool_not_ready` | 可执行文件存在，但尚不能使用（例如首次运行条款） |
| `platform_not_supported` | 当前主机平台没有实现 |
| `capability_unavailable` | 依赖的更底层能力不可用 |

补救（`Remedy`）可以指向另一次库操作，或给出调用方需在库外完成的步骤。`RemedyKind` 为 `library_operation` 或 `manual`。

区分「缺工具」和「未实现」：`devices_profiles` 在没有 Android CLI 时是 `tool_not_found`；`packages_install` 在任何环境都是 `not_implemented`。

## 当前能力

实现名是路由选中的候选，例如 `android_cli`、`avd_files`、`emulator`。未支持的平台上，下列已接入能力仍会变为 `unavailable`，原因包含 `platform_not_supported`。

| ID | 当前行为 |
| --- | --- |
| `environment` | 返回缓存的环境报告 |
| `capabilities` | 返回能力矩阵 |
| `refresh` | 重新探测并替换快照 |
| `packages_list_installed` | 返回快照中扫描到的已安装包 |
| `devices_list` / `devices_get` | 读 AVD 文件，实现名 `avd_files` |
| `devices_profiles` | Android CLI 六个预设档位 |
| `devices_plan_create` | 执行创建计划；编译计划本身不检查此能力 |
| `devices_delete` | 删除设备，要求 Android CLI 与标准 AVD 布局 |
| `runtime_running` | adb 与发现文件 |
| `runtime_boot_status` | 开机探测 |
| `runtime_start` | 预设参数启动 emulator |
| `runtime_stop` | 优先 `adb emu kill` |

下列入口已出现在公开枚举中，调用返回 `capability_unavailable`，原因是 `not_implemented`：

- `packages_list_available`、`packages_install`、`packages_remove`、`packages_update`
- `devices_plan_create_custom_hardware`、`devices_plan_edit`、`devices_plan_move`、`devices_plan_duplicate`
- `runtime_start_custom`、`runtime_stop_all`
- `snapshots_list`、`snapshots_save`、`snapshots_load`、`snapshots_delete`

创建与删除还要求 AVD 目录位于 `ANDROID_USER_HOME/avd`。单独设置的 `ANDROID_AVD_HOME` 会被 Android CLI 忽略，因此其他布局报告 `not_implemented`，而不是写到错误目录。见[环境与路由](../explanation/environment.md)。
