# 进程执行

查询和长任务最终都会调用 Android 官方进程。`drivers` 只描述“要执行什么”，`process` 统一保证“怎样安全执行”。工具输出代表成功还是失败，仍由驱动判断。

如果每个驱动直接使用 `tokio::process::Command`，很快会出现不同实现：有的命令忘记超时，有的只杀直接子进程，有的继承意外的 stdin，有的丢失 stderr，Swift Task 与 `Operation` 的取消也无法衔接。

## 一次调用的生命周期

普通工具调用经历：

```text
CommandSpec
  → 检查是否已经取消
  → 创建独立进程组
  → 写入固定 stdin 或连接空设备
  → 同时等待进程、超时或取消
  → 捕获 stdout / stderr / 终止状态
  → 正常退出时解除进程组保护
```

超时、取消或调用 future 被丢弃时，进程组保护器负责终止本次调用拥有的整个进程组。

## `CommandSpec`

`CommandSpec` 是驱动和执行器之间的中性描述：程序路径、独立参数、本次覆盖的环境变量、可选工作目录和可选 stdin 字节。执行器不会清空整个继承环境。PATH 等变量仍可使用，但 SDK 和 AVD 相关变量由驱动显式覆盖。参数不通过 shell 拼接。stdin 缺省连接空设备。

## `Output` 只记录事实

命令正常结束后，`Runner` 返回可选数字退出码、原始 stdout 和原始 stderr。被信号终止时可能没有数字退出码。字节按 UTF-8 容错转换，避免工具输出中的局部无效字节使整个调用失败。

`process` 不把“退出码 0”解释为成功，也不把非零退出码直接映射为领域错误。原因是 Android CLI 已实测会在 AVD 不存在、名称冲突和包不存在时仍返回 0。驱动必须验证每条命令自己的成功特征。

## 超时与错误

`Runner` 提供 `run`、`run_with_timeout`、`run_cancellable` 和 `run_cancellable_with_timeout`。

超时返回稳定错误码 `timeout`。可执行文件不存在返回 `tool_not_found`；其他启动和等待 I/O 错误返回 `internal`。这些错误表示进程没有产生可供驱动解释的完整结果。工具已经运行但输出格式未知时，使用的是驱动层的 `tool_output_unrecognized`。

## 从 `Operation.cancel()` 到进程组

`CancellationToken` 可以克隆。任意克隆调用 `cancel()` 后，状态永久保持为已取消；重复取消没有额外副作用；尚未开始的调用不会启动进程；正在等待的调用返回 `cancelled`。

```text
Swift Task / CLI 信号
  → Operation.cancel()
  → CancellationToken
  → Runner
  → ProcessGroup
```

UniFFI 生成的 Swift async 桥接不会自动传播 `Task.cancel()`。Swift 易用层使用 `withTaskCancellationHandler` 调用 `Operation.cancel()`；独立实验结果见 [UniFFI 与 Swift 6](../findings/uniffi-swift.md)，正式绑定还用临时 Android CLI 工具进程重复验证同一链路。

创建计划执行已经用这条链路运行 Android CLI；取消创建会终止工具进程组，并在需要时清理部分创建的 AVD。

## 为什么终止进程组

Android 工具可能再启动 shell、下载进程或 Java 进程。只对直接子进程调用 kill，后代进程可能继续持有 SDK 锁、写入下载缓存、占用管道，或在调用方已经收到取消结果后继续修改磁盘。

Unix 上，`platform` 在启动前为每次普通调用建立独立进程组。正常结束后保护器解除；异常路径向整个组发送终止信号。

平台差异只存在于 `platform` crate。未实现的平台上，脱离启动、进程存在查询、进程身份查询和进程组信号都返回“不支持”错误，由 `process::platform_error` 映射为 `platform_not_supported`，而不是静默成功。普通短命令在这些平台上仍可执行，只是依靠 `kill_on_drop` 终止直接子进程，不对后代进程分组。能力矩阵不会开放这些平台上的领域操作。

## 普通工具调用与 emulator 启动不同

`Runner` 拥有短生命周期命令：查询、安装、删除和控制命令应当随调用结束而结束。

emulator 主进程则要在创建它的 CLI 或应用退出后继续运行。`spawn_detached` 让它成为新会话首进程，把 stdout、stderr 写入文件且不设置 `kill_on_drop`；之后由运行实例发现与停止流程接管，见[运行时](../guide/runtime.md)。

## 当前输出模型的限制

现有 `Runner` 在命令结束后一次性返回完整输出，适合环境探测和短查询。它不提供逐行事件，因此不能用于实现实时下载进度。

即使没有流式 API，stdout 和 stderr 也始终被读取，避免子进程因为管道写满而阻塞。
