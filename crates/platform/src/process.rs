use std::{io, path::PathBuf, process::Command};

/// Configures a child so its descendants can be terminated as one unit.
pub fn prepare_child(command: &mut Command) {
    os::prepare_child(command);
}

/// Configures a long-lived child to run in a new session.
///
/// Returns [`io::ErrorKind::Unsupported`] where detaching is not implemented.
pub fn prepare_detached_child(command: &mut Command) -> io::Result<()> {
    os::prepare_detached_child(command)
}

/// Signal used when stopping a detached emulator process group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSignal {
    /// Request orderly termination.
    Terminate,
    /// Force immediate termination.
    Kill,
}

/// Returns whether a process identifier still refers to a live process.
///
/// A live identifier may belong to an unrelated process after PID reuse; use
/// [`process_executable`] before acting on a process by identifier alone.
pub fn process_exists(pid: u32) -> io::Result<bool> {
    os::process_exists(pid)
}

/// Returns the executable image of a live process, or `None` when it has exited.
pub fn process_executable(pid: u32) -> io::Result<Option<PathBuf>> {
    os::process_executable(pid)
}

/// Sends a signal to the process group led by `pid`, falling back to the process.
pub fn signal_process_group(pid: u32, signal: ProcessSignal) -> io::Result<()> {
    os::signal_process_group(pid, signal)
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
    use super::ProcessSignal;
    use std::{io, os::unix::process::CommandExt, path::PathBuf, process::Command};

    pub fn prepare_child(command: &mut Command) {
        command.process_group(0);
    }

    pub fn prepare_detached_child(command: &mut Command) -> io::Result<()> {
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        Ok(())
    }

    pub fn group_id(pid: u32) -> Option<i32> {
        i32::try_from(pid).ok()
    }

    pub fn kill_group(id: i32) -> io::Result<()> {
        match signal(-id, libc::SIGKILL) {
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => Ok(()),
            result => result,
        }
    }

    pub fn process_exists(pid: u32) -> io::Result<bool> {
        let Ok(pid) = i32::try_from(pid) else {
            return Ok(false);
        };
        if unsafe { libc::kill(pid, 0) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EPERM) => Ok(true),
            Some(libc::ESRCH) => Ok(false),
            _ => Err(error),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn process_executable(pid: u32) -> io::Result<Option<PathBuf>> {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let Ok(pid) = i32::try_from(pid) else {
            return Ok(None);
        };
        let mut buffer = vec![0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let length =
            unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
        if length <= 0 {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(None),
                _ => Err(error),
            };
        }
        buffer.truncate(length as usize);
        Ok(Some(PathBuf::from(OsStr::from_bytes(&buffer))))
    }

    #[cfg(target_os = "linux")]
    pub fn process_executable(pid: u32) -> io::Result<Option<PathBuf>> {
        match std::fs::read_link(format!("/proc/{pid}/exe")) {
            Ok(path) => Ok(Some(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn process_executable(_pid: u32) -> io::Result<Option<PathBuf>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub fn signal_process_group(pid: u32, signal_kind: ProcessSignal) -> io::Result<()> {
        let pid = i32::try_from(pid)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "process ID is too large"))?;
        let signal_number = match signal_kind {
            ProcessSignal::Terminate => libc::SIGTERM,
            ProcessSignal::Kill => libc::SIGKILL,
        };
        match signal(-pid, signal_number) {
            Ok(()) => Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => signal(pid, signal_number),
            Err(error) => Err(error),
        }
    }

    fn signal(pid: i32, signal: i32) -> io::Result<()> {
        let result = unsafe { libc::kill(pid, signal) };
        if result == 0 {
            return Ok(());
        }
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
mod os {
    use super::ProcessSignal;
    use std::{io, path::PathBuf, process::Command};

    // Short-lived commands still terminate through `kill_on_drop`; only their
    // descendants are not grouped on this platform.
    pub fn prepare_child(_command: &mut Command) {}

    pub fn prepare_detached_child(_command: &mut Command) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub fn group_id(_pid: u32) -> Option<i32> {
        None
    }

    pub fn kill_group(_id: i32) -> io::Result<()> {
        Ok(())
    }

    pub fn process_exists(_pid: u32) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub fn process_executable(_pid: u32) -> io::Result<Option<PathBuf>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub fn signal_process_group(_pid: u32, _signal: ProcessSignal) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
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

    #[test]
    fn detached_child_leads_a_new_session() {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 120")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        prepare_detached_child(&mut command).unwrap();

        let mut child = command.spawn().unwrap();
        let pid = i32::try_from(child.id()).unwrap();
        assert_eq!(unsafe { libc::getsid(pid) }, pid);
        signal_process_group(child.id(), ProcessSignal::Kill).unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn reports_the_executable_of_a_live_process_and_none_after_exit() {
        let program = ["/bin/sleep", "/usr/bin/sleep"]
            .into_iter()
            .find(|path| std::path::Path::new(path).is_file())
            .expect("sleep is installed");
        let expected_name = std::path::Path::new(program).file_name().unwrap();
        let mut child = Command::new(program)
            .arg("120")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        // spawn can return before proc_pidpath or /proc reports the new image.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("sleep exited before its executable could be read: {status}");
            }
            match process_executable(child.id()).unwrap() {
                Some(executable) if executable.file_name() == Some(expected_name) => break,
                Some(executable) if Instant::now() >= deadline => {
                    panic!("pid {} executable was {}", child.id(), executable.display());
                }
                None if Instant::now() >= deadline => panic!("process disappeared"),
                _ => thread::sleep(Duration::from_millis(20)),
            }
        }

        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(process_executable(child.id()).unwrap(), None);
        assert!(!super::process_exists(child.id()).unwrap());
    }
}
