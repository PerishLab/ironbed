mod pipe;
mod seat;

use pipe::{Chunk, Stream};
use seat::{Seat, Substrate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc;

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

struct Offset {
    out: u64,
    err: u64,
}

struct Output {
    stream: Stream,
    offset: u64,
    bytes: Vec<u8>,
}

impl Offset {
    fn new() -> Self {
        Self { out: 0, err: 0 }
    }

    fn add(&mut self, stream: Stream, size: usize) -> u64 {
        let offset = match stream {
            Stream::Out => &mut self.out,
            Stream::Err => &mut self.err,
        };
        let prior = *offset;
        *offset += size as u64;
        prior
    }
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
    pipe::spawn(out, Stream::Out, send.clone());
    pipe::spawn(err, Stream::Err, send);
    let mut writer = io::stdout().lock();
    if let Err(error) = emit(&mut writer, &started(&attempt, &seat, &observed)) {
        return abort(&mut child, error);
    }
    let (sequence, offset, failed) = match drain(&mut writer, &attempt.id, receive) {
        Ok(drained) => drained,
        Err(error) => return abort(&mut child, error),
    };
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => return abort(&mut child, format!("cannot reap child: {error}")),
    };
    if let Some(error) = failed {
        return Err(error);
    }
    emit(
        &mut writer,
        &finished(&attempt.id, sequence, offset, status),
    )
}

fn abort(child: &mut std::process::Child, error: String) -> Result<(), String> {
    let _ = child.kill();
    let _ = child.wait();
    Err(error)
}

fn drain(
    writer: &mut impl Write,
    attempt: &str,
    receive: mpsc::Receiver<Chunk>,
) -> Result<(u64, Offset, Option<String>), String> {
    let mut sequence = 1_u64;
    let mut offset = Offset::new();
    let mut done = 0_u8;
    let mut failed = None;
    while done < 2 {
        match receive.recv().map_err(|error| error.to_string())? {
            Chunk::Data(stream, data) => {
                let start = offset.add(stream, data.len());
                let output = Output {
                    stream,
                    offset: start,
                    bytes: data,
                };
                emit(writer, &frame(attempt, sequence, output))?;
                sequence += 1;
            }
            Chunk::Done => done += 1,
            Chunk::Failed(stream, error) => {
                failed = Some(format!("cannot read {}: {error}", stream.name()));
                done += 1;
            }
        }
    }
    Ok((sequence, offset, failed))
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

fn emit(writer: &mut impl Write, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
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

fn frame(attempt: &str, sequence: u64, output: Output) -> Value {
    json!({
        "schema": FRAME,
        "attempt": attempt,
        "sequence": sequence,
        "kind": "output",
        "stream": output.stream.name(),
        "offset": output.offset,
        "bytes": output.bytes
    })
}

fn finished(
    attempt: &str,
    sequence: u64,
    offset: Offset,
    status: std::process::ExitStatus,
) -> Value {
    json!({
        "schema": FRAME,
        "attempt": attempt,
        "sequence": sequence,
        "kind": "finished",
        "process": {
            "success": status.success(),
            "code": status.code()
        },
        "evidence": {
            "stdout_bytes": offset.out,
            "stderr_bytes": offset.err
        },
        "cleanup": {
            "process_reaped": true
        }
    })
}

fn system() -> String {
    format!("{OS}-{ARCH}")
}
