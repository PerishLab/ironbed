use super::attempt::Attempt;
use std::io;
use std::process::{Child, Command, Stdio};

pub(super) fn command(attempt: &Attempt) -> Command {
    let mut command = Command::new(&attempt.process.program);
    command
        .args(&attempt.process.args)
        .current_dir(&attempt.process.cwd)
        .env_clear()
        .envs(&attempt.process.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

pub(super) fn abort(child: &mut Child, error: String) -> Result<(), String> {
    let _ = stop(child);
    let _ = child.wait();
    Err(error)
}

pub(super) fn stop(child: &mut Child) -> Result<bool, String> {
    #[cfg(unix)]
    {
        let group = -(child.id() as i32);
        let result = unsafe { libc::kill(group, libc::SIGKILL) };
        if result == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(false);
        }
        Err(format!("cannot terminate child process group: {error}"))
    }
    #[cfg(not(unix))]
    {
        match child.try_wait() {
            Ok(Some(_)) => Ok(false),
            Ok(None) => child
                .kill()
                .map(|_| true)
                .map_err(|error| format!("cannot terminate child: {error}")),
            Err(error) => Err(format!("cannot inspect child: {error}")),
        }
    }
}

pub(super) fn scope() -> &'static str {
    #[cfg(unix)]
    {
        "process_group"
    }
    #[cfg(not(unix))]
    {
        "process"
    }
}
