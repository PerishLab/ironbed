#![cfg(target_os = "linux")]

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ARCHIVE: &str = "ironbed-x86_64-unknown-linux-gnu.tar.gz";
const VERSION: &str = "v0.1.0-beta.1";

struct Inputs {
    source: PathBuf,
    commit: String,
    plumb: PathBuf,
    cargo: PathBuf,
    toolchain: PathBuf,
}

struct Case<'a> {
    generation: &'a str,
    scratch: &'a Path,
    target: &'a Path,
    workspace: &'a Path,
}

#[test]
#[ignore = "requires exact Ironbed source, Plumb, Cargo cache, Rust toolchain, git, and tar"]
fn builds() {
    let inputs = Inputs {
        source: required("IRONBED_SOURCE_ROOT"),
        commit: text("IRONBED_SOURCE_COMMIT"),
        plumb: required("IRONBED_PLUMB"),
        cargo: required("IRONBED_CARGO_HOME"),
        toolchain: required("IRONBED_RUST_SYSROOT"),
    };
    exact(&inputs.source, &inputs.commit);
    let mut archives = Vec::new();
    for generation in ["release-one", "release-two"] {
        let scratch = tempfile::tempdir().expect("release scratch");
        let retired = scratch.path().to_path_buf();
        let target = tempfile::tempdir().expect("artifact target");
        let workspace = checkout(&inputs, scratch.path());
        let frames = launch(
            &inputs,
            Case {
                generation,
                scratch: scratch.path(),
                target: target.path(),
                workspace: &workspace,
            },
        );
        let archive = target.path().join(ARCHIVE);
        let bytes = fs::read(&archive).expect("release archive");
        proof(&frames, &bytes);
        inspect(&archive);
        assert!(clean(&workspace));
        archives.push(bytes);
        drop(scratch);
        assert!(!retired.exists());
        assert!(archive.exists());
    }
    assert_eq!(archives[0], archives[1]);
}

fn checkout(inputs: &Inputs, scratch: &Path) -> PathBuf {
    let workspace = scratch.join("workspace");
    assert!(
        Command::new("git")
            .args(["clone", "--quiet", "--no-hardlinks", "--no-checkout"])
            .arg(&inputs.source)
            .arg(&workspace)
            .status()
            .expect("clone should run")
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&workspace)
            .args(["checkout", "--quiet", "--detach"])
            .arg(&inputs.commit)
            .status()
            .expect("checkout should run")
            .success()
    );
    workspace
}

fn launch(inputs: &Inputs, case: Case<'_>) -> Vec<Value> {
    let seat = case.scratch.join("seat.json");
    let artifacts = case.scratch.join("artifacts");
    let home = case.scratch.join("home");
    let temp = case.scratch.join("temp");
    fs::create_dir(&home).expect("home should create");
    fs::create_dir(&temp).expect("temp should create");
    fs::write(
        &seat,
        serde_json::to_vec(&json!({
            "schema": "ironbed.seat/v0",
            "generation": case.generation,
            "surface": {
                "system": "linux-x86_64",
                "substrate": "host",
                "image": null
            },
            "provider": {
                "identity": "release-fixture",
                "network": "closed",
                "resources": [
                    {
                        "id": "plumb",
                        "path": inputs.plumb,
                        "class": "read_only",
                        "digest": format!("sha256:{}", hash(&fs::read(&inputs.plumb).unwrap()))
                    },
                    {
                        "id": "workspace",
                        "path": case.workspace,
                        "class": "temporary",
                        "source": format!("git:{}", inputs.commit)
                    },
                    {"id": "cargo", "path": inputs.cargo, "class": "read_only"},
                    {"id": "toolchain", "path": inputs.toolchain, "class": "read_only"},
                    {"id": "scratch", "path": case.scratch, "class": "temporary"},
                    {"id": "artifact", "path": case.target, "class": "artifact"}
                ]
            }
        }))
        .expect("seat should encode"),
    )
    .expect("seat should write");
    let input = json!({
        "schema": "ironbed.rehearsal/v0",
        "id": "actions-release-build",
        "surface": {"system": "linux-x86_64", "substrate": "host"},
        "resources": ["plumb", "workspace", "cargo", "toolchain", "scratch"],
        "artifacts": [{
            "id": "archive",
            "name": ARCHIVE,
            "source": artifacts.join(ARCHIVE),
            "grant": "scratch",
            "target": "artifact"
        }],
        "process": {
            "program": inputs.plumb,
            "args": ["release", "build"],
            "cwd": case.workspace,
            "env": {
                "HOME": home,
                "PATH": format!("{}/bin:/usr/bin:/bin", inputs.toolchain.display()),
                "TMPDIR": temp,
                "CARGO_HOME": inputs.cargo,
                "CARGO_NET_OFFLINE": "true",
                "CARGO_TARGET_DIR": case.scratch.join("target"),
                "PLUMB_RELEASE_ROOT": case.workspace,
                "PLUMB_RELEASE_CHANNEL": "beta",
                "PLUMB_RELEASE_VERSION": VERSION,
                "PLUMB_RELEASE_COMMIT": inputs.commit,
                "PLUMB_RELEASE_TARGET": "x86_64-unknown-linux-gnu",
                "PLUMB_RELEASE_ARTIFACTS": artifacts
            }
        },
        "limits": {
            "timeout_ms": 120_000,
            "output_bytes": 1_048_576,
            "artifact_bytes": 16_777_216
        }
    });
    execute(&seat, &input)
}

fn execute(seat: &Path, input: &Value) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ironbed"))
        .args(["run", "--seat"])
        .arg(seat)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Ironbed should start");
    serde_json::to_writer(child.stdin.as_mut().expect("stdin"), input).expect("attempt");
    child.stdin.take().expect("stdin").flush().expect("flush");
    let output = child.wait_with_output().expect("Ironbed should finish");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("frame should decode"))
        .collect()
}

fn proof(frames: &[Value], bytes: &[u8]) {
    let artifact = frames
        .iter()
        .find(|frame| frame["kind"] == "artifact")
        .expect("archive artifact");
    assert_eq!(artifact["artifact"]["id"], "archive");
    assert_eq!(artifact["artifact"]["name"], ARCHIVE);
    assert_eq!(artifact["artifact"]["bytes"], bytes.len());
    assert_eq!(
        artifact["artifact"]["digest"],
        format!("sha256:{}", hash(bytes))
    );
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["process"]["success"], true);
    assert_eq!(finished["artifacts"]["complete"], true);
}

fn inspect(archive: &Path) {
    let unpack = tempfile::tempdir().expect("archive inspection");
    assert!(
        Command::new("tar")
            .args(["-xzf"])
            .arg(archive)
            .arg("-C")
            .arg(unpack.path())
            .status()
            .expect("tar should run")
            .success()
    );
    let output = Command::new(unpack.path().join("ironbed"))
        .arg("--version")
        .output()
        .expect("archive binary should run");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ironbed v0.1.0-beta.1\n"
    );
}

fn exact(root: &Path, commit: &str) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git revision");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), commit);
    assert!(clean(root));
}

fn clean(root: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("git status")
        .stdout
        .is_empty()
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn required(name: &str) -> PathBuf {
    PathBuf::from(text(name))
}

fn text(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}
