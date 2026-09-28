use std::{io, process::Command};

/// Configures a child so its descendants can be terminated as one unit.
pub fn prepare_child(command: &mut Command) {
    os::prepare_child(command);
}

/// Owns the platform process group created for one child process.
#[derive(Debug)]
pub struct ProcessGroup {
    id: Option<i32>,
    armed: bool,
}

impl ProcessGroup {
    pub fn for_child(pid: u32) -> Self {
        Self {
            id: os::group_id(pid),
            armed: true,
        }
    }

    pub fn kill(&mut self) -> io::Result<()> {
        if !self.armed {
            return Ok(());
        }
        let result = match self.id {
            Some(id) => os::kill_group(id),
            None => Ok(()),
        };
        if result.is_ok() {
            self.armed = false;
        }
        result
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

#[cfg(unix)]
mod os {
    use std::{io, os::unix::process::CommandExt, process::Command};

    pub fn prepare_child(command: &mut Command) {
        command.process_group(0);
    }

    pub fn group_id(pid: u32) -> Option<i32> {
        i32::try_from(pid).ok()
    }

    pub fn kill_group(id: i32) -> io::Result<()> {
        let result = unsafe { libc::kill(-id, libc::SIGKILL) };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(not(unix))]
mod os {
    use std::{io, process::Command};

    pub fn prepare_child(_command: &mut Command) {}

    pub fn group_id(_pid: u32) -> Option<i32> {
        None
    }

    pub fn kill_group(_id: i32) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        process::Stdio,
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{Duration, Instant},
    };

    static NEXT_MARKER: AtomicU64 = AtomicU64::new(1);

    fn process_exists(pid: i32) -> bool {
        let result = unsafe { libc::kill(pid, 0) };
        result == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    #[test]
    fn killing_a_group_terminates_the_child_and_its_descendant() {
        let marker = std::env::temp_dir().join(format!(
            "process-group-{}-{}",
            std::process::id(),
            NEXT_MARKER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 120 & child=$!; printf '%s %s' $$ \"$child\" > \"$1\"; wait")
            .arg("process-group-test")
            .arg(&marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        prepare_child(&mut command);

        let mut child = command.spawn().unwrap();
        let mut group = ProcessGroup::for_child(child.id());
        let deadline = Instant::now() + Duration::from_secs(5);
        let processes = loop {
            if let Ok(contents) = fs::read_to_string(&marker) {
                let values = contents
                    .split_whitespace()
                    .map(str::parse::<i32>)
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                if values.len() == 2 {
                    break values;
                }
            }
            assert!(Instant::now() < deadline, "child did not write its PIDs");
            thread::sleep(Duration::from_millis(25));
        };

        group.kill().unwrap();
        child.wait().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while processes.iter().copied().any(process_exists) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }

        assert!(processes.iter().copied().all(|pid| !process_exists(pid)));
        let _ = fs::remove_file(marker);
    }
}
