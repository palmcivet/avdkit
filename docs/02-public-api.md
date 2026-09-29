# 公共 API

[上一章](01-policy.md)解释了能力、策略和预检。本章从调用方视角把它们组合起来：如何创建 `Kit`、判断功能是否可用、发起操作并处理稳定结果。

## `Kit` 是唯一领域入口

Rust 调用方只需要依赖包 `avdkit`。`Kit` 持有配置和当前环境快照，并提供所有领域操作。

最小调用流程：

```rust
use avdkit::{Kit, KitConfig};

let kit = Kit::new_async(KitConfig::default()).await?;

let environment = kit.environment().await?;
let capabilities = kit.capabilities().await?;
let profiles = kit.profiles().await?;
```

创建 `Kit` 时会：

1. 校验整份 `KitConfig`；
2. 合并进程与平台登录环境；
3. 探测主机、路径、工具版本和已安装 SDK 包；
4. 生成不可变环境快照；
5. 由固定领域路由从快照生成能力矩阵。

`Kit` 实现 `Send + Sync`，可以放入 `Arc` 后由多个异步任务共享。查询返回的数据是快照副本，后续刷新不会修改调用方已经持有的值。

异步应用使用 `new_async()`，环境探测会进入 Tokio blocking pool。没有异步运行时的调用方也可以使用同步的 `Kit::new()`；该方法会在当前线程完成文件系统探测。

## 环境快照

`environment()` 返回 `EnvironmentReport`，其中包含：

- `snapshot.host`：平台、CPU 架构以及当前是否受支持；
- `snapshot.host.android_abi`：运行时探测的 Android ABI；
- `snapshot.paths` 与 `sdk_root_source`：SDK、Android 用户目录、AVD 目录、运行发现目录、库数据目录以及 SDK 选择来源；
- `snapshot.environment`：参与路径选择的环境值及其来源；
- `snapshot.tools`：每个官方工具的路径、版本、状态和包来源；
- `snapshot.installed_packages`：从 `source.properties` 扫描的工具包和系统镜像；
- `snapshot.diagnostics`：不阻止探测完成的来源冲突和探针失败；
- `capabilities`：基于这份快照推导的能力矩阵。

环境变量、PATH 或 SDK 内容变化后，调用：

```rust
let new_report = kit.refresh().await?;
```

`refresh()` 会完整重建快照和能力矩阵，不做局部修改。一次领域操作因此始终基于一份一致的环境描述。

## 先查能力，再展示或调用操作

`capabilities()` 返回所有已声明能力，而不只是当前可用项。每项状态是：

- `available`：带实现名，例如 `android_cli`；
- `unavailable`：带一个或多个结构化原因和可选补救方式。

调用方可以直接根据原因码决定 UI 或自动化行为：

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

即使调用方没有预先查询，具体方法也会再次检查能力。因此能力查询适合产品展示和调度，但不是安全检查的替代品。

## 三种操作形态

avdkit 根据操作的时间和副作用采用三种 API 形态。

### 查询：直接返回数据

短查询是异步方法，返回 `Result<T, Error>`：

```rust
let profiles = kit.profiles().await?;
for profile in profiles {
    println!("{}", profile.id);
}
```

环境、能力、SDK 包、设备、机型、运行实例和开机状态都采用这种形态。运行实例组合 adb 与存活发现文件，开机就绪同时验证系统属性和包管理器。

### 长任务：返回 `Operation`

安装、删除、启动和停止可能持续数秒到数分钟，因此调用方法立即返回 `Operation`。调用方可以消费事件、发出取消并等待唯一的最终结果：

```rust
let mut operation = kit.install(package_id);

while let Some(event) = operation.next_event().await {
    println!("{event:?}");
}

let result = operation.result().await?;
```

`cancel()` 是幂等的：

```rust
operation.cancel();
```

`Operation`、计划执行器、运行时执行器、驱动和进程执行层使用同一种取消令牌。创建和模拟器生命周期操作的取消会沿这条链路终止对应进程组，最终结果使用 `cancelled`。

启动会在新会话中运行 emulator，等待发现、adb 和开机就绪；停止先使用 adb，再按超时升级到进程组信号。安装和删除入口尚未接入真实驱动，调用时仍返回 `capability_unavailable`。

### 组合操作：先生成 `Plan`

创建设备需要安装检查、创建临时 AVD、改写配置和移动目录。此类意图先编译为 `Plan`：

```rust
let plan = kit.plan_create(draft)?;
```

`Plan` 是可序列化数据，包含：

- 计划类型和稳定 ID；
- 可序列化的领域意图；
- 有序步骤；
- 每步的类型与描述；
- 可以执行的补偿动作。

调用方可以在产生副作用前展示、记录或审批计划，再用 `execute_plan` 启动执行。执行器会从领域意图重新编译并验证公开步骤未被修改，然后完成预检、Android CLI 创建和文件事务。自定义硬件配置会明确返回 `capability_unavailable`，而不是被忽略。

创建计划目前只在受支持的平台上开放；它不要求 Android 工具已经安装。未支持平台的 `devices_plan_create` 能力包含 `platform_not_supported`。

## 可选字段不是 `null`

不同工具能提供的信息不同。公共模型使用 `Field<T>` 表达三种情况：

- `present`：字段有可靠值；
- `unavailable`：当前实现或工具没有提供；
- `inapplicable`：这个字段对当前对象或状态没有意义。

例如，Android CLI 的预设机型命令只返回 ID，不返回显示名称：

```json
{
  "id": "medium_phone",
  "display_name": {
    "state": "unavailable"
  }
}
```

调用方不需要猜测 `null`、空字符串或默认值分别代表什么。

## 错误契约

所有出口共享 `Error`。重要字段包括：

- `code`：调用方应依赖的稳定错误码；
- `message`：用于展示或日志；
- `reasons`：能力不可用的结构化原因；
- `failed_step`：组合计划失败的位置；
- `compensations`：原始失败后各项补偿的成功状态和消息；
- `diagnostic`：可选的底层命令、stdout、stderr 和退出状态。

常见错误码：

- `capability_unavailable`：平台、工具或实现不可用；
- `invalid_input`：公开模型或参数无效；
- `device_not_found`、`package_not_found`：请求的对象不存在；
- `precondition_failed`、`name_conflict`：本次请求不满足条件；
- `tool_output_unrecognized`：工具运行了，但输出不符合已知契约；
- `timeout`、`cancelled`：执行没有正常完成；
- `internal`：库内部或底层 I/O 失败。

底层工具文本只进入诊断，不作为公共控制流。调用方应匹配 `code` 和 `reasons`。

## JSON 与其他出口

公开模型都可以序列化，字段统一使用 `snake_case`。CLI 的完整 JSON 响应使用：

```json
{
  "schema_version": "0.1.0",
  "data": {}
}
```

`Envelope<T>` 的版本属于数据契约，不是 Android 工具版本。Rust API、CLI JSON 和 UniFFI 边界共用同一套 `model` 类型，因此不同出口不会重新定义字段和错误。

下一章开始进入内部实现，先看所有工具驱动共同依赖的[进程执行层](03-process-execution.md)。
