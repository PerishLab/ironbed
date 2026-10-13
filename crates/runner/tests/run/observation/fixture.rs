use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

pub(super) struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let fixture = Self {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(fixture.seat()).unwrap();
        fixture
    }

    pub(super) fn seat(&self) -> PathBuf {
        self.root
            .path()
            .join(if cfg!(windows) { "ironbed" } else { ".ironbed" })
    }

    pub(super) fn configure(&self, text: &str) {
        std::fs::write(self.seat().join("config.toml"), text).unwrap();
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ironbed"));
        command.env_clear();
        for name in ["PATH", "SYSTEMROOT", "WINDIR"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("HOME", self.root.path())
            .env("USERPROFILE", self.root.path())
            .env("LOCALAPPDATA", self.root.path());
        command
    }

    pub(super) fn invoke(&self, args: Option<&[&str]>) -> Output {
        match args {
            Some(args) => self.launch(args, false),
            None => self.command().output().unwrap(),
        }
    }

    pub(super) fn refuse(&self) -> Output {
        let mut child = self
            .command()
            .args(["run", "--seat", "private-seat"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"private-malformed")
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn launch(&self, args: &[&str], cancel: bool) -> Output {
        let program = if args == ["60"] {
            "/bin/sleep"
        } else {
            env!("CARGO_BIN_EXE_ironbed")
        };
        let binary = std::path::Path::new(program).canonicalize().unwrap();
        let system = format!(
            "{}-{}",
            if cfg!(target_os = "macos") {
                "macos"
            } else {
                std::env::consts::OS
            },
            std::env::consts::ARCH
        );
        let path = self.root.path().join("seat.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "schema": "ironbed.seat/v0", "generation": "private-generation",
                "surface": { "system": system, "substrate": "host" },
                "provider": { "identity": "private-provider", "network": "inherited", "resources": [
                    { "id": "program", "path": binary, "class": "read_only" },
                    { "id": "cwd", "path": self.root.path(), "class": "temporary" }
                ] }
            }))
            .unwrap(),
        )
        .unwrap();
        let input = json!({
            "schema": "ironbed.rehearsal/v0", "id": "private-attempt",
            "surface": { "system": system, "substrate": "host" }, "resources": ["program", "cwd"],
            "process": { "program": binary, "args": args, "cwd": self.root.path(), "env": { "PRIVATE": "private-environment" } },
            "limits": { "timeout_ms": if args == ["60"] { 100 } else { 5000 }, "output_bytes": 1048576 }
        });
        let mut command = self.command();
        command.args(["run", "--seat"]).arg(path);
        if cancel {
            let path = self.root.path().join("cancel");
            std::fs::write(&path, b"x").unwrap();
            command.arg("--cancel").arg(path);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        serde_json::to_writer(child.stdin.take().unwrap(), &input).unwrap();
        child.wait_with_output().unwrap()
    }

    #[cfg(unix)]
    pub(super) fn supervised(&self, cancel: bool) -> Output {
        self.launch(&["60"], cancel)
    }

    pub(super) fn records(&self) -> Vec<Value> {
        let mut atoms = Vec::new();
        for entry in std::fs::read_dir(self.seat().join("state/locus")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if name == "active.jsonl" || name.starts_with("sealed-") {
                let text = std::fs::read_to_string(path).unwrap();
                atoms.extend(
                    text.lines()
                        .map(|line| serde_json::from_str::<Value>(line).unwrap()),
                );
            }
        }
        atoms
    }
}

pub(super) fn frames(output: &Output) -> Vec<Value> {
    output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect()
}

pub(super) fn same(left: &Output, right: &Output) {
    let left = frames(left);
    let right = frames(right);
    assert_eq!(left.first(), right.first());
    let mut a = left.last().unwrap().clone();
    let mut b = right.last().unwrap().clone();
    a.as_object_mut().unwrap().remove("sequence");
    b.as_object_mut().unwrap().remove("sequence");
    assert_eq!(a, b);
    for stream in ["stdout", "stderr"] {
        let bytes = |frames: &[Value]| -> Vec<u64> {
            frames
                .iter()
                .filter(|frame| frame["stream"] == stream)
                .flat_map(|frame| {
                    frame["bytes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_u64().unwrap())
                })
                .collect()
        };
        assert_eq!(bytes(&left), bytes(&right));
    }
}
