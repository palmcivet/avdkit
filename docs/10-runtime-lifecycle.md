# 模拟器运行时闭环

运行时闭环组合 adb、平台发现文件和 emulator 进程。公开 API 不依赖 Android CLI 的对齐表格，也不把残留锁文件单独当作运行证据。

## 运行实例发现

环境快照中的 `paths.runtime_root` 由平台层提供。macOS arm64 使用 `~/Library/Caches/TemporaryItems/avd/running`，其中 `pid_<pid>.ini` 记录 `avd.id`、`port.serial` 等值。

`running()` 先执行 `adb devices -l`，再按以下顺序确定每个 emulator serial 的 AVD ID：

1. 使用 serial 对应的发现文件，并确认文件名中的 PID 仍存活；
2. 对 adb 状态为 `device` 的实例读取 `ro.boot.qemu.avd_name`；
3. 最后用两秒超时调用 `adb emu avd name`。

失效发现文件被忽略。发现文件提供可靠 PID 时，`RunningInstance.pid` 为 `present`；通过 adb 回退得到名称时 PID 为 `unavailable`。离线实例只有在仍有存活发现文件时才能返回。

## 开机状态

`boot_status()` 没有找到实例时返回 `offline`；adb 尚未进入 `device` 状态或系统服务尚未就绪时返回 `booting`。

`ready` 同时要求：

- `getprop sys.boot_completed` 输出 `1`；
- `pm path android` 成功并返回 Android framework 包路径。

开机期间的 offline、暂时 not found、命令失败或服务尚未发布都视为未就绪，而不是立即把启动操作判为工具错误。

## 脱离启动

默认启动直接执行 `emulator -avd <id>`。平台层让 emulator 成为新会话首进程，因此创建它的 CLI、Rust future 或宿主应用退出后，模拟器不会因继承父进程组而被终止。

stdin 连接空设备，stdout 与 stderr 分别写入库数据目录下按 AVD ID 分隔的日志文件，不创建无人读取的管道。启动操作依次等待：

1. 存活的发现文件，预设超时 15 秒；
2. adb 中对应的 serial，预设超时 60 秒；
3. 两项开机条件，预设总超时 300 秒。

目标已经运行时直接返回现有实例，不再启动第二个进程。等待期间如果 emulator 提前退出，返回 `launch_failed`，日志和退出状态进入 `Diagnostic`。超时或取消会强制清理本次启动的进程组。

## 停止与强制终止

有 adb 时，停止流程对目标实例执行 `adb -s <serial> emu kill`，随后同时等待 serial 从 adb 消失以及发现文件中的 PID 退出。两项都满足才报告成功。

优雅停止十秒后仍未完成时：

1. 发送 `Warning`，说明强制停止可能损坏快照；
2. 向模拟器进程组发送 SIGTERM，再等待五秒；
3. 仍未退出则发送 SIGKILL，再等待五秒。

adb 不可用但 Android CLI 可用时，固定路由回退到 `android emulator stop <id>`，仍用存活发现文件中的 PID确认退出并在需要时升级信号。已经停止的目标按幂等成功处理。

## 取消与平台边界

启动、开机探测、停止命令和轮询使用同一个 `CancellationToken`。查询命令有短超时，可能挂起的 `emu avd name` 不会阻塞整个发现流程。

新会话建立、进程存活检查和进程组信号只在 `platform` crate 中实现。其他平台仍能编译和完成环境探测，但 P0 能力矩阵只在 macOS arm64 开放运行时操作。
