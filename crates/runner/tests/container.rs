#![cfg(unix)]

#[path = "hardrig/model.rs"]
mod model;
#[path = "hardrig/server.rs"]
mod server;

use serde_json::{Value, json};
use server::{Cube, sshd};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Docker {
    image: String,
    digest: String,
    ironbed: PathBuf,
    hardrig: PathBuf,
    user: String,
}

impl Docker {
    fn new(state: &Path) -> Self {
        let image = std::env::var("IRONBED_CONTAINER_IMAGE").expect("container image");
        let digest = digest(&image);
        let ironbed = PathBuf::from(env!("CARGO_BIN_EXE_ironbed"))
            .canonicalize()
            .expect("Ironbed binary");
        let hardrig =
            PathBuf::from(std::env::var_os("IRONBED_HARDRIG_BIN").expect("Hardrig binary"))
                .canonicalize()
                .expect("Hardrig binary");
        let owner = fs::metadata(state).expect("state metadata");
        Self {
            image,
            digest,
            ironbed,
            hardrig,
            user: format!("{}:{}", owner.uid(), owner.gid()),
        }
    }

    fn run(&self, model: &Path, state: &Path, action: &str) -> (Output, Vec<Value>) {
        let mut descriptor = tempfile::NamedTempFile::new().expect("seat");
        serde_json::to_writer(
            &mut descriptor,
            &json!({
                "schema": "ironbed.seat/v0",
                "surface": {
                    "system": system(),
                    "substrate": "container",
                    "image": &self.digest
                }
            }),
        )
        .expect("seat");
        descriptor.flush().expect("seat");
        let ironbed = mount(&self.ironbed, "/dut/ironbed", true);
        let hardrig = mount(&self.hardrig, "/dut/hardrig", true);
        let ca = mount(
            Path::new("/etc/ssl/certs/ca-certificates.crt"),
            "/dut/ca.pem",
            true,
        );
        let model = mount(model, "/model", true);
        let state = mount(state, "/state", false);
        let seat = mount(descriptor.path(), "/dut/seat.json", true);
        let input = attempt(action);
        let mut child = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--interactive",
                "--network",
                "host",
                "--user",
                &self.user,
            ])
            .args(["--mount", &ironbed, "--mount", &hardrig, "--mount", &ca])
            .args(["--mount", &model, "--mount", &state, "--mount", &seat])
            .args(["--workdir", "/model", "--entrypoint", "/dut/ironbed"])
            .arg(&self.image)
            .args(["run", "--seat", "/dut/seat.json"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("container seat");
        serde_json::to_writer(child.stdin.as_mut().expect("container stdin"), &input)
            .expect("attempt");
        drop(child.stdin.take());
        let output = child.wait_with_output().expect("container result");
        let frames = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).expect("Ironbed frame"))
            .collect();
        (output, frames)
    }
}

fn digest(image: &str) -> String {
    let output = Command::new("docker")
        .args(["image", "inspect", "--format", "{{.Id}}", image])
        .output()
        .expect("container image");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .expect("image digest")
        .trim()
        .to_string()
}

fn mount(source: &Path, target: &str, read: bool) -> String {
    format!(
        "type=bind,src={},dst={target}{}",
        source.display(),
        if read { ",readonly" } else { "" }
    )
}

fn attempt(action: &str) -> Value {
    let mut args = vec![
        "--root".to_string(),
        "/model".to_string(),
        "--state".to_string(),
        "/state".to_string(),
        action.to_string(),
        "liberte.top".to_string(),
    ];
    if action == "apply" {
        args.push("--yes".to_string());
    }
    json!({
        "schema": "ironbed.rehearsal/v0",
        "id": format!("hardrig-container-{action}"),
        "surface": {
            "system": system(),
            "substrate": "container"
        },
        "process": {
            "program": "/dut/hardrig",
            "args": args,
            "cwd": "/model",
            "env": {
                "SSL_CERT_FILE": "/dut/ca.pem"
            }
        }
    })
}

fn system() -> String {
    format!("linux-{}", std::env::consts::ARCH)
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
#[ignore = "requires Docker, a Linux image, a real Hardrig binary, sshd, and ssh-keygen"]
fn recovers() {
    let service = tempfile::tempdir().expect("service root");
    let (_sshd, port, user) = sshd(service.path());
    let cube = Cube::start(port, &user);
    let desired = tempfile::tempdir().expect("model root");
    model::fixture(desired.path(), &cube.target, port, &user);
    let before = fs::read(desired.path().join("hosts/example/one/host.toml")).expect("model");
    let private = tempfile::tempdir().expect("private parent");
    let state = private.path().join("state");
    model::state(&state);
    let seat = Docker::new(&state);

    let (apply, frames) = seat.run(desired.path(), &state, "apply");
    assert!(apply.status.success(), "{apply:?}");
    assert_eq!(frames[0]["surface"]["required"]["substrate"], "container");
    assert_eq!(frames[0]["surface"]["provided"]["substrate"], "container");
    assert_eq!(frames[0]["surface"]["authority"]["substrate"], "provider");
    assert_eq!(code(&frames), Some(2));
    let stdout = String::from_utf8(bytes(&frames, "stdout")).expect("Hardrig stdout");
    let stderr = String::from_utf8(bytes(&frames, "stderr")).expect("Hardrig stderr");
    assert!(stdout.contains("applied: root seed"), "{stdout}\n{stderr}");
    let seed = state.join("secrets/roots/liberte.top.json");
    assert!(seed.is_file());
    assert_eq!(
        fs::metadata(&seed)
            .expect("seed metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let (plan, frames) = seat.run(desired.path(), &state, "plan");
    assert!(plan.status.success(), "{plan:?}");
    assert_eq!(code(&frames), Some(0));
    let stdout = String::from_utf8(bytes(&frames, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("observed resource.seed: ready"));
    assert!(stdout.contains("observed session.ssh: change"));
    assert_eq!(
        fs::read(desired.path().join("hosts/example/one/host.toml")).expect("model"),
        before
    );
    assert_eq!(cube.finish().len(), 7);
}
