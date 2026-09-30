# 运行时

运行时闭环组合 adb、平台发现文件和 emulator 进程。公开 API 不依赖 Android CLI 的对齐表格，也不把残留锁文件单独当作运行证据。

当前实现的运行时闭环只覆盖 macOS arm64。其他平台可以完成环境探测，但相关能力包含 `platform_not_supported`。

```sh
avdkit runtime running
avdkit runtime boot-status <id>
avdkit runtime start <id>
avdkit runtime stop <id>
```

## 运行实例发现

环境快照中的 `paths.runtime_root` 由平台层提供。macOS arm64 使用 `~/Library/Caches/TemporaryItems/avd/running`，其中 `pid_<pid>.ini` 记录 `avd.id`、`port.serial` 等值。

`running()` 先执行 `adb devices -l`，再按以下顺序确定每个 emulator serial 的 AVD ID：

1. 使用 serial 对应的发现文件，并确认文件名中的 PID 仍属于 emulator 进程
2. 对 adb 状态为 `device` 的实例读取 `ro.boot.qemu.avd_name`
3. 最后用两秒超时调用 `adb emu avd name`

失效发现文件被忽略。发现文件提供可靠 PID 时，`RunningInstance.pid` 为 `present`；通过 adb 回退得到名称时 PID 为 `unavailable`。离线实例只有在仍有有效发现文件时才能返回。

## 进程身份

emulator 崩溃后，发现文件可能残留，文件名中的 PID 之后可能被系统分配给无关进程。仅凭 PID 存活就发信号，会误杀用户的其他进程。因此每次采信发现文件、判断停止是否完成、发送 SIGTERM 或 SIGKILL 之前，都通过平台层读取该 PID 当前的可执行文件：只有文件名等于平台的 emulator 可执行文件名，或以 `qemu-system-` 开头时，才视为 emulator。macOS 使用 `proc_pidpath`，Linux 读取 `/proc/<pid>/exe`；其他平台返回 `platform_not_supported`，而不是假定进程不存在。

## 开机状态

`boot_status()` 没有找到实例时返回 `offline`；adb 尚未进入 `device` 状态或系统服务尚未就绪时返回 `booting`。

`ready` 同时要求：

- `getprop sys.boot_completed` 输出 `1`
- `pm path android` 成功并返回 Android framework 包路径

开机期间的 offline、暂时 not found、命令失败或服务尚未发布都视为未就绪，而不是立即把启动操作判为工具错误。

## 脱离启动

默认启动直接执行 `emulator -avd <id>`。平台层让 emulator 成为新会话首进程，因此创建它的 CLI、Rust future 或宿主应用退出后，模拟器不会因继承父进程组而被终止。

stdin 连接空设备，stdout 与 stderr 分别写入库数据目录下按 AVD ID 分隔的日志文件，不创建无人读取的管道。启动操作依次等待：

1. 存活的发现文件，预设超时 15 秒
2. adb 中对应的 serial，预设超时 60 秒
3. 两项开机条件，预设总超时 300 秒

目标已经运行时不启动第二个进程，而是按同样的开机条件等待现有实例就绪，预设超时同为 300 秒；等待期间实例消失则返回 `launch_failed`。因此 `DeviceStarted` 始终表示 Android 已就绪，与实例是否由本次调用启动无关。

本次启动的 emulator 在等待期间提前退出时，返回 `launch_failed`，日志和退出状态进入 `Diagnostic`。超时或取消会强制清理本次启动的进程组；已在运行的实例不属于本次操作，不会被清理。

自定义启动参数返回 `capability_unavailable`，原因是 `not_implemented`。

## 停止与强制终止

有 adb 时，停止流程对目标实例执行 `adb -s <serial> emu kill`，随后同时等待 serial 从 adb 消失以及发现文件中的 emulator 进程退出。两项都满足才报告成功。PID 已被无关进程复用时视为 emulator 已退出，也不会收到信号。

优雅停止十秒后仍未完成时：

1. 发送 `Warning`，说明强制停止可能损坏快照
2. 向模拟器进程组发送 SIGTERM，再等待五秒
3. 仍未退出则发送 SIGKILL，再等待五秒

adb 不可用但 Android CLI 可用时，固定路由回退到 `android emulator stop <id>`，仍用存活发现文件中的 PID 确认退出并在需要时升级信号。已经停止的目标按幂等成功处理。

`runtime_stop_all` 当前返回 `not_implemented`。

## 取消与平台边界

启动、开机探测、停止命令和轮询使用同一个 `CancellationToken`。查询命令有短超时，可能挂起的 `emu avd name` 不会阻塞整个发现流程。

新会话建立、进程身份查询和进程组信号只在 `platform` crate 中实现。未实现的平台上，这些原语返回“不支持”错误并映射为 `platform_not_supported`，不会静默报告成功。进程执行细节见[进程执行](../internals/process.md)。
