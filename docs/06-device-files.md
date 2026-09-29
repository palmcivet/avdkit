# 第二个纵向切片：设备文件查询

[上一章](05-read-only-queries.md)通过 Android CLI 查询预设机型。本章实现另一类只读操作：不启动官方工具，直接从 AVD 文件读取设备列表与详情。

这条路径对应公开入口：

```rust
let devices = kit.list_devices().await?;
let device = kit.get_device(&id).await?;
```

以及命令行：

```sh
avdkit devices list
avdkit devices get <id>
```

两个命令都支持全局 `--json`。

## 为什么直接读文件

`android emulator list --long` 使用按字符数补齐的文本表格。AVD 显示名称包含 CJK 字符时，显示宽度和字符数不同，后续列会错位。

AVD 自身已经保存了更稳定的数据：

```text
<ANDROID_AVD_HOME>/
  <id>.ini

<id>.ini 中的 path 指向：
  <实际 AVD 目录>/
    config.ini
```

因此设备文件是列表和详情的主数据源。`emulator -list-avds` 解析器仍然存在，但不负责提供详情。

## 能力

`devices_list` 和 `devices_get` 的实现名为 `avd_files`。它们只要求当前平台受支持，不要求 Android CLI、emulator 或 adb 存在。

AVD 根目录不存在时，列表返回空数组。这代表用户还没有 AVD，不是能力错误。

在未支持的平台上，能力为 `unavailable`，原因包含 `platform_not_supported`，但不包含 `not_implemented`。

## 文件定位

`AvdStore` 从环境快照中的 `avd_root` 开始扫描，只处理顶层 `.ini` 文件。

设备 ID 来自文件名，而不是 `config.ini` 中的 `AvdId`：

```text
avdkit_test_phone.ini → id = avdkit_test_phone
```

读取实际 AVD 目录时：

1. 优先使用 `<id>.ini` 中的绝对 `path=`；
2. 缺少 `path` 时，使用相对于 Android 用户目录的 `path.rel=`；
3. 不假设目录一定叫 `<id>.avd`，也不假设它与索引文件相邻。

这个规则支持 Android 工具创建的标准布局，也支持 `avdmanager create -p` 等自定义位置。

## 字段映射

索引文件提供：

- 文件名 → `Device.id`；
- `target` → `Device.target`；
- `path` / `path.rel` → `config.ini` 的位置。

`config.ini` 提供：

- `avd.ini.displayname` → `Device.display_name`；
- `hw.device.name` → `Device.profile`；
- `image.sysdir.1` → `Device.image`；
- `target` → 索引文件缺少 target 时的备选值。

系统镜像路径：

```text
system-images/android-36/google_apis/arm64-v8a/
```

会转换为结构化 `PackageId`，而不是作为不透明字符串返回。

## 缺失字段和损坏文件

文件状态被区分处理：

- AVD 根目录不存在：设备列表为空；
- 指定 ID 的索引不存在：`device_not_found`；
- 索引既没有 `path` 也没有 `path.rel`：`precondition_failed`；
- `config.ini` 不存在：设备仍返回，但配置字段为 `unavailable`；
- 文件存在但无法读取：`internal`，消息包含失败路径和 I/O 原因；
- 单个配置键不存在或无法解析：对应公共字段为 `unavailable`。

列表按 AVD ID 排序，因此文件系统枚举顺序不会影响 Rust 或 JSON 输出。

## 公共结果

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

缺少 `config.ini` 时不会伪造默认机型或镜像。相应字段明确使用：

```json
{ "state": "unavailable" }
```

## 分层对应

这条调用链不经过 `drivers` 或 `process`：

```text
Kit.list_devices / Kit.get_device
  → 能力矩阵：avd_files
  → 环境快照：avd_root
  → avdfs::AvdStore
  → IniDocument
  → avdfs::AvdMetadata
  → core 映射为 model::Device
  → Rust / CLI JSON
```

`avdfs` 返回文件层元数据，`core` 决定如何使用 `Field<T>` 表达缺失值。这样文件解析层不需要知道 CLI 或语言绑定。

## 测试边界

单元测试只在临时目录中创建带 `test_avd_prefix()` 派生前缀的 AVD 索引和配置。测试覆盖：

- `path` 指向非相邻目录；
- 显示名称、机型、镜像和 target 映射；
- 稳定排序；
- 空目录；
- 指定设备不存在；
- 索引缺少路径。

测试结束后删除临时目录，不读取或修改用户现有 AVD。
