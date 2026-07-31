#![cfg(unix)]

#[path = "hardrig/model.rs"]
mod model;
#[path = "hardrig/server.rs"]
mod server;

use serde_json::{Value, json};
use server::{Cube, sshd};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn attempt(program: &Path, root: &Path, state: &Path, action: &str) -> (Output, Vec<Value>) {
    let binary = env!("CARGO_BIN_EXE_ironbed");
    let mut args = vec![
        "--root".to_string(),
        root.display().to_string(),
        "--state".to_string(),
        state.display().to_string(),
        action.to_string(),
        "liberte.top".to_string(),
    ];
    if action == "apply" {
        args.push("--yes".to_string());
    }
    let input = json!({
        "schema": "ironbed.rehearsal/v0",
        "id": format!("hardrig-{action}"),
        "surface": {
            "system": system(),
            "substrate": "host"
        },
        "process": {
            "program": program,
            "args": args,
            "cwd": root,
            "env": {}
        }
    });
    let mut child = Command::new(binary)
        .arg("run")
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

#[test]
#[ignore = "requires a real Hardrig binary, sshd, and ssh-keygen"]
fn recovers() {
    let binary = PathBuf::from(std::env::var_os("IRONBED_HARDRIG_BIN").expect("Hardrig binary"));
    let binary = binary.canonicalize().expect("absolute Hardrig binary");
    let service = tempfile::tempdir().expect("service root");
    let (_sshd, port, user) = sshd(service.path());
    let cube = Cube::start(port, &user);
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
