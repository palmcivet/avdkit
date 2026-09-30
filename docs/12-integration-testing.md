# macOS arm64 集成测试

真实 Android 工具链测试默认不运行。设置 `AVDKIT_INTEGRATION=1` 后，`avdkit` crate 的集成测试会使用本机 Android CLI、emulator、adb 和一个已经安装的、与主机 ABI 匹配的系统镜像：

```sh
AVDKIT_INTEGRATION=1 cargo test -p avdkit --test macos_arm64_integration -- --nocapture
```

测试为 `ANDROID_USER_HOME` 和 `ANDROID_AVD_HOME` 创建独立临时目录，不读写调用方已有 AVD。Android CLI 当前忽略单独设置的 `ANDROID_AVD_HOME`，因此测试必须同时隔离用户目录；隔离目录只链接原用户目录中的 CLI bundle 与首次运行状态，避免重复下载或再次显示条款。设备 ID 和临时目录均使用由品牌常量派生的测试前缀。

## 生命周期覆盖

一次测试顺序验证：

1. 查询 Android CLI 预设机型和已安装系统镜像；
2. 阻断事务备份目录，确认真实 Android CLI 已创建临时 AVD 后的文件事务失败会执行补偿且不留残余；
3. 编译并执行正常创建计划；
4. 通过文件入口重新读取设备；
5. 启动模拟器并等待 adb 与 Android 服务就绪；
6. 重复启动，验证幂等返回已有实例；
7. 停止模拟器并确认离线；
8. 重复停止，验证幂等成功；
9. 删除 AVD 并确认设备查询返回 `device_not_found`。

测试作用域退出时还会按 AVD ID 查找并停止残留 emulator，再删除隔离目录。CLI 的 `devices cleanup-tests --approve` 可清扫默认 Android 用户目录中所有带测试前缀的残留 AVD；它会先幂等停止，再删除每个设备。单个设备可用 `devices delete <id> --approve` 删除。

## 前置条件

测试只消费已经安装的系统镜像，不隐式下载、不接受许可协议。没有兼容镜像时测试会明确失败并提示先安装镜像，避免常规集成测试意外产生数 GB 网络流量。
