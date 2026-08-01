#[path = "run/actions.rs"]
mod actions;
#[path = "run/artifact.rs"]
mod artifact;
#[path = "run/fixture.rs"]
mod fixture;
#[path = "run/package.rs"]
mod package;

use fixture::{Trial, execute, launch, offer, submit, system};
use std::path::Path;

#[test]
fn reports() {
    let (status, frames) = execute(&["--version"]);
    assert!(status.success());
    assert_eq!(
        frames.first().and_then(|frame| frame["kind"].as_str()),
        Some("started")
    );
    assert_eq!(frames[0]["seat"]["generation"], "host-fixture");
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
fn generation() {
    let current = system();
    let (status, frames) = launch(Trial {
        generation: "",
        required: &current,
        provided: &current,
        program: Path::new(env!("CARGO_BIN_EXE_ironbed")),
        args: &["--version"],
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 1_048_576,
        artifact: None,
    });
    assert_eq!(status.code(), Some(2));
    assert!(frames.is_empty());
}

#[test]
fn grants() {
    let current = system();
    let (status, frames) = launch(Trial {
        generation: "host-fixture",
        required: &current,
        provided: &current,
        program: Path::new(env!("CARGO_BIN_EXE_ironbed")),
        args: &["--version"],
        resources: &["cwd"],
        timeout: 5_000,
        output: 1_048_576,
        artifact: None,
    });
    assert_eq!(status.code(), Some(2));
    assert!(frames.is_empty());
}

#[cfg(unix)]
#[test]
fn expires() {
    let current = system();
    let (status, frames) = launch(Trial {
        generation: "host-fixture",
        required: &current,
        provided: &current,
        program: Path::new("/bin/sleep"),
        args: &["1"],
        resources: &["program", "cwd"],
        timeout: 10,
        output: 1_048_576,
        artifact: None,
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
        generation: "host-fixture",
        required: &current,
        provided: &current,
        program: Path::new(env!("CARGO_BIN_EXE_ironbed")),
        args: &["--help"],
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 16,
        artifact: None,
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
        generation: "host-fixture",
        required: &current,
        provided: &current,
        program: Path::new("/bin/sh"),
        args: &["-c", "sleep 60 &"],
        resources: &["program", "cwd"],
        timeout: 50,
        output: 1_048_576,
        artifact: None,
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
