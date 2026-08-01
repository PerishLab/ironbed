use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) struct Trial<'a> {
    pub(super) generation: &'a str,
    pub(super) required: &'a str,
    pub(super) provided: &'a str,
    pub(super) program: &'a Path,
    pub(super) args: &'a [&'a str],
    pub(super) resources: &'a [&'a str],
    pub(super) timeout: u64,
    pub(super) output: u64,
    pub(super) artifact: Option<Export<'a>>,
}

#[derive(Clone, Copy)]
pub(super) struct Export<'a> {
    pub(super) target: &'a Path,
    pub(super) limit: u64,
}

pub(super) fn system() -> String {
    format!(
        "{}-{}",
        match std::env::consts::OS {
            "macos" => "macos",
            value => value,
        },
        std::env::consts::ARCH
    )
}

pub(super) fn launch(trial: Trial<'_>) -> (std::process::ExitStatus, Vec<Value>) {
    let binary = env!("CARGO_BIN_EXE_ironbed");
    let cwd = tempfile::tempdir().expect("temporary cwd");
    let seat = cwd.path().join("seat.json");
    let source = cwd.path().join("artifact");
    let mut resources = vec![
        json!({
            "id": "program",
            "path": trial.program,
            "class": "read_only"
        }),
        json!({
            "id": "cwd",
            "path": cwd.path(),
            "class": "temporary"
        }),
    ];
    let mut artifacts = Vec::new();
    let mut env = json!({});
    let limit = match trial.artifact {
        Some(export) => {
            resources.push(json!({
                "id": "artifact",
                "path": export.target,
                "class": "artifact"
            }));
            artifacts.push(json!({
                "id": "artifact",
                "source": source,
                "grant": "cwd",
                "target": "artifact"
            }));
            env = json!({"IRONBED_ARTIFACT": source});
            export.limit
        }
        None => 0,
    };
    fs::write(
        &seat,
        serde_json::to_vec(&json!({
            "schema": "ironbed.seat/v0",
            "generation": trial.generation,
            "surface": {
                "system": trial.provided,
                "substrate": "host",
                "image": null
            },
            "provider": {
                "identity": "current",
                "network": "inherited",
                "resources": resources
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
        "artifacts": artifacts,
        "process": {
            "program": trial.program,
            "args": trial.args,
            "cwd": cwd.path(),
            "env": env
        },
        "limits": {
            "timeout_ms": trial.timeout,
            "output_bytes": trial.output,
            "artifact_bytes": limit
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

pub(super) fn offer(
    required: &str,
    provided: &str,
    args: &[&str],
) -> (std::process::ExitStatus, Vec<Value>) {
    launch(Trial {
        generation: "host-fixture",
        required,
        provided,
        program: Path::new(env!("CARGO_BIN_EXE_ironbed")),
        args,
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 1_048_576,
        artifact: None,
    })
}

pub(super) fn submit(required: &str, args: &[&str]) -> (std::process::ExitStatus, Vec<Value>) {
    offer(required, &system(), args)
}

pub(super) fn execute(args: &[&str]) -> (std::process::ExitStatus, Vec<Value>) {
    submit(&system(), args)
}
