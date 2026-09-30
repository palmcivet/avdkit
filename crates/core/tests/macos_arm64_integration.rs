use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use avdfs as _;
use avdkit::{
    test_avd_prefix, AvdId, BootStatus, CreateDeviceDraft, ErrorCode, HardwareConfig, Kit,
    KitConfig, Operation, OperationResult, PackageKind, ProfileId, StartOptions,
};
use drivers as _;
use environment as _;
use model as _;
use platform as _;
use process as _;
use serde_json as _;

const ENABLE: &str = "AVDKIT_INTEGRATION";

struct IsolatedAndroidHome {
    root: PathBuf,
    id: AvdId,
    sdk_root: PathBuf,
    previous_user_home: Option<String>,
    previous_avd_home: Option<String>,
    previous_sdk_root: Option<String>,
}

impl IsolatedAndroidHome {
    fn install(id: AvdId, sdk_root: PathBuf) -> Self {
        let root = env::temp_dir().join(format!(
            "{}integration_{}",
            test_avd_prefix(),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("user/avd")).unwrap();
        let previous_user_home = env::var("ANDROID_USER_HOME").ok();
        let cli_root = previous_user_home
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(&env::var("HOME").expect("HOME is required")).join(".android")
            })
            .join("cli");
        let status = Command::new("ln")
            .args(["-s"])
            .arg(&cli_root)
            .arg(root.join("user/cli"))
            .status()
            .expect("macOS ln is required for Android CLI state isolation");
        assert!(status.success(), "could not link Android CLI state");
        let previous_avd_home = env::var("ANDROID_AVD_HOME").ok();
        let previous_sdk_root = env::var("ANDROID_SDK_ROOT").ok();
        env::set_var("ANDROID_USER_HOME", root.join("user"));
        env::set_var("ANDROID_AVD_HOME", root.join("user/avd"));
        env::set_var("ANDROID_SDK_ROOT", &sdk_root);
        Self {
            root,
            id,
            sdk_root,
            previous_user_home,
            previous_avd_home,
            previous_sdk_root,
        }
    }

    fn restore(name: &str, value: &Option<String>) {
        match value {
            Some(value) => env::set_var(name, value),
            None => env::remove_var(name),
        }
    }
}

impl Drop for IsolatedAndroidHome {
    fn drop(&mut self) {
        let adb = self.sdk_root.join("platform-tools/adb");
        let mut matching_serials = Vec::new();
        if let Ok(output) = Command::new(&adb).args(["devices"]).output() {
            for serial in String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.split_whitespace().next())
                .filter(|serial| serial.starts_with("emulator-"))
            {
                let name = Command::new(&adb)
                    .args(["-s", serial, "shell", "getprop", "ro.boot.qemu.avd_name"])
                    .output()
                    .ok()
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
                if name.as_deref() == Some(self.id.as_str()) {
                    matching_serials.push(serial.to_owned());
                    let _ = Command::new(&adb)
                        .args(["-s", serial, "emu", "kill"])
                        .output();
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let visible = Command::new(&adb)
                .args(["devices"])
                .output()
                .ok()
                .is_some_and(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter_map(|line| line.split_whitespace().next())
                        .any(|serial| matching_serials.iter().any(|value| value == serial))
                });
            if !visible {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = fs::remove_dir_all(&self.root);
        Self::restore("ANDROID_USER_HOME", &self.previous_user_home);
        Self::restore("ANDROID_AVD_HOME", &self.previous_avd_home);
        Self::restore("ANDROID_SDK_ROOT", &self.previous_sdk_root);
    }
}

async fn finish(operation: Operation) -> Result<OperationResult, Box<avdkit::Error>> {
    while operation.next_event().await.is_some() {}
    operation.result().await.map_err(Box::new)
}

fn sdk_root() -> PathBuf {
    env::var_os("ANDROID_SDK_ROOT")
        .or_else(|| env::var_os("ANDROID_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(&env::var("HOME").expect("HOME is required")).join("Library/Android/sdk")
        })
}

#[tokio::test]
async fn create_start_stop_delete_lifecycle() {
    if env::var(ENABLE).as_deref() != Ok("1") {
        return;
    }

    let id = AvdId::new(format!(
        "{}integration_{}",
        test_avd_prefix(),
        std::process::id()
    ))
    .unwrap();
    let isolated = IsolatedAndroidHome::install(id.clone(), sdk_root());
    let original_home = env::var("HOME").expect("HOME is required");
    env::set_var("HOME", isolated.root.join("failure-home"));
    let failure_kit = Kit::new_async(KitConfig::default()).await.unwrap();
    env::set_var("HOME", &original_home);
    let report = failure_kit.environment().await.unwrap();
    assert!(report.snapshot.host.supported, "requires macOS arm64");
    let abi = report
        .snapshot
        .host
        .android_abi
        .as_value()
        .expect("host Android ABI is required");
    let image = report
        .snapshot
        .installed_packages
        .iter()
        .find(|package| {
            package.installed
                && package.id.kind == PackageKind::SystemImage
                && package.id.abi.as_deref() == Some(abi)
        })
        .expect("install a host-compatible system image before running integration tests")
        .id
        .clone();
    let profile = ProfileId::new("medium_phone").unwrap();
    assert!(failure_kit
        .profiles()
        .await
        .unwrap()
        .iter()
        .any(|candidate| candidate.id == profile));

    let failure_id = AvdId::new(format!(
        "{}compensation_{}",
        test_avd_prefix(),
        std::process::id()
    ))
    .unwrap();
    fs::create_dir_all(report.snapshot.paths.data_root.parent().unwrap()).unwrap();
    fs::write(&report.snapshot.paths.data_root, "block backup directory").unwrap();
    let failure_plan = failure_kit
        .plan_create(CreateDeviceDraft {
            id: failure_id.clone(),
            profile: profile.clone(),
            image: image.clone(),
            display_name: None,
            hardware: HardwareConfig::default(),
        })
        .unwrap();
    let error = finish(failure_kit.execute_plan(failure_plan))
        .await
        .unwrap_err();
    assert_eq!(error.failed_step.as_deref(), Some("rename"));
    assert_eq!(error.compensations.len(), 1);
    assert!(error.compensations[0].succeeded);
    let avd_root = &report.snapshot.paths.avd_root;
    assert!(!avd_root.join("medium_phone.ini").exists());
    assert!(!avd_root.join("medium_phone.avd").exists());
    assert!(!avd_root.join(format!("{failure_id}.ini")).exists());
    assert!(!avd_root.join(format!("{failure_id}.avd")).exists());
    fs::remove_file(&report.snapshot.paths.data_root).unwrap();

    let kit = Kit::new_async(KitConfig::default()).await.unwrap();

    let plan = kit
        .plan_create(CreateDeviceDraft {
            id: id.clone(),
            profile,
            image,
            display_name: Some("avdkit integration test".into()),
            hardware: HardwareConfig::default(),
        })
        .unwrap();
    assert!(matches!(
        finish(kit.execute_plan(plan)).await.unwrap(),
        OperationResult::PlanCompleted { .. }
    ));
    assert_eq!(kit.get_device(&id).await.unwrap().id, id);
    assert_eq!(kit.boot_status(&id).await.unwrap(), BootStatus::Offline);

    assert!(matches!(
        finish(kit.start(id.clone(), StartOptions::default()))
            .await
            .unwrap(),
        OperationResult::DeviceStarted { .. }
    ));
    assert_eq!(kit.boot_status(&id).await.unwrap(), BootStatus::Ready);
    assert!(matches!(
        finish(kit.start(id.clone(), StartOptions::default()))
            .await
            .unwrap(),
        OperationResult::DeviceStarted { .. }
    ));

    assert_eq!(
        finish(kit.stop(id.clone())).await.unwrap(),
        OperationResult::DeviceStopped { id: id.clone() }
    );
    assert_eq!(kit.boot_status(&id).await.unwrap(), BootStatus::Offline);
    assert_eq!(
        finish(kit.stop(id.clone())).await.unwrap(),
        OperationResult::DeviceStopped { id: id.clone() }
    );

    assert_eq!(
        finish(kit.delete_device(id.clone())).await.unwrap(),
        OperationResult::DeviceDeleted { id: id.clone() }
    );
    assert_eq!(
        kit.get_device(&id).await.unwrap_err().code,
        ErrorCode::DeviceNotFound
    );
    drop(isolated);
}
