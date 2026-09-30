# 概念

avdkit 在调用官方工具之前先回答三个问题：当前主机有哪些工具和路径、这个操作现在能否执行、调用方是否允许相应副作用。真正执行时，领域操作再选择工具、构造命令、验证输出，并把工具特有结果转换为稳定的公共模型。

配置细节见[策略](policy.md)。分层与 crate 边界见[架构](../explanation/architecture.md)。

## 三个判断

一次有副作用的操作依次经过：

```text
配置合法
  → 静态能力可用
  → 调用方策略允许
  → 本次请求预检通过
  → 执行
```

**能力**来自环境快照，描述静态前提：平台是否受支持、所需工具是否存在、对应实现是否已经接入。例如 `devices_profiles` 需要 macOS arm64 和 Android CLI；工具缺失时原因是 `tool_not_found`。

**策略**来自 `KitConfig.policy`，控制库可以代表调用方产生哪些副作用。策略允许并不等于能力可用：把 `explicit_install` 设为允许，不会凭空安装缺失的工具。

**预检**使用请求参数和实时状态，例如名称是否冲突、AVD 是否正在运行、创建所需镜像是否已经安装。预检失败使用 `precondition_failed`、`name_conflict` 等错误码，不会改写能力矩阵。

## `Kit` 是唯一领域入口

Rust 调用方只依赖包 `avdkit`。`Kit` 持有配置和当前环境快照，并提供所有领域操作。

```rust
use avdkit::{Kit, KitConfig};

let kit = Kit::new_async(KitConfig::default()).await?;

let environment = kit.environment().await?;
let capabilities = kit.capabilities().await?;
let profiles = kit.profiles().await?;
```

创建 `Kit` 时会校验整份 `KitConfig`，合并进程与平台登录环境，探测主机、路径、工具版本和已安装 SDK 包，生成不可变快照，再由固定领域路由推导能力矩阵。

`Kit` 实现 `Send + Sync`，可以放入 `Arc` 后由多个异步任务共享。查询返回的数据是快照副本，后续刷新不会修改调用方已经持有的值。

异步应用使用 `new_async()`，环境探测进入 Tokio blocking pool。没有异步运行时的调用方可以使用同步的 `Kit::new()`；该方法在当前线程完成文件系统探测。

## 环境快照

`environment()` 返回 `EnvironmentReport`，其中包含：

- `snapshot.host`：平台、CPU 架构以及当前是否受支持
- `snapshot.host.android_abi`：运行时探测的 Android ABI
- `snapshot.paths` 与 `sdk_root_source`：SDK、Android 用户目录、AVD 目录、运行发现目录、库数据目录以及 SDK 选择来源
- `snapshot.environment`：参与路径选择的环境值及其来源
- `snapshot.tools`：每个官方工具的路径、版本、状态和包来源
- `snapshot.installed_packages`：从 `source.properties` 扫描的工具包和系统镜像
- `snapshot.diagnostics`：不阻止探测完成的来源冲突和探针失败
- `capabilities`：基于这份快照推导的能力矩阵

环境变量、PATH 或 SDK 内容变化后调用 `kit.refresh().await?`。`refresh()` 完整重建快照和能力矩阵，不做局部修改。一次领域操作始终基于一份一致的环境描述。探测规则见[环境与路由](../explanation/environment.md)。

## 先查能力，再展示或调用

`capabilities()` 返回所有已声明能力，而不只是当前可用项。每项状态是：

- `available`：带实现名，例如 `android_cli`
- `unavailable`：带一个或多个结构化原因和可选补救方式

```rust
use avdkit::{CapabilityId, CapabilityState};

let profile_capability = kit
    .capabilities()
    .await?
    .into_iter()
    .find(|item| item.id == CapabilityId::DevicesProfiles)
    .expect("the matrix contains every declared capability");

match profile_capability.state {
    CapabilityState::Available { implementation } => {
        println!("profiles use {implementation}");
    }
    CapabilityState::Unavailable { reasons } => {
        for reason in reasons {
            println!("{}: {}", reason.code.as_str(), reason.message);
        }
    }
}
```

即使调用方没有预先查询，具体方法也会再次检查能力。能力查询适合产品展示和调度，但不是安全检查的替代品。完整列表见[能力](../reference/capabilities.md)。

## 三种操作形态

avdkit 根据操作的时间和副作用采用三种 API 形态。

### 查询：直接返回数据

短查询是异步方法，返回 `Result<T, Error>`。环境、能力、SDK 包、设备、机型、运行实例和开机状态都采用这种形态。运行实例组合 adb 与存活发现文件，开机就绪同时验证系统属性和包管理器。

### 长任务：返回 `Operation`

安装、删除、启动和停止可能持续数秒到数分钟，因此调用方法立即返回 `Operation`。调用方可以消费事件、发出取消并等待唯一的最终结果。

```rust
let mut operation = kit.delete_device(id);

while let Some(event) = operation.next_event().await {
    println!("{event:?}");
}

let result = operation.result().await?;
```

`cancel()` 是幂等的。`Operation`、计划执行器、运行时执行器、驱动和进程执行层使用同一种取消令牌。创建和模拟器生命周期操作的取消会沿这条链路终止对应进程组，最终结果使用 `cancelled`。

`next_event()` 与 `result()` 使用共享引用，便于 UniFFI 对象安全地跨 Swift Task 使用；最终结果仍只能取得一次。返回 `Operation` 的方法必须在当前 Tokio 运行时内调用，否则操作立即以 `internal` 结束。丢弃 `Operation` 不会取消后台任务，需要停止时显式调用 `cancel()`。能力检查或参数校验在启动前失败的操作不产生事件，事件流直接结束，最终结果是该错误。

启动会在新会话中运行 emulator，等待发现、adb 和开机就绪；目标已在运行时不重复启动，而是等待现有实例就绪。停止先使用 adb，再按超时升级到进程组信号。删除调用 Android CLI，并在完成后确认索引和目录都已消失。SDK 包安装与移除调用返回 `capability_unavailable`，原因是 `not_implemented`。

### 组合操作：先生成 `Plan`

创建设备需要安装检查、创建临时 AVD、改写配置和移动目录。此类意图先编译为可序列化的 `Plan`：

```rust
let plan = kit.plan_create(draft)?;
```

调用方可以在产生副作用前展示、记录或审批计划，再用 `execute_plan` 启动执行。`plan_create` 是纯编译，不依赖平台和工具；`execute_plan` 才检查 `devices_plan_create` 能力。执行细节见[创建计划](plans.md)。

## 可选字段不是 `null`

不同工具能提供的信息不同。公共模型使用 `Field<T>` 表达三种情况：

- `present`：字段有可靠值
- `unavailable`：当前实现或工具没有提供
- `inapplicable`：这个字段对当前对象或状态没有意义

例如，Android CLI 的预设机型命令只返回 ID，不返回显示名称：

```json
{
  "id": "medium_phone",
  "display_name": {
    "state": "unavailable"
  }
}
```

调用方不需要猜测 `null`、空字符串或默认值分别代表什么。错误码与 JSON 信封见[错误](../reference/errors.md)。
