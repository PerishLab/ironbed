use super::fixture::{Export, Trial, launch, system};
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

enum Visibility {
    Staging,
    Published,
    Reported,
}

struct Case {
    scratch: tempfile::TempDir,
    target: tempfile::TempDir,
    seat: PathBuf,
    input: Vec<u8>,
}

#[test]
#[ignore = "requires strace syscall delay injection"]
fn retires() {
    interrupt(Visibility::Staging);
    interrupt(Visibility::Published);
    report();
    fresh();
}

fn interrupt(visibility: Visibility) {
    let case = prepare(&visibility);
    let delay = match visibility {
        Visibility::Staging => "delay_enter=20s",
        Visibility::Published => "delay_exit=20s",
        Visibility::Reported => panic!("reported loss uses frame injection"),
    };
    let expression = format!("inject=link,linkat:{delay}");
    let child = spawn(&case, &expression);
    wait(&case.target, &visibility);
    let output = kill(child);
    assert!(!output.status.success());
    let frames = String::from_utf8_lossy(&output.stdout);
    assert!(!frames.contains("\"kind\":\"artifact\""));
    assert!(!frames.contains("\"kind\":\"finished\""));
    inspect(&case.target, visibility);
    retire(case);
}

fn spawn(case: &Case, expression: &str) -> Child {
    let mut command = Command::new("strace");
    command
        .args(["-qq", "--kill-on-exit", "-o", "/dev/null", "-e"])
        .arg(expression)
        .arg(env!("CARGO_BIN_EXE_ironbed"))
        .args(["run", "--seat"])
        .arg(&case.seat)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command.spawn().expect("strace should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should exist")
        .write_all(&case.input)
        .expect("attempt should write");
    drop(child.stdin.take());
    child
}

fn kill(child: Child) -> std::process::Output {
    let group = -(child.id().cast_signed());
    let killed = unsafe { libc::kill(group, libc::SIGKILL) };
    assert_eq!(killed, 0);
    child.wait_with_output().expect("strace should stop")
}

fn retire(case: Case) {
    let target = case.target.path().to_path_buf();
    case.target.close().expect("target should retire");
    assert!(!target.exists());
    assert!(case.scratch.path().exists());
}

fn report() {
    let case = prepare(&Visibility::Reported);
    let mut child = spawn(&case, "inject=write:delay_enter=20s:when=4");
    let stdout = child.stdout.take().expect("stdout should exist");
    let mut reader = BufReader::new(stdout);
    let mut lines = String::new();
    reader.read_line(&mut lines).expect("started should read");
    reader.read_line(&mut lines).expect("artifact should read");
    let frames = lines
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("frame should decode"))
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0]["kind"], "started");
    assert_eq!(frames[1]["kind"], "artifact");
    let group = -(child.id().cast_signed());
    assert_eq!(unsafe { libc::kill(group, libc::SIGKILL) }, 0);
    assert!(!child.wait().expect("strace should stop").success());
    let mut rest = String::new();
    reader
        .read_to_string(&mut rest)
        .expect("stdout should drain");
    assert!(!rest.contains("\"kind\":\"finished\""));
    inspect(&case.target, Visibility::Reported);
    retire(case);
}

fn prepare(visibility: &Visibility) -> Case {
    let scratch = tempfile::tempdir().expect("temporary scratch");
    let target = tempfile::tempdir().expect("artifact target");
    let seat = scratch.path().join("seat.json");
    let source = scratch.path().join("artifact");
    let generation = match visibility {
        Visibility::Staging => "loss-staging",
        Visibility::Published => "loss-published",
        Visibility::Reported => "loss-reported",
    };
    fs::write(
        &seat,
        serde_json::to_vec(&json!({
            "schema": "ironbed.seat/v0",
            "generation": generation,
            "surface": {
                "system": system(),
                "substrate": "host",
                "image": null
            },
            "provider": {
                "identity": "current",
                "network": "inherited",
                "resources": [
                    {"id": "program", "path": "/bin/sh", "class": "read_only"},
                    {"id": "cwd", "path": scratch.path(), "class": "temporary"},
                    {"id": "artifact", "path": target.path(), "class": "artifact"}
                ]
            }
        }))
        .expect("seat should encode"),
    )
    .expect("seat should write");
    let input = serde_json::to_vec(&json!({
        "schema": "ironbed.rehearsal/v0",
        "id": "loss",
        "surface": {"system": system(), "substrate": "host"},
        "resources": ["program", "cwd"],
        "artifacts": [{
            "id": "artifact",
            "source": source,
            "grant": "cwd",
            "target": "artifact"
        }],
        "process": {
            "program": "/bin/sh",
            "args": ["-c", "printf artifact > \"$IRONBED_ARTIFACT\""],
            "cwd": scratch.path(),
            "env": {"IRONBED_ARTIFACT": source}
        },
        "limits": {
            "timeout_ms": 5_000,
            "output_bytes": 1_048_576,
            "artifact_bytes": 8
        }
    }))
    .expect("attempt should encode");
    Case {
        scratch,
        target,
        seat,
        input,
    }
}

fn wait(target: &tempfile::TempDir, visibility: &Visibility) {
    let started = Instant::now();
    loop {
        let staging = target.path().join(".artifact.tmp").exists();
        let published = target.path().join("artifact").exists();
        let visible = match visibility {
            Visibility::Staging => staging && !published,
            Visibility::Published => staging && published,
            Visibility::Reported => !staging && published,
        };
        if visible {
            return;
        }
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn inspect(target: &tempfile::TempDir, visibility: Visibility) {
    let staging = target.path().join(".artifact.tmp");
    let published = target.path().join("artifact");
    match visibility {
        Visibility::Staging => {
            assert_eq!(
                fs::read(&staging).expect("staging should exist"),
                b"artifact"
            );
            assert!(!published.exists());
        }
        Visibility::Published => {
            assert_eq!(
                fs::read(&staging).expect("staging should exist"),
                b"artifact"
            );
            assert_eq!(
                fs::read(&published).expect("artifact should exist"),
                b"artifact"
            );
            assert_eq!(inode(&staging), inode(&published));
        }
        Visibility::Reported => {
            assert!(!staging.exists());
            assert_eq!(
                fs::read(&published).expect("artifact should exist"),
                b"artifact"
            );
        }
    }
}

fn inode(path: &Path) -> u64 {
    fs::metadata(path).expect("artifact metadata").ino()
}

fn fresh() {
    let current = system();
    let target = tempfile::tempdir().expect("fresh artifact target");
    let (status, frames) = launch(Trial {
        generation: "loss-replacement",
        required: &current,
        provided: &current,
        program: Path::new("/bin/sh"),
        args: &["-c", "printf artifact > \"$IRONBED_ARTIFACT\""],
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 1_048_576,
        artifact: Some(Export {
            target: target.path(),
            limit: 8,
        }),
    });
    assert!(status.success());
    assert_eq!(
        fs::read(target.path().join("artifact")).unwrap(),
        b"artifact"
    );
    assert_eq!(frames[frames.len() - 2]["kind"], "artifact");
    assert_eq!(frames.last().expect("finished frame")["kind"], "finished");
}
