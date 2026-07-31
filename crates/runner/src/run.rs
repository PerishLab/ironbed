mod pipe;
mod report;
mod seat;

use seat::{Seat, Substrate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const ATTEMPT: &str = "ironbed.rehearsal/v0";
const FRAME: &str = "ironbed.frame/v0";

#[cfg(target_os = "linux")]
const OS: &str = "linux";
#[cfg(target_os = "macos")]
const OS: &str = "macos";
#[cfg(target_os = "windows")]
const OS: &str = "windows";
#[cfg(target_arch = "aarch64")]
const ARCH: &str = "aarch64";
#[cfg(target_arch = "x86_64")]
const ARCH: &str = "x86_64";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    schema: String,
    id: String,
    surface: Requirement,
    process: Process,
    limits: Limits,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    system: String,
    substrate: Substrate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Process {
    program: String,
    args: Vec<String>,
    cwd: String,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    #[serde(rename = "timeout_ms")]
    timeout: u64,
}

pub fn start(seat: &Path) -> ExitCode {
    match execute(seat) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ironbed: {error}");
            ExitCode::from(2)
        }
    }
}

fn execute(path: &Path) -> Result<(), String> {
    let attempt = read()?;
    let seat = seat::read(path)?;
    validate(&attempt, &seat)?;
    let observed = system();
    if seat.surface.system != observed {
        return Err(format!(
            "provided system {} does not match observed {observed}",
            seat.surface.system
        ));
    }
    let mut child = command(&attempt)
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", attempt.process.program))?;
    let out = match child.stdout.take() {
        Some(out) => out,
        None => return abort(&mut child, "child stdout is unavailable".to_string()),
    };
    let err = match child.stderr.take() {
        Some(err) => err,
        None => return abort(&mut child, "child stderr is unavailable".to_string()),
    };
    let (send, receive) = mpsc::channel();
    pipe::spawn(out, pipe::Stream::Out, send.clone());
    pipe::spawn(err, pipe::Stream::Err, send);
    let mut writer = io::stdout().lock();
    let mut reporter = report::Reporter::new(&mut writer, &attempt.id);
    if let Err(error) = reporter.emit(&started(&attempt, &seat, &observed)) {
        return abort(&mut child, error);
    }
    let timeout = Duration::from_millis(attempt.limits.timeout);
    let drained = match reporter.drain(receive, &mut child, timeout) {
        Ok(drained) => drained,
        Err(error) => return abort(&mut child, error),
    };
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => return abort(&mut child, format!("cannot reap child: {error}")),
    };
    if let Some(error) = drained.failed.as_ref() {
        return Err(error.clone());
    }
    reporter.emit(&finished(&attempt.id, status, &drained))?;
    Ok(())
}

fn abort(child: &mut std::process::Child, error: String) -> Result<(), String> {
    let _ = child.kill();
    let _ = child.wait();
    Err(error)
}

fn read() -> Result<Attempt, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read attempt: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid attempt: {error}"))
}

fn validate(attempt: &Attempt, seat: &Seat) -> Result<(), String> {
    if attempt.schema != ATTEMPT {
        return Err(format!("attempt schema must be {ATTEMPT}"));
    }
    if attempt.id.is_empty() || attempt.id.len() > 128 {
        return Err("attempt id must contain 1 through 128 bytes".to_string());
    }
    let program = Path::new(&attempt.process.program);
    if !program.is_absolute() || !program.is_file() {
        return Err("program must be one existing absolute file".to_string());
    }
    let cwd = Path::new(&attempt.process.cwd);
    if !cwd.is_absolute() || !cwd.is_dir() {
        return Err("cwd must be one existing absolute directory".to_string());
    }
    if attempt.process.env.keys().any(|key| key.is_empty()) {
        return Err("environment keys cannot be empty".to_string());
    }
    if attempt.limits.timeout == 0 || attempt.limits.timeout > 86_400_000 {
        return Err("timeout_ms must be 1 through 86400000".to_string());
    }
    if attempt.surface.system != seat.surface.system {
        return Err(format!(
            "required system {} does not match provided {}",
            attempt.surface.system, seat.surface.system
        ));
    }
    if attempt.surface.substrate != seat.surface.substrate {
        return Err("required substrate does not match provided substrate".to_string());
    }
    Ok(())
}

fn command(attempt: &Attempt) -> Command {
    let mut command = Command::new(&attempt.process.program);
    command
        .args(&attempt.process.args)
        .current_dir(&attempt.process.cwd)
        .env_clear()
        .envs(&attempt.process.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn started(attempt: &Attempt, seat: &Seat, observed: &str) -> Value {
    json!({
        "schema": FRAME,
        "attempt": attempt.id,
        "sequence": 0,
        "kind": "started",
        "surface": {
            "required": attempt.surface,
            "provided": seat.surface,
            "observed_system": observed,
            "authority": {
                "system": "provider_and_runner",
                "substrate": "provider",
                "image": "provider"
            }
        }
    })
}

fn finished(attempt: &str, status: std::process::ExitStatus, drain: &report::Drain) -> Value {
    json!({
        "schema": FRAME,
        "attempt": attempt,
        "sequence": drain.sequence,
        "kind": "finished",
        "process": {
            "success": status.success(),
            "code": status.code(),
            "termination": if drain.expired { "timeout" } else { "process" }
        },
        "evidence": {
            "stdout_bytes": drain.out,
            "stderr_bytes": drain.err
        },
        "cleanup": {
            "process_reaped": true
        }
    })
}

fn system() -> String {
    format!("{OS}-{ARCH}")
}
