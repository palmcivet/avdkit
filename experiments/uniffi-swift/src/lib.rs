use std::{process::Stdio, sync::Arc};

use tokio::process::Command;
use tokio::sync::watch;

uniffi::setup_scaffolding!();

struct ProcessGroup {
    id: i32,
    armed: bool,
}

impl ProcessGroup {
    fn new(id: u32) -> Self {
        Self {
            id: id as i32,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        if self.armed {
            // A negative PID addresses the whole process group. The child is
            // placed in a fresh group before exec, so this cannot target us.
            unsafe {
                libc::kill(-self.id, libc::SIGKILL);
            }
        }
    }
}

#[derive(uniffi::Object)]
pub struct CancellationProbe {
    marker_path: String,
    cancellation: watch::Sender<bool>,
}

#[uniffi::export(async_runtime = "tokio")]
impl CancellationProbe {
    #[uniffi::constructor]
    pub fn new(marker_path: String) -> Arc<Self> {
        let (cancellation, _) = watch::channel(false);
        Arc::new(Self {
            marker_path,
            cancellation,
        })
    }

    pub async fn wait(&self) -> String {
        let mut cancellation = self.cancellation.subscribe();
        if *cancellation.borrow() {
            return "cancelled".into();
        }

        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 120 & child=$!; printf '%s %s' $$ \"$child\" > \"$1\"; wait")
            .arg("avdkit-cancellation-probe")
            .arg(&self.marker_path)
            .process_group(0)
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = command.spawn().expect("failed to start cancellation probe");
        let mut group = ProcessGroup::new(child.id().expect("probe process has no PID"));

        tokio::select! {
            status = child.wait() => {
                let status = status.expect("failed to wait for probe process");
                group.disarm();
                format!("probe exited with {status}")
            }
            result = cancellation.changed() => {
                result.expect("cancellation sender was dropped");
                "cancelled".into()
            },
        }
    }

    pub fn cancel(&self) {
        self.cancellation.send_replace(true);
    }
}

#[uniffi::export]
pub fn binding_probe() -> String {
    "avdkit-uniffi-validation".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_before_wait_is_observed() {
        let probe = CancellationProbe::new("unused".into());
        probe.cancel();
        assert_eq!(probe.wait().await, "cancelled");
    }
}
