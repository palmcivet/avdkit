# 设备与机型

设备相关的只读入口走两条路径：预设机型调用 Android CLI；设备列表与详情直接读 AVD 文件。两条路径都经过能力检查，并输出同一套 `model` 类型。

有副作用的创建、删除见[创建计划](plans.md)。运行中的实例见[运行时](runtime.md)。

## 预设机型

`devices_profiles` 使用 Android CLI。当前实现要求主机为 macOS arm64，且环境探测发现 Android CLI。`KitConfig` 使用当前支持的默认策略。

满足条件时，能力矩阵包含：

```json
{
  "id": "devices_profiles",
  "state": {
    "state": "available",
    "implementation": "android_cli"
  }
}
```

Android CLI 缺失时，该能力为 `unavailable`，原因包含 `tool_not_found`，不包含 `not_implemented`。这表示功能已经实现，只是当前环境缺依赖。

```rust
use avdkit::{Kit, KitConfig};

let kit = Kit::new_async(KitConfig::default()).await?;
let profiles = kit.profiles().await?;

for profile in profiles {
    println!("{}", profile.id);
}
```

命令行：

```sh
avdkit devices profiles
avdkit --json devices profiles
```

Android CLI 返回六个固定档位：`large_desktop`、`medium_desktop`、`medium_phone`、`medium_tablet`、`small_desktop`、`small_phone`。这不是 `avdmanager list device` 的完整设备目录。

这条命令只提供 ID。Android CLI 没有可靠显示名称，因此 `display_name` 为 `unavailable`。avdkit 不把 `medium_phone` 猜成 `Medium Phone`。

内部路径：

```text
Kit.profiles
  → 能力：devices_profiles
  → 环境快照中的 Android CLI 与 SDK
  → android --sdk=<SDK> emulator create --list-profiles
  → 驱动解析 ID
  → Profile { id, display_name: unavailable }
```

查询期间不会重新读取 PATH 或环境变量。进程执行层把 stdin 接到空设备，捕获输出，应用默认超时，并在 future 被丢弃或超时时清理进程组。这是短查询，没有公开 `Operation`。

解析器先去除公共噪声，再要求退出状态为 0、至少有一个输出行、每一行都是不含空白的单个机型 ID。已知工具错误映射为稳定错误码；其他格式返回 `tool_output_unrecognized`，并在 `diagnostic` 中保留原始输出。

常见失败：

- `capability_unavailable`：平台或 Android CLI 不可用
- `tool_not_found`：快照中的可执行文件在执行前后失效
- `timeout`：Android CLI 没有在预设时间内结束
- `tool_output_unrecognized`：工具版本输出了尚未覆盖的格式
- `internal`：启动或等待进程时发生其他 I/O 错误

对于 `tool_output_unrecognized`，应把 `diagnostic` 中的 stdout、stderr 和退出状态连同本次已知调用参数录制为新的脱敏 fixture。驱动契约见[工具驱动](../internals/drivers.md)。

## 设备列表与详情

```rust
let devices = kit.list_devices().await?;
let device = kit.get_device(&id).await?;
```

```sh
avdkit devices list
avdkit devices get <id>
```

两个命令都支持全局 `--json`。

`android emulator list --long` 使用按字符数补齐的文本表格。AVD 显示名称包含 CJK 字符时，显示宽度和字符数不同，后续列会错位。因此设备列表和详情的主数据源是 AVD 文件：

```text
<ANDROID_AVD_HOME>/
  <id>.ini

<id>.ini 中的 path 指向：
  <实际 AVD 目录>/
    config.ini
```

`emulator -list-avds` 解析器仍然存在，但不负责提供详情。

`devices_list` 和 `devices_get` 的实现名为 `avd_files`。它们只要求当前平台受支持，不要求 Android CLI、emulator 或 adb 存在。AVD 根目录不存在时，列表返回空数组。这代表用户还没有 AVD，不是能力错误。在未支持的平台上，能力为 `unavailable`，原因包含 `platform_not_supported`，但不包含 `not_implemented`。

## 文件定位

`AvdStore` 从环境快照中的 `avd_root` 开始扫描，只处理顶层 `.ini` 文件。设备 ID 来自文件名，而不是 `config.ini` 中的 `AvdId`：

```text
avdkit_test_phone.ini → id = avdkit_test_phone
```

读取实际 AVD 目录时：

1. 优先使用 `<id>.ini` 中的绝对 `path=`
2. 缺少 `path` 时，使用相对于 Android 用户目录的 `path.rel=`
3. 不假设目录一定叫 `<id>.avd`，也不假设它与索引文件相邻

这个规则支持 Android 工具创建的标准布局，也支持 `avdmanager create -p` 等自定义位置。

## 字段映射

索引文件提供：

- 文件名 → `Device.id`
- `target` → `Device.target`
- `path` / `path.rel` → `config.ini` 的位置

`config.ini` 提供：

- `avd.ini.displayname` → `Device.display_name`
- `hw.device.name` → `Device.profile`
- `image.sysdir.1` → `Device.image`
- `target` → 索引文件缺少 target 时的备选值

系统镜像路径 `system-images/android-36/google_apis/arm64-v8a/` 会转换为结构化 `PackageId`，而不是作为不透明字符串返回。

## 缺失字段和损坏文件

- AVD 根目录不存在：设备列表为空
- 指定 ID 的索引不存在：`device_not_found`
- 指定 ID 的索引路径存在但不是普通文件：`precondition_failed`
- 索引既没有 `path` 也没有 `path.rel`：`precondition_failed`
- `config.ini` 不存在：设备仍返回，但配置字段为 `unavailable`
- 文件存在但无法读取：`internal`，消息包含失败路径和 I/O 原因
- 单个配置键不存在或无法解析：对应公共字段为 `unavailable`

列表会忽略文件名不能构成合法 AVD ID 的 `.ini`。合法 ID 的索引一旦无法读取或缺少路径，整个列表查询返回相应错误，不返回不完整的部分结果。列表按 AVD ID 排序，因此文件系统枚举顺序不会影响 Rust 或 JSON 输出。

完整设备的 JSON 形态：

```json
{
  "id": "avdkit_test_phone",
  "display_name": {
    "state": "present",
    "value": "Test Phone"
  },
  "profile": {
    "state": "present",
    "value": "medium_phone"
  },
  "image": {
    "state": "present",
    "value": {
      "kind": "system_image",
      "api": "36",
      "tag": "google_apis",
      "abi": "arm64-v8a",
      "qualifier": null
    }
  },
  "target": {
    "state": "present",
    "value": "android-36"
  }
}
```

缺少 `config.ini` 时不会伪造默认机型或镜像。相应字段明确使用 `{ "state": "unavailable" }`。

这条调用链不经过 `drivers` 或 `process`：

```text
Kit.list_devices / Kit.get_device
  → 能力矩阵：avd_files
  → 环境快照：avd_root
  → avdfs::AvdStore
  → IniDocument
  → avdfs::AvdMetadata
  → core 映射为 model::Device
```

`avdfs` 返回文件层元数据，`core` 决定如何使用 `Field<T>` 表达缺失值。
