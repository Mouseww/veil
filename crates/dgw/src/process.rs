use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Error;

/// JSON pid file stored at `data_dir/dgw.pid`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PidFile {
    pub pid: u32,
    pub proxy_port: u16,
    pub management_port: u16,
    pub exe: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartOutcome {
    Started {
        proxy_port: u16,
        management_port: u16,
    },
    AlreadyRunning {
        proxy_port: u16,
        management_port: u16,
    },
}

pub fn pid_path(data_dir: &Path) -> PathBuf {
    data_dir.join("dgw.pid")
}

pub fn current_exe() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from("dgw"))
}

pub fn write_pid(data_dir: &Path, file: &PidFile) -> Result<(), Error> {
    std::fs::create_dir_all(data_dir)?;
    let json = serde_json::to_string(file)?;
    std::fs::write(pid_path(data_dir), json)?;
    Ok(())
}

pub fn read_pid(data_dir: &Path) -> Result<Option<PidFile>, Error> {
    match std::fs::read_to_string(pid_path(data_dir)) {
        Ok(text) => Ok(Some(serde_json::from_str(text.trim())?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// True if `pid` still refers to a live process.
pub fn pid_is_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        win::is_alive(pid)
    }
    #[cfg(unix)]
    {
        unix::is_alive(pid)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = pid;
        false
    }
}

/// If a live pid file exists for the same exe, return `AlreadyRunning`.
/// Otherwise write the current pid and return `Started`.
pub fn try_start(
    data_dir: &Path,
    proxy_port: u16,
    management_port: u16,
    exe: &str,
) -> Result<StartOutcome, Error> {
    if let Some(existing) = read_pid(data_dir)? {
        if pid_is_alive(existing.pid) && existing.exe == exe {
            return Ok(StartOutcome::AlreadyRunning {
                proxy_port: existing.proxy_port,
                management_port: existing.management_port,
            });
        }
    }
    write_pid(
        data_dir,
        &PidFile {
            pid: std::process::id(),
            proxy_port,
            management_port,
            exe: exe.to_string(),
        },
    )?;
    Ok(StartOutcome::Started {
        proxy_port,
        management_port,
    })
}

/// Spawn this binary as `dgw start --foreground` detached from the console.
pub fn spawn_daemon(exe: &str) -> Result<(), Error> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("start").arg("--foreground");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }
    cmd.spawn()?;
    Ok(())
}

/// Block until the OS kills this process (used by `--foreground`).
pub fn wait_forever() {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

/// Kill the recorded pid only when its stored exe matches this binary, then
/// delete the pid file. Missing file is success.
pub fn stop(data_dir: &Path) -> Result<(), Error> {
    let existing = match read_pid(data_dir)? {
        Some(p) => p,
        None => return Ok(()),
    };
    if pid_is_alive(existing.pid) && existing.exe == current_exe() {
        kill_pid(existing.pid)?;
    }
    match std::fs::remove_file(pid_path(data_dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn kill_pid(pid: u32) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        win::terminate(pid)
    }
    #[cfg(unix)]
    {
        unix::terminate(pid)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = pid;
        Ok(())
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const STILL_ACTIVE: u32 = 259;

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetExitCodeProcess(handle: *mut c_void, exit_code: *mut u32) -> i32;
        fn TerminateProcess(handle: *mut c_void, exit_code: u32) -> i32;
    }

    pub fn is_alive(pid: u32) -> bool {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            ok != 0 && code == STILL_ACTIVE
        }
    }

    pub fn terminate(pid: u32) -> std::io::Result<()> {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle.is_null() {
                return Ok(());
            }
            let _ = TerminateProcess(handle, 1);
            CloseHandle(handle);
            Ok(())
        }
    }
}

#[cfg(unix)]
mod unix {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    pub fn is_alive(pid: u32) -> bool {
        let rc = unsafe { kill(pid as i32, 0) };
        rc == 0 || std::io::Error::last_os_error().kind() == std::io::ErrorKind::PermissionDenied
    }

    pub fn terminate(pid: u32) -> std::io::Result<()> {
        const SIGTERM: i32 = 15;
        let rc = unsafe { kill(pid as i32, SIGTERM) };
        if rc == 0 {
            Ok(())
        } else {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{read_pid, stop, try_start, write_pid, PidFile, StartOutcome};

    #[test]
    fn write_read_pid_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let pf = PidFile {
            pid: 4242,
            proxy_port: 18787,
            management_port: 18788,
            exe: "/tmp/dgw".into(),
        };
        write_pid(dir.path(), &pf).unwrap();
        let got = read_pid(dir.path()).unwrap().expect("pid file");
        assert_eq!(got, pf);
    }

    #[test]
    fn try_start_twice_with_current_pid_is_already_running() {
        let dir = tempfile::tempdir().unwrap();
        let exe = "test-dgw";
        let first = try_start(dir.path(), 18787, 18788, exe).unwrap();
        assert_eq!(
            first,
            StartOutcome::Started {
                proxy_port: 18787,
                management_port: 18788,
            }
        );
        let recorded = read_pid(dir.path()).unwrap().unwrap();
        assert_eq!(recorded.pid, std::process::id());
        assert_eq!(recorded.exe, exe);
        let second = try_start(dir.path(), 19999, 19998, exe).unwrap();
        assert_eq!(
            second,
            StartOutcome::AlreadyRunning {
                proxy_port: 18787,
                management_port: 18788,
            }
        );
    }

    #[test]
    fn stop_missing_pid_file_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        stop(dir.path()).unwrap();
    }
}
