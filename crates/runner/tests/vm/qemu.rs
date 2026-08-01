use super::{
    Input,
    guest::{Guest, Spec},
    media,
    provider::Cleanup,
};
use serde_json::Value;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct Qemu {
    base: PathBuf,
    code: PathBuf,
    media: PathBuf,
    state: PathBuf,
    root: tempfile::TempDir,
    pub(super) image: String,
    pub(super) seal: String,
    pub(super) origin: String,
    cube: u16,
    ssh: u16,
}

pub(super) struct Completed {
    pub(super) generation: String,
    pub(super) frames: Vec<Value>,
    pub(super) cleanup: Cleanup,
}

impl Qemu {
    pub(super) fn new(input: Input<'_>) -> Self {
        for tool in [
            "qemu-system-x86_64",
            "qemu-img",
            "cloud-localds",
            "genisoimage",
            "mkfs.ext4",
            "debugfs",
            "nc",
            "pgrep",
        ] {
            require(tool);
        }
        assert!(input.image.is_file(), "VM image must exist");
        assert!(input.hardrig.is_file(), "Hardrig binary must exist");
        assert!(
            !input.source.is_empty(),
            "VM image source must not be empty"
        );
        assert!(!input.origin.is_empty(), "Hardrig source must not be empty");
        let base = input.image.canonicalize().expect("VM image");
        let image = sha256(&base);
        assert_eq!(image, input.digest, "VM image digest");
        let seal = sha256(input.hardrig);
        let code = PathBuf::from("/usr/share/OVMF/OVMF_CODE_4M.fd");
        assert!(code.is_file(), "OVMF code must exist");
        let root = tempfile::tempdir().expect("VM rehearsal");
        let media = root.path().join("media.iso");
        media::pack(&media, &input);
        let state = root.path().join("state.ext4");
        File::create(&state)
            .expect("private state")
            .set_len(134_217_728)
            .expect("private state size");
        command(
            Command::new("mkfs.ext4")
                .args(["-q", "-F", "-L", "IRONSTATE", "-d"])
                .arg(input.state)
                .arg(&state),
            "private state format",
        );
        Self {
            base,
            code,
            media,
            state,
            root,
            image,
            seal,
            origin: input.origin.to_string(),
            cube: input.cube,
            ssh: input.ssh,
        }
    }

    pub(super) fn run(&self, name: &str, action: &str) -> Completed {
        let cleanup = self.spawn(name, action).finish();
        Completed {
            generation: name.to_string(),
            frames: self.frames(name),
            cleanup,
        }
    }

    pub(super) fn spawn(&self, name: &str, action: &str) -> Guest {
        assert!(
            name.bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "seat name"
        );
        assert!(matches!(action, "apply" | "plan"), "Hardrig action");
        let seat = tempfile::tempdir_in(self.root.path()).expect("VM seat");
        let descriptor = seat.path().join("seat.iso");
        media::seat(
            &descriptor,
            media::Descriptor {
                image: &self.image,
                seal: &self.seal,
                origin: &self.origin,
                generation: name,
            },
        );
        let vars = seat.path().join("vars.fd");
        fs::copy("/usr/share/OVMF/OVMF_VARS_4M.fd", &vars).expect("OVMF variables");
        let overlay = seat.path().join("root.qcow2");
        command(
            Command::new("qemu-img")
                .args(["create", "-q", "-f", "qcow2", "-F", "qcow2", "-b"])
                .arg(&self.base)
                .arg(&overlay),
            "root overlay",
        );
        let userdata = seat.path().join("user-data");
        let metadata = seat.path().join("meta-data");
        fs::write(&userdata, cloud(name, action)).expect("cloud user data");
        fs::write(
            &metadata,
            format!("instance-id: ironbed-{name}\nlocal-hostname: ironbed-{name}\n"),
        )
        .expect("cloud metadata");
        let seed = seat.path().join("seed.iso");
        command(
            Command::new("cloud-localds")
                .arg(&seed)
                .arg(&userdata)
                .arg(&metadata),
            "cloud seed",
        );
        let serial = seat.path().join("serial.log");
        Guest::new(
            name,
            seat,
            Spec {
                code: &self.code,
                vars: &vars,
                overlay: &overlay,
                state: &self.state,
                media: &self.media,
                descriptor: &descriptor,
                seed: &seed,
                serial: &serial,
                cube: self.cube,
                ssh: self.ssh,
            },
        )
    }

    pub(super) fn frames(&self, name: &str) -> Vec<Value> {
        let bytes = read(&self.state, &format!("/frames-{name}.ndjson"));
        bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).expect("Ironbed frame"))
            .collect()
    }

    pub(super) fn read(&self, path: &str) -> Vec<u8> {
        read(&self.state, path)
    }
}

fn cloud(name: &str, action: &str) -> String {
    format!(
        "#cloud-config\nbootcmd:\n  - [mkdir, -p, /state, /dut, /seat, /scratch]\nruncmd:\n  - [mount, -L, IRONSTATE, /state]\n  - [mount, -o, ro, -L, IRONMEDIA, /dut]\n  - [mount, -o, ro, -L, IRONSEAT, /seat]\n  - [sh, -c, 'while [ ! -e /dev/virtio-ports/ironbed.cancel ]; do sleep 0.1; done; /dut/ironbed run --seat /seat/seat.json --cancel /dev/virtio-ports/ironbed.cancel < /dut/{action}.json > /state/frames-{name}.ndjson 2> /state/stderr-{name}.log']\n  - [sync]\n  - [poweroff]\n"
    )
}

fn read(image: &Path, path: &str) -> Vec<u8> {
    let output = Command::new("debugfs")
        .args(["-R", &format!("cat {path}")])
        .arg(image)
        .output()
        .expect("private state read");
    assert!(output.status.success(), "{output:?}");
    assert!(
        !output.stdout.is_empty(),
        "private state path {path} is empty or missing: {output:?}"
    );
    output.stdout
}

fn sha256(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("digest");
    assert!(output.status.success(), "{output:?}");
    format!(
        "sha256:{}",
        String::from_utf8(output.stdout)
            .expect("digest")
            .split_whitespace()
            .next()
            .expect("digest")
    )
}

fn require(tool: &str) {
    command(
        Command::new("sh").args(["-c", "command -v \"$1\"", "sh", tool]),
        tool,
    );
}

fn command(command: &mut Command, purpose: &str) {
    let output = command.output().expect(purpose);
    assert!(output.status.success(), "{purpose}: {output:?}");
}
