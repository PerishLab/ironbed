mod attempt;
mod cancel;
mod child;
mod pipe;
mod report;
mod seat;

use attempt::Attempt;
use seat::Seat;
use serde_json::{Value, json};
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

const FRAME: &str = "ironbed.frame/v0";
const DEPTH: usize = 8;

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

pub fn start(seat: &Path, cancel: Option<&Path>) -> ExitCode {
    match execute(seat, cancel) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ironbed: {error}");
            ExitCode::from(2)
        }
    }
}

fn execute(path: &Path, control: Option<&Path>) -> Result<(), String> {
    let attempt = Attempt::read()?;
    let seat = seat::read(path)?;
    attempt.validate(&seat)?;
    let observed = system();
    if seat.surface.system != observed {
        return Err(format!(
            "provided system {} does not match observed {observed}",
            seat.surface.system
        ));
    }
    let cancel = cancel::Cancel::new(control)?;
    let mut child = child::command(&attempt)
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", attempt.process.program))?;
    let out = match child.stdout.take() {
        Some(out) => out,
        None => return child::abort(&mut child, "child stdout is unavailable".to_string()),
    };
    let err = match child.stderr.take() {
        Some(err) => err,
        None => return child::abort(&mut child, "child stderr is unavailable".to_string()),
    };
    let (send, receive) = mpsc::sync_channel(DEPTH);
    pipe::spawn(out, pipe::Stream::Out, send.clone());
    pipe::spawn(err, pipe::Stream::Err, send);
    let mut writer = io::stdout().lock();
    let mut reporter = report::Reporter::new(&mut writer, &attempt.id, cancel);
    if let Err(error) = reporter.emit(&started(&attempt, &seat, &observed)) {
        return child::abort(&mut child, error);
    }
    let timeout = Duration::from_millis(attempt.limits.timeout);
    let drained = match reporter.drain(receive, &mut child, timeout, attempt.limits.output) {
        Ok(drained) => drained,
        Err(error) => return child::abort(&mut child, error),
    };
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => return child::abort(&mut child, format!("cannot reap child: {error}")),
    };
    if let Some(error) = drained.failed.as_ref() {
        return Err(error.clone());
    }
    reporter.emit(&finished(&attempt.id, status, &drained))?;
    Ok(())
}

fn started(attempt: &Attempt, seat: &Seat, observed: &str) -> Value {
    json!({
        "schema": FRAME,
        "attempt": attempt.id,
        "sequence": 0,
        "kind": "started",
        "seat": {
            "generation": seat.generation
        },
        "surface": {
            "required": attempt.surface,
            "provided": seat.surface,
            "observed_system": observed,
            "authority": {
                "system": "provider_and_runner",
                "substrate": "provider",
                "image": "provider"
            }
        },
        "provider": seat.provider
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
            "termination": drain.termination.name()
        },
        "evidence": {
            "stdout_bytes": drain.kept.out,
            "stderr_bytes": drain.kept.err,
            "discarded_stdout_bytes": drain.lost.out,
            "discarded_stderr_bytes": drain.lost.err,
            "output_limit_bytes": drain.limit
        },
        "cleanup": {
            "direct_process_reaped": true,
            "termination_signal": signal(drain)
        }
    })
}

fn signal(drain: &report::Drain) -> Option<Value> {
    drain.signalled.then(|| {
        json!({
            "signal": "kill",
            "scope": child::scope()
        })
    })
}

fn system() -> String {
    format!("{OS}-{ARCH}")
}
