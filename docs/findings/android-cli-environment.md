# Android CLI 环境变量行为

实测环境：macOS arm64，Android CLI `1.0.16457483`。

- 只把 `ANDROID_AVD_HOME` 指向临时目录后执行 `android emulator create medium_phone`，设备仍写入 `ANDROID_USER_HOME/avd`；该命令没有使用单独指定的 AVD 目录。
- 同时把 `ANDROID_USER_HOME` 指向临时目录后，创建结果写入该临时用户目录下的 `avd`。
- 全新的 `ANDROID_USER_HOME` 会使 Android CLI wrapper 安装内嵌 bundle，并在 `--version` 的版本号之前输出条款和提示文本。
- `ANDROID_USER_HOME/cli/tos_displayed` 记录条款已经显示；在隔离用户目录中链接已有 `cli` 目录后，`--version` 不再输出条款文本。
