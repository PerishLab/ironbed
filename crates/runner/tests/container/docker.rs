use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) struct Docker {
    image: String,
    digest: String,
    ironbed: PathBuf,
    hardrig: PathBuf,
    pub(super) seal: String,
    pub(super) origin: String,
    pub(super) user: String,
}

impl Docker {
    pub(super) fn new(state: &Path) -> Self {
        let image = std::env::var("IRONBED_CONTAINER_IMAGE").expect("container image");
        let digest = digest(&image);
        let ironbed = PathBuf::from(env!("CARGO_BIN_EXE_ironbed"))
            .canonicalize()
            .expect("Ironbed binary");
        let hardrig =
            PathBuf::from(std::env::var_os("IRONBED_HARDRIG_BIN").expect("Hardrig binary"))
                .canonicalize()
                .expect("Hardrig binary");
        let seal = sha256(&hardrig);
        let origin = std::env::var("IRONBED_HARDRIG_SOURCE").expect("Hardrig source identity");
        let owner = fs::metadata(state).expect("state metadata");
        Self {
            image,
            digest,
            ironbed,
            hardrig,
            seal,
            origin,
            user: format!("{}:{}", owner.uid(), owner.gid()),
        }
    }

    pub(super) fn run(&self, model: &Path, state: &Path, action: &str) -> (Output, Vec<Value>) {
        let (_descriptor, child) = self.spawn(model, state, action, None);
        result(child)
    }

    pub(super) fn spawn(
        &self,
        model: &Path,
        state: &Path,
        action: &str,
        name: Option<&str>,
    ) -> (tempfile::NamedTempFile, Child) {
        let mut descriptor = tempfile::NamedTempFile::new().expect("seat");
        serde_json::to_writer(
            &mut descriptor,
            &json!({
                "schema": "ironbed.seat/v0",
                "surface": {
                    "system": system(),
                    "substrate": "container",
                    "image": &self.digest
                },
                "provider": {
                    "identity": &self.user,
                    "network": "host",
                    "resources": [
                        {
                            "id": "hardrig",
                            "path": "/dut/hardrig",
                            "class": "read_only",
                            "digest": &self.seal,
                            "source": &self.origin
                        },
                        {
                            "id": "model",
                            "path": "/model",
                            "class": "read_only"
                        },
                        {
                            "id": "private-state",
                            "path": "/state",
                            "class": "private"
                        },
                        {
                            "id": "ca-bundle",
                            "path": "/dut/ca.pem",
                            "class": "read_only"
                        }
                    ]
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
        let mut command = Command::new("docker");
        command.args([
            "run",
            "--rm",
            "--interactive",
            "--network",
            "host",
            "--user",
            &self.user,
        ]);
        if let Some(name) = name {
            command.args(["--name", name]);
        }
        let mut child = command
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
        (descriptor, child)
    }
}

pub(super) fn result(child: Child) -> (Output, Vec<Value>) {
    let output = child.wait_with_output().expect("container result");
    let frames = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("Ironbed frame"))
        .collect();
    (output, frames)
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
        "resources": ["hardrig", "model", "private-state", "ca-bundle"],
        "process": {
            "program": "/dut/hardrig",
            "args": args,
            "cwd": "/model",
            "env": {
                "SSL_CERT_FILE": "/dut/ca.pem"
            }
        },
        "limits": {
            "timeout_ms": 60000,
            "output_bytes": 1048576
        }
    })
}

fn system() -> String {
    format!("linux-{}", std::env::consts::ARCH)
}

pub(super) fn bytes(frames: &[Value], stream: &str) -> Vec<u8> {
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

pub(super) fn code(frames: &[Value]) -> Option<i64> {
    frames
        .last()
        .and_then(|frame| frame["process"]["code"].as_i64())
}

pub(super) fn name() -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("ironbed-loss-{}-{nonce}", std::process::id())
}

pub(super) fn remove(name: &str) {
    let output = Command::new("docker")
        .args(["kill", "--signal", "KILL", name])
        .output()
        .expect("kill container seat");
    assert!(output.status.success(), "{output:?}");
}

pub(super) fn absent(name: &str) {
    for _ in 0..100 {
        let output = Command::new("docker")
            .args(["container", "inspect", name])
            .output()
            .expect("inspect container seat");
        if !output.status.success() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("container seat {name} was not removed");
}
