use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

struct Trial<'a> {
    required: &'a str,
    provided: &'a str,
    program: &'a Path,
    args: &'a [&'a str],
    resources: &'a [&'a str],
    timeout: u64,
    output: u64,
}

fn system() -> String {
    format!(
        "{}-{}",
        match std::env::consts::OS {
            "macos" => "macos",
            value => value,
        },
        std::env::consts::ARCH
    )
}

fn launch(trial: Trial<'_>) -> (std::process::ExitStatus, Vec<Value>) {
    let binary = env!("CARGO_BIN_EXE_ironbed");
    let cwd = tempfile::tempdir().expect("temporary cwd");
    let seat = cwd.path().join("seat.json");
    fs::write(
        &seat,
        serde_json::to_vec(&json!({
            "schema": "ironbed.seat/v0",
            "surface": {
                "system": trial.provided,
                "substrate": "host",
                "image": null
            },
            "provider": {
                "identity": "current",
                "network": "inherited",
                "resources": [
                    {
                        "id": "program",
                        "path": trial.program,
                        "class": "read_only"
                    },
                    {
                        "id": "cwd",
                        "path": cwd.path(),
                        "class": "temporary"
                    }
                ]
            }
        }))
        .expect("seat"),
    )
    .expect("seat");
    let input = json!({
        "schema": "ironbed.rehearsal/v0",
        "id": "fixture",
        "surface": {
            "system": trial.required,
            "substrate": "host"
        },
        "resources": trial.resources,
        "process": {
            "program": trial.program,
            "args": trial.args,
            "cwd": cwd.path(),
            "env": {}
        },
        "limits": {
            "timeout_ms": trial.timeout,
            "output_bytes": trial.output
        }
    });
    let mut child = Command::new(binary)
        .args(["run", "--seat"])
        .arg(seat)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("ironbed should start");
    serde_json::to_writer(child.stdin.as_mut().expect("stdin should exist"), &input)
        .expect("attempt should write");
    child
        .stdin
        .take()
        .expect("stdin should exist")
        .flush()
        .expect("attempt should flush");
    let output = child.wait_with_output().expect("ironbed should finish");
    let frames = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("frame should be JSON"))
        .collect();
    (output.status, frames)
}

fn offer(required: &str, provided: &str, args: &[&str]) -> (std::process::ExitStatus, Vec<Value>) {
    launch(Trial {
        required,
        provided,
        program: Path::new(env!("CARGO_BIN_EXE_ironbed")),
        args,
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 1_048_576,
    })
}

fn submit(required: &str, args: &[&str]) -> (std::process::ExitStatus, Vec<Value>) {
    offer(required, &system(), args)
}

fn execute(args: &[&str]) -> (std::process::ExitStatus, Vec<Value>) {
    submit(&system(), args)
}

#[test]
fn reports() {
    let (status, frames) = execute(&["--version"]);
    assert!(status.success());
    assert_eq!(
        frames.first().and_then(|frame| frame["kind"].as_str()),
        Some("started")
    );
    assert_eq!(
        frames.last().and_then(|frame| frame["kind"].as_str()),
        Some("finished")
    );
    assert_eq!(
        frames
            .last()
            .and_then(|frame| frame["process"]["code"].as_i64()),
        Some(0)
    );
    assert_eq!(
        frames
            .last()
            .and_then(|frame| frame["process"]["termination"].as_str()),
        Some("process")
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame["stream"] == "stdout" && frame["bytes"].is_array())
    );
}

#[test]
fn preserves() {
    let (status, frames) = execute(&["unknown"]);
    assert!(status.success());
    assert_eq!(
        frames
            .last()
            .and_then(|frame| frame["process"]["code"].as_i64()),
        Some(2)
    );
    assert_eq!(
        frames
            .last()
            .and_then(|frame| frame["process"]["success"].as_bool()),
        Some(false)
    );
}

#[test]
fn refuses() {
    let (status, frames) = submit("not-a-system", &["--version"]);
    assert_eq!(status.code(), Some(2));
    assert!(frames.is_empty());
}

#[test]
fn detects() {
    let (status, frames) = offer("not-a-system", "not-a-system", &["--version"]);
    assert_eq!(status.code(), Some(2));
    assert!(frames.is_empty());
}

#[test]
fn grants() {
    let current = system();
    let (status, frames) = launch(Trial {
        required: &current,
        provided: &current,
        program: Path::new(env!("CARGO_BIN_EXE_ironbed")),
        args: &["--version"],
        resources: &["cwd"],
        timeout: 5_000,
        output: 1_048_576,
    });
    assert_eq!(status.code(), Some(2));
    assert!(frames.is_empty());
}

#[cfg(unix)]
#[test]
fn expires() {
    let current = system();
    let (status, frames) = launch(Trial {
        required: &current,
        provided: &current,
        program: Path::new("/bin/sleep"),
        args: &["1"],
        resources: &["program", "cwd"],
        timeout: 10,
        output: 1_048_576,
    });
    assert!(status.success());
    assert_eq!(
        frames
            .last()
            .and_then(|frame| frame["process"]["termination"].as_str()),
        Some("timeout")
    );
    assert_eq!(
        frames
            .last()
            .and_then(|frame| frame["process"]["code"].as_i64()),
        None
    );
}

#[test]
fn output() {
    let current = system();
    let (status, frames) = launch(Trial {
        required: &current,
        provided: &current,
        program: Path::new(env!("CARGO_BIN_EXE_ironbed")),
        args: &["--help"],
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 16,
    });
    assert!(status.success());
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["process"]["termination"], "output_limit");
    assert_eq!(
        finished["evidence"]["stdout_bytes"]
            .as_u64()
            .unwrap_or_default()
            + finished["evidence"]["stderr_bytes"]
                .as_u64()
                .unwrap_or_default(),
        16
    );
    assert!(
        finished["evidence"]["discarded_stdout_bytes"]
            .as_u64()
            .unwrap_or_default()
            + finished["evidence"]["discarded_stderr_bytes"]
                .as_u64()
                .unwrap_or_default()
            > 0
    );
}

#[cfg(unix)]
#[test]
fn tree() {
    let current = system();
    let started = std::time::Instant::now();
    let (status, frames) = launch(Trial {
        required: &current,
        provided: &current,
        program: Path::new("/bin/sh"),
        args: &["-c", "sleep 60 &"],
        resources: &["program", "cwd"],
        timeout: 50,
        output: 1_048_576,
    });
    assert!(status.success());
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["process"]["termination"], "timeout");
    assert_eq!(finished["process"]["code"], 0);
    assert_eq!(
        finished["cleanup"]["termination_signal"]["scope"],
        "process_group"
    );
    assert_eq!(finished["cleanup"]["direct_process_reaped"], true);
}
