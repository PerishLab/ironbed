#![cfg(unix)]

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Context<'a> {
    script: &'a Path,
    source: &'a str,
    digest: &'a str,
    fixture: &'a Path,
}

#[test]
#[ignore = "requires one exact local Actions source"]
fn setup() {
    let root = PathBuf::from(std::env::var("IRONBED_ACTIONS_ROOT").expect("Actions root"));
    let source = std::env::var("IRONBED_ACTIONS_SOURCE").expect("Actions source");
    let script = root.join("setup-binary/install.sh");
    exact(&root, &source);
    let digest = format!(
        "sha256:{}",
        hash(&fs::read(&script).expect("Actions script"))
    );
    let fixture = tempfile::tempdir().expect("fixture");
    prepare(fixture.path());
    let context = Context {
        script: &script,
        source: &source,
        digest: &digest,
        fixture: fixture.path(),
    };
    for _ in 0..2 {
        let scratch = tempfile::tempdir().expect("scratch");
        let retired = scratch.path().to_path_buf();
        let target = tempfile::tempdir().expect("artifact target");
        let frames = launch(&context, scratch.path(), target.path());
        let install = scratch
            .path()
            .join("temp/setup-binary/probe/beta/v1.2.3-beta.4");
        assert!(install.join("install").is_dir());
        assert!(install.join("bin").is_dir());
        assert_eq!(
            fs::read_to_string(scratch.path().join("github-path")).expect("GitHub path"),
            format!("{}\n", install.join("bin").display())
        );
        assert_eq!(
            fs::read_to_string(scratch.path().join("manager-call")).expect("manager call"),
            format!(
                "channel=beta\nversion=v1.2.3-beta.4\ninstall={}\nbin={}\n",
                install.join("install").display(),
                install.join("bin").display()
            )
        );
        let seal = fs::read(fixture.path().join("seal.json")).expect("fixture seal");
        assert_eq!(
            fs::read(target.path().join("seal")).expect("sealed artifact"),
            seal
        );
        let artifact = frames
            .iter()
            .find(|frame| frame["kind"] == "artifact")
            .expect("artifact frame");
        assert_eq!(artifact["artifact"]["id"], "seal");
        assert_eq!(artifact["artifact"]["target"], "artifact");
        assert_eq!(
            artifact["artifact"]["digest"],
            format!("sha256:{}", hash(&seal))
        );
        let action = frames[0]["provider"]["resources"]
            .as_array()
            .and_then(|resources| resources.iter().find(|item| item["id"] == "action"))
            .expect("action resource");
        assert_eq!(action["digest"], context.digest);
        assert_eq!(
            action["source"],
            format!("git:{}:setup-binary/install.sh", context.source)
        );
        let finished = frames.last().expect("finished frame");
        assert_eq!(finished["process"]["success"], true);
        assert_eq!(finished["artifacts"]["complete"], true);
        drop(scratch);
        assert!(!retired.exists());
        assert!(target.path().join("seal").exists());
    }
}

fn exact(root: &Path, expected: &str) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git revision");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["diff", "--quiet", "HEAD", "--", "setup-binary"])
            .status()
            .expect("git diff")
            .success()
    );
}

fn prepare(root: &Path) {
    let bin = root.join("bin");
    fs::create_dir(&bin).expect("fixture bin");
    let manager = root.join("manager.sh");
    fs::write(
        &manager,
        "#!/usr/bin/env sh\nset -eu\nmkdir -p \"$PROBE_INSTALL_ROOT\" \"$PROBE_LOCAL_BIN_DIR\"\nprintf 'channel=%s\\nversion=%s\\ninstall=%s\\nbin=%s\\n' \"$PROBE_CHANNEL\" \"$PROBE_VERSION\" \"$PROBE_INSTALL_ROOT\" \"$PROBE_LOCAL_BIN_DIR\" > \"$MANAGER_CALL\"\n",
    )
    .expect("fixture manager");
    let curl = bin.join("curl");
    fs::write(
        &curl,
        "#!/usr/bin/env sh\nset -eu\nurl=\nout=\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    -o) out=$2; shift 2 ;;\n    http*) url=$1; shift ;;\n    *) shift ;;\n  esac\ndone\ncase \"$url\" in\n  */seal.json) cp \"$FAKE_SEAL\" \"$out\" ;;\n  *) cp \"$FAKE_MANAGER\" \"$out\" ;;\nesac\n",
    )
    .expect("fixture curl");
    let mut permissions = fs::metadata(&curl).expect("curl metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&curl, permissions).expect("curl permissions");
    fs::write(
        root.join("seal.json"),
        "{\n  \"managers\": {\n    \"unix\": {\n      \"url\": \"https://objects.example.test/manage.sh\"\n    }\n  }\n}\n",
    )
    .expect("fixture seal");
}

fn launch(context: &Context<'_>, scratch: &Path, target: &Path) -> Vec<Value> {
    let binary = env!("CARGO_BIN_EXE_ironbed");
    let seat = scratch.join("seat.json");
    let temp = scratch.join("temp");
    let home = scratch.join("home");
    fs::create_dir(&temp).expect("runner temp");
    fs::create_dir(&home).expect("home");
    let seal = temp.join("probe-beta-v1.2.3-beta.4-seal.json");
    let generation = scratch
        .file_name()
        .and_then(|name| name.to_str())
        .expect("generation");
    fs::write(
        &seat,
        serde_json::to_vec(&json!({
            "schema": "ironbed.seat/v0",
            "generation": generation,
            "surface": {
                "system": format!("linux-{}", std::env::consts::ARCH),
                "substrate": "host",
                "image": null
            },
            "provider": {
                "identity": "actions-fixture",
                "network": "fixture-only",
                "resources": [
                    {"id": "shell", "path": "/bin/sh", "class": "read_only"},
                    {
                        "id": "action",
                        "path": context.script,
                        "class": "read_only",
                        "digest": context.digest,
                        "source": format!("git:{}:setup-binary/install.sh", context.source)
                    },
                    {"id": "fixture", "path": context.fixture, "class": "read_only"},
                    {"id": "scratch", "path": scratch, "class": "temporary"},
                    {"id": "artifact", "path": target, "class": "artifact"}
                ]
            }
        }))
        .expect("seat JSON"),
    )
    .expect("seat");
    let input = json!({
        "schema": "ironbed.rehearsal/v0",
        "id": "actions-exact-setup",
        "surface": {
            "system": format!("linux-{}", std::env::consts::ARCH),
            "substrate": "host"
        },
        "resources": ["shell", "action", "fixture", "scratch"],
        "artifacts": [{
            "id": "seal",
            "source": seal,
            "grant": "scratch",
            "target": "artifact"
        }],
        "process": {
            "program": "/bin/sh",
            "args": [context.script],
            "cwd": scratch,
            "env": {
                "HOME": home,
                "RUNNER_TEMP": temp,
                "GITHUB_PATH": scratch.join("github-path"),
                "PATH": format!("{}/bin:{}", context.fixture.display(), std::env::var("PATH").unwrap()),
                "PERISH_SETUP_PRODUCT": "probe",
                "PERISH_SETUP_CHANNEL": "beta",
                "PERISH_SETUP_VERSION": "v1.2.3-beta.4",
                "MANAGER_CALL": scratch.join("manager-call"),
                "FAKE_MANAGER": context.fixture.join("manager.sh"),
                "FAKE_SEAL": context.fixture.join("seal.json")
            }
        },
        "limits": {
            "timeout_ms": 5_000,
            "output_bytes": 1_048_576,
            "artifact_bytes": fs::metadata(context.fixture.join("seal.json")).unwrap().len()
        }
    });
    let mut child = Command::new(binary)
        .args(["run", "--seat"])
        .arg(seat)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Ironbed should start");
    serde_json::to_writer(child.stdin.as_mut().expect("stdin"), &input).expect("attempt");
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
        .map(|line| serde_json::from_slice(line).expect("frame JSON"))
        .collect()
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
