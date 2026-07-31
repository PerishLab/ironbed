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
    timeout: u64,
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
        "process": {
            "program": trial.program,
            "args": trial.args,
            "cwd": cwd.path(),
            "env": {}
        },
        "limits": {
            "timeout_ms": trial.timeout
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
        timeout: 5_000,
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

#[cfg(unix)]
#[test]
fn expires() {
    let current = system();
    let (status, frames) = launch(Trial {
        required: &current,
        provided: &current,
        program: Path::new("/bin/sleep"),
        args: &["1"],
        timeout: 10,
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
