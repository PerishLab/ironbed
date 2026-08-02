#![cfg(target_os = "linux")]

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Sources {
    actions: PathBuf,
    action: String,
    shield: PathBuf,
    package: String,
    deno: PathBuf,
}

#[test]
#[ignore = "requires exact local Actions and Shield sources, Deno, and git"]
fn package() {
    let sources = Sources {
        actions: required("IRONBED_ACTIONS_ROOT"),
        action: text("IRONBED_ACTIONS_SOURCE"),
        shield: required("IRONBED_SHIELD_ROOT"),
        package: text("IRONBED_SHIELD_SOURCE"),
        deno: required("IRONBED_DENO"),
    };
    exact(&sources.actions, &sources.action, "package-exact");
    exact(&sources.shield, &sources.package, ".");
    let (endpoint, server) = server();
    let mut records = Vec::new();
    for _ in 0..2 {
        let scratch = tempfile::tempdir().expect("scratch");
        let retired = scratch.path().to_path_buf();
        let target = tempfile::tempdir().expect("artifact target");
        let frames = launch(&sources, &endpoint, scratch.path(), target.path());
        let record = fs::read(target.path().join("record.json")).expect("package record");
        let document: Value = serde_json::from_slice(&record).expect("record JSON");
        assert_eq!(document["format"], "package-exact/v1");
        assert_eq!(document["product"], "shield");
        assert_eq!(document["registry"], "jsr");
        assert_eq!(document["package"], "@perish/shield");
        assert_eq!(document["channel"], "beta");
        assert_eq!(document["version"], "v0.1.1-beta.1");
        assert_eq!(document["commit"], sources.package);
        assert_eq!(document["status"], "planned");
        assert!(
            document["integrity"]
                .as_str()
                .is_some_and(|value| value.starts_with("sha256-"))
        );
        let artifact = frames
            .iter()
            .find(|frame| frame["kind"] == "artifact")
            .expect("record artifact");
        assert_eq!(artifact["artifact"]["id"], "record");
        assert_eq!(artifact["artifact"]["name"], "record.json");
        assert_eq!(artifact["artifact"]["bytes"], record.len());
        assert_eq!(
            artifact["artifact"]["digest"],
            format!("sha256:{}", hash(&record))
        );
        let resources = frames[0]["provider"]["resources"]
            .as_array()
            .expect("provider resources");
        let action = resources
            .iter()
            .find(|item| item["id"] == "action")
            .expect("action resource");
        let workspace = resources
            .iter()
            .find(|item| item["id"] == "workspace")
            .expect("workspace resource");
        assert_eq!(
            action["source"],
            format!("git:{}:package-exact", sources.action)
        );
        assert_eq!(workspace["source"], format!("git:{}", sources.package));
        let finished = frames.last().expect("finished frame");
        assert_eq!(finished["process"]["success"], true);
        assert_eq!(finished["artifacts"]["complete"], true);
        assert!(clean(&scratch.path().join("workspace")));
        records.push(record);
        drop(scratch);
        assert!(!retired.exists());
        assert!(target.path().join("record.json").exists());
    }
    server.join().expect("registry fixture");
    assert_eq!(records[0], records[1]);
}

fn launch(sources: &Sources, endpoint: &str, scratch: &Path, target: &Path) -> Vec<Value> {
    let workspace = scratch.join("workspace");
    assert!(
        Command::new("git")
            .args(["clone", "--quiet", "--no-hardlinks", "--no-checkout"])
            .arg(&sources.shield)
            .arg(&workspace)
            .status()
            .expect("clone")
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&workspace)
            .args(["checkout", "--quiet", "--detach"])
            .arg(&sources.package)
            .status()
            .expect("checkout")
            .success()
    );
    let temp = scratch.join("temp");
    let home = scratch.join("home");
    fs::create_dir(&temp).expect("runner temp");
    fs::create_dir(&home).expect("home");
    let script = sources.actions.join("package-exact/main.ts");
    let seat = scratch.join("seat.json");
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
                "identity": "package-fixture",
                "network": "loopback-registry",
                "resources": [
                    {
                        "id": "deno",
                        "path": sources.deno,
                        "class": "read_only",
                        "digest": format!("sha256:{}", hash(&fs::read(&sources.deno).unwrap()))
                    },
                    {
                        "id": "action",
                        "path": sources.actions.join("package-exact"),
                        "class": "read_only",
                        "source": format!("git:{}:package-exact", sources.action)
                    },
                    {
                        "id": "workspace",
                        "path": workspace,
                        "class": "temporary",
                        "source": format!("git:{}", sources.package)
                    },
                    {"id": "scratch", "path": scratch, "class": "temporary"},
                    {"id": "artifact", "path": target, "class": "artifact"}
                ]
            }
        }))
        .expect("seat JSON"),
    )
    .expect("seat");
    let record = temp.join("package-exact/shield/record.json");
    let input = json!({
        "schema": "ironbed.rehearsal/v0",
        "id": "actions-package-record",
        "surface": {
            "system": format!("linux-{}", std::env::consts::ARCH),
            "substrate": "host"
        },
        "resources": ["deno", "action", "workspace", "scratch"],
        "artifacts": [{
            "id": "record",
            "name": "record.json",
            "source": record,
            "grant": "scratch",
            "target": "artifact"
        }],
        "process": {
            "program": sources.deno,
            "args": [
                "run", "--quiet", "--no-config", "--allow-read", "--allow-write",
                "--allow-env", format!("--allow-net={}", endpoint.trim_start_matches("http://")),
                "--allow-run=deno,git,node,npm,pnpm,sh", script
            ],
            "cwd": workspace,
            "env": {
                "HOME": home,
                "DENO_DIR": scratch.join("deno"),
                "GITHUB_OUTPUT": scratch.join("output"),
                "PATH": std::env::var("PATH").unwrap(),
                "RUNNER_TEMP": temp,
                "PACKAGE_EXACT_PRODUCT": "shield",
                "PACKAGE_EXACT_CHANNEL": "beta",
                "PACKAGE_EXACT_VERSION": "v0.1.1-beta.1",
                "PACKAGE_EXACT_COMMIT": sources.package,
                "PACKAGE_EXACT_PUBLISH": "false",
                "PACKAGE_EXACT_JSR_URL": endpoint
            }
        },
        "limits": {
            "timeout_ms": 30_000,
            "output_bytes": 1_048_576,
            "artifact_bytes": 4_096
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
        .map(|line| serde_json::from_slice(line).expect("frame JSON"))
        .collect()
}

fn exact(root: &Path, expected: &str, scope: &str) {
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
            .args(["diff", "--quiet", "HEAD", "--", scope])
            .status()
            .expect("git diff")
            .success()
    );
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

fn server() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("registry fixture");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("registry request");
            let mut request = [0_u8; 4_096];
            let _ = stream.read(&mut request).expect("registry read");
            stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("registry response");
        }
    });
    (endpoint, handle)
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
