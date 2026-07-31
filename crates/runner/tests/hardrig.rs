#![cfg(unix)]

#[path = "hardrig/model.rs"]
mod model;
#[path = "hardrig/server.rs"]
mod server;

use serde_json::{Value, json};
use server::{Cube, sshd};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

struct Trial<'a> {
    program: &'a Path,
    root: &'a Path,
    state: &'a Path,
    action: &'a str,
    timeout: u64,
}

fn submit(trial: Trial<'_>) -> (Output, Vec<Value>) {
    let binary = env!("CARGO_BIN_EXE_ironbed");
    let mut seat = tempfile::NamedTempFile::new().expect("seat");
    serde_json::to_writer(
        &mut seat,
        &json!({
            "schema": "ironbed.seat/v0",
            "surface": {
                "system": system(),
                "substrate": "host",
                "image": null
            },
            "provider": {
                "identity": "current",
                "network": "host",
                "resources": [
                    {
                        "id": "hardrig",
                        "path": trial.program,
                        "class": "read_only",
                        "digest": sha256(trial.program),
                        "source": origin()
                    },
                    {
                        "id": "model",
                        "path": trial.root,
                        "class": "read_only"
                    },
                    {
                        "id": "private-state",
                        "path": trial.state,
                        "class": "private"
                    }
                ]
            }
        }),
    )
    .expect("seat");
    seat.flush().expect("seat");
    let mut args = vec![
        "--root".to_string(),
        trial.root.display().to_string(),
        "--state".to_string(),
        trial.state.display().to_string(),
        trial.action.to_string(),
        "liberte.top".to_string(),
    ];
    if trial.action == "apply" {
        args.push("--yes".to_string());
    }
    let input = json!({
        "schema": "ironbed.rehearsal/v0",
        "id": format!("hardrig-{}", trial.action),
        "surface": {
            "system": system(),
            "substrate": "host"
        },
        "resources": ["hardrig", "model", "private-state"],
        "process": {
            "program": trial.program,
            "args": args,
            "cwd": trial.root,
            "env": {}
        },
        "limits": {
            "timeout_ms": trial.timeout,
            "output_bytes": 1048576
        }
    });
    let mut child = Command::new(binary)
        .args(["run", "--seat"])
        .arg(seat.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Ironbed");
    serde_json::to_writer(child.stdin.as_mut().expect("Ironbed stdin"), &input).expect("attempt");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("Ironbed result");
    let frames = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("Ironbed frame"))
        .collect();
    (output, frames)
}

fn attempt(program: &Path, root: &Path, state: &Path, action: &str) -> (Output, Vec<Value>) {
    submit(Trial {
        program,
        root,
        state,
        action,
        timeout: 60_000,
    })
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

fn sha256(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum");
    assert!(output.status.success(), "{output:?}");
    format!(
        "sha256:{}",
        String::from_utf8(output.stdout)
            .expect("sha256sum output")
            .split_whitespace()
            .next()
            .expect("sha256 digest")
    )
}

fn origin() -> String {
    std::env::var("IRONBED_HARDRIG_SOURCE").expect("Hardrig source identity")
}

fn bytes(frames: &[Value], stream: &str) -> Vec<u8> {
    frames
        .iter()
        .filter(|frame| frame["kind"] == "output" && frame["stream"] == stream)
        .flat_map(|frame| {
            frame["bytes"]
                .as_array()
                .expect("output bytes")
                .iter()
                .map(|byte| byte.as_u64().expect("byte") as u8)
        })
        .collect()
}

fn code(frames: &[Value]) -> Option<i64> {
    frames
        .last()
        .and_then(|frame| frame["process"]["code"].as_i64())
}

fn termination(frames: &[Value]) -> Option<&str> {
    frames
        .last()
        .and_then(|frame| frame["process"]["termination"].as_str())
}

#[test]
#[ignore = "requires a source-bound real Hardrig binary, sha256sum, sshd, and ssh-keygen"]
fn recovers() {
    let binary = PathBuf::from(std::env::var_os("IRONBED_HARDRIG_BIN").expect("Hardrig binary"));
    let binary = binary.canonicalize().expect("absolute Hardrig binary");
    let service = tempfile::tempdir().expect("service root");
    let (_sshd, port, user) = sshd(service.path());
    let cube = Cube::start(port, &user, Duration::ZERO);
    let model = tempfile::tempdir().expect("model root");
    model::fixture(model.path(), &cube.target, port, &user);
    let before = fs::read(model.path().join("hosts/example/one/host.toml")).expect("model");
    let private = tempfile::tempdir().expect("private parent");
    let local = private.path().join("state");
    model::state(&local);

    let (apply, frames) = attempt(&binary, model.path(), &local, "apply");
    assert!(apply.status.success(), "{apply:?}");
    assert_eq!(code(&frames), Some(2));
    assert!(
        String::from_utf8(bytes(&frames, "stdout"))
            .expect("Hardrig stdout")
            .contains("applied: root seed")
    );
    assert!(
        String::from_utf8(bytes(&frames, "stderr"))
            .expect("Hardrig stderr")
            .contains("rehearsal fence")
    );
    let seed = local.join("secrets/roots/liberte.top.json");
    assert!(seed.is_file());
    assert_eq!(
        fs::metadata(&seed)
            .expect("seed metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let (plan, frames) = attempt(&binary, model.path(), &local, "plan");
    assert!(plan.status.success(), "{plan:?}");
    assert_eq!(code(&frames), Some(0));
    let stdout = String::from_utf8(bytes(&frames, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("observed resource.seed: ready"));
    assert!(stdout.contains("observed session.ssh: change"));
    assert_eq!(
        fs::read(model.path().join("hosts/example/one/host.toml")).expect("model"),
        before
    );
    assert_eq!(cube.finish().len(), 7);
}

#[test]
#[ignore = "requires a source-bound real Hardrig binary, sha256sum, sshd, and ssh-keygen"]
fn expires() {
    let binary = PathBuf::from(std::env::var_os("IRONBED_HARDRIG_BIN").expect("Hardrig binary"));
    let binary = binary.canonicalize().expect("absolute Hardrig binary");
    let service = tempfile::tempdir().expect("service root");
    let (_sshd, port, user) = sshd(service.path());
    let cube = Cube::start(port, &user, Duration::from_secs(2));
    let desired = tempfile::tempdir().expect("model root");
    model::fixture(desired.path(), &cube.target, port, &user);
    let private = tempfile::tempdir().expect("private parent");
    let state = private.path().join("state");
    model::state(&state);

    let (apply, frames) = submit(Trial {
        program: &binary,
        root: desired.path(),
        state: &state,
        action: "apply",
        timeout: 500,
    });
    assert!(apply.status.success(), "{apply:?}");
    assert_eq!(termination(&frames), Some("timeout"));
    assert_eq!(code(&frames), None);
    assert!(state.join("secrets/roots/liberte.top.json").is_file());

    let (plan, frames) = attempt(&binary, desired.path(), &state, "plan");
    assert!(plan.status.success(), "{plan:?}");
    assert_eq!(termination(&frames), Some("process"));
    assert_eq!(code(&frames), Some(0));
    let stdout = String::from_utf8(bytes(&frames, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("observed resource.seed: ready"));
    assert_eq!(cube.finish().len(), 7);
}
