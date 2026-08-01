#![cfg(unix)]

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};

struct Session {
    child: Child,
    output: BufReader<std::process::ChildStdout>,
    _root: tempfile::TempDir,
}

impl Session {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary root");
        let seat = root.path().join("seat.json");
        fs::write(
            &seat,
            serde_json::to_vec(&json!({
                "schema": "ironbed.seat/v0",
                "generation": "host-cancel",
                "surface": {
                    "system": system(),
                    "substrate": "host",
                    "image": null
                },
                "provider": {
                    "identity": "current",
                    "network": "inherited",
                    "resources": [
                        {
                            "id": "program",
                            "path": "/bin/sleep",
                            "class": "read_only"
                        },
                        {
                            "id": "cwd",
                            "path": root.path(),
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
            "id": "cancel",
            "surface": {
                "system": system(),
                "substrate": "host"
            },
            "resources": ["program", "cwd"],
            "process": {
                "program": "/bin/sleep",
                "args": ["60"],
                "cwd": root.path(),
                "env": {}
            },
            "limits": {
                "timeout_ms": 60000,
                "output_bytes": 1048576
            }
        });
        let mut child = Command::new(env!("CARGO_BIN_EXE_ironbed"))
            .args(["run", "--seat"])
            .arg(seat)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Ironbed");
        serde_json::to_writer(child.stdin.as_mut().expect("Ironbed stdin"), &input)
            .expect("attempt");
        drop(child.stdin.take());
        let output = BufReader::new(child.stdout.take().expect("Ironbed stdout"));
        Self {
            child,
            output,
            _root: root,
        }
    }

    fn finish(mut self) -> (std::process::ExitStatus, Vec<Value>) {
        let mut bytes = Vec::new();
        self.output.read_to_end(&mut bytes).expect("Ironbed frames");
        let status = self.child.wait().expect("Ironbed result");
        let frames = bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).expect("Ironbed frame"))
            .collect();
        (status, frames)
    }
}

fn system() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

#[test]
fn cancels() {
    let mut session = Session::new();
    let mut line = String::new();
    session.output.read_line(&mut line).expect("started frame");
    let started: Value = serde_json::from_str(&line).expect("started frame");
    assert_eq!(started["kind"], "started");

    let result = unsafe { libc::kill(session.child.id() as i32, libc::SIGTERM) };
    assert_eq!(result, 0, "cancel Ironbed");
    let (status, mut frames) = session.finish();
    assert!(status.success());
    frames.insert(0, started);
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["kind"], "finished");
    assert_eq!(finished["process"]["termination"], "cancelled");
    assert_eq!(finished["process"]["code"], Value::Null);
    assert_eq!(finished["cleanup"]["termination_signal"]["signal"], "kill");
    assert_eq!(
        finished["cleanup"]["termination_signal"]["scope"],
        "process_group"
    );
}
