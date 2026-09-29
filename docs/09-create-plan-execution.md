# 创建计划执行

创建 AVD 是由工具调用和文件事务组成的组合操作。公开 `Plan` 负责展示与审批；`core` 在执行前验证计划没有被修改，再从计划保留的领域意图编译私有执行步骤。工具参数和临时路径不进入公共契约。

## 自包含计划

创建计划包含稳定 ID、计划类型、有序步骤、补偿说明和 `PlanIntent`。意图保留完整的 `CreateDeviceDraft`，因此计划序列化、跨出口审批后仍可执行，不依赖进程内注册表。

`execute_plan()` 会重新编译意图，并逐字段比较公开计划。步骤、描述或计划 ID 被改写时返回 `invalid_input`，不会按一份与调用方审批内容不同的计划执行。P0 仍只接受默认硬件配置。

## 一次性预检

执行在创建 AVD 前固定使用 `Kit` 持有的环境快照，并检查：

- Android CLI、emulator 和 adb 都处于可用状态；
- emulator、platform-tools 和目标系统镜像已安装；
- 镜像目录仍存在，且镜像 ID 包含 API、tag 和 ABI；
- 目标 ID 与 Android CLI 临时使用的档位 ID 都没有索引或目录冲突；
- `adb devices -l` 中没有属于目标 ID 的运行实例。

运行实例名称通过 `adb shell getprop ro.boot.qemu.avd_name` 确认。无法辨认的离线 emulator 会使预检失败，而不是在状态未知时继续写文件。预检不会安装包、接受许可或调用 Android CLI 的创建命令。

## 锁与执行顺序

同一进程先按目标 ID 和档位 ID 取得写锁。预检通过后，在 AVD 目录中以原子 `create_new` 建立由产品名派生的目录锁，并再次检查名称冲突，避免两个协作进程同时占用同一档位或目标。

执行步骤固定为：

1. `android --sdk=<snapshot> emulator create <profile>`；
2. 将 `<profile>.avd` 和 `<profile>.ini` 移到目标 ID；
3. 改写索引中的 `path`、`path.rel`、`target`，以及配置中的 `AvdId`、镜像、tag、Play Store 和 target；
4. 改写显示名。

每个工具调用都显式注入快照中的 SDK、Android 用户目录和 AVD 目录。Android CLI 的退出码不单独代表成功；已知错误文本和非零退出码仍由驱动映射为稳定错误。

提交事务前会通过 AVD 文件读取路径重新读取目标设备，并核对目录、档位、镜像、target 和显示名。工具表面成功但没有产生预期状态时仍视为执行失败并进入补偿。

## 文件事务

Android CLI 创建的索引和 `config.ini` 在第一次改写前复制到库数据目录的操作备份目录。INI 修改只替换目标键，保留未知行、原有顺序和值中额外的 `=`。

文件更新先在目标文件的同目录创建临时文件、写入并同步，再用重命名原子替换。目录与索引移动无法组成单个文件系统原子操作，因此任一步失败后由补偿恢复备份并删除本次创建的索引与目录。成功后删除本次操作的备份；P0 不保留持久计划日志。

## 事件、取消与补偿

`Operation` 为每个正向步骤发送 `StepStarted` 和 `StepFinished`，并把工具 stdout、stderr 转换为 `Log`。同一个 `CancellationToken` 传给计划执行器、驱动和 `Runner`；取消或超时会终止 Android CLI 进程组。

文件改写是短小的原子单元，令牌在每个单元开始前检查。创建命令完成后出现任何失败或取消，执行器都会发送 `CompensationStarted`、`CompensationFinished`，并删除新 AVD、清理备份。

最终 `Error.failed_step` 保留原始失败步骤，`Error.compensations` 单独记录每项补偿是否成功及其消息。补偿失败不会覆盖原始错误码。
