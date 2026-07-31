use super::{Input, media};
use serde_json::Value;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

struct Seat<'a> {
    code: &'a Path,
    vars: &'a Path,
    overlay: &'a Path,
    state: &'a Path,
    media: &'a Path,
    seed: &'a Path,
    serial: &'a Path,
    cube: u16,
    ssh: u16,
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
        media::pack(&media, &input, &seal);
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

    pub(super) fn run(&self, name: &str, action: &str) -> Vec<Value> {
        assert!(
            name.bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "seat name"
        );
        assert!(matches!(action, "apply" | "plan"), "Hardrig action");
        let seat = tempfile::tempdir_in(self.root.path()).expect("VM seat");
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
        let mut child = launch(
            name,
            Seat {
                code: &self.code,
                vars: &vars,
                overlay: &overlay,
                state: &self.state,
                media: &self.media,
                seed: &seed,
                serial: &serial,
                cube: self.cube,
                ssh: self.ssh,
            },
        );
        reap(&mut child, &serial);
        clean(self.cube);
        clean(self.ssh);
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
        "#cloud-config\nbootcmd:\n  - [mkdir, -p, /state, /dut]\nruncmd:\n  - [mount, -L, IRONSTATE, /state]\n  - [mount, -o, ro, -L, IRONMEDIA, /dut]\n  - [sh, -c, '/dut/ironbed run --seat /dut/seat.json < /dut/{action}.json > /state/frames-{name}.ndjson 2> /state/stderr-{name}.log']\n  - [sync]\n  - [poweroff]\n"
    )
}

fn launch(name: &str, seat: Seat<'_>) -> Child {
    let network = format!(
        "user,restrict=on,id=net0,guestfwd=tcp:10.0.2.100:{}-cmd:/bin/nc -q 0 127.0.0.1 {},guestfwd=tcp:10.0.2.100:{}-cmd:/bin/nc -q 0 127.0.0.1 {}",
        seat.cube, seat.cube, seat.ssh, seat.ssh
    );
    Command::new("qemu-system-x86_64")
        .args(["-name", &format!("ironbed-{name}")])
        .args(["-machine", "q35", "-accel", "tcg,thread=multi"])
        .args(["-cpu", "qemu64"])
        .args(["-smp", "2", "-m", "2048"])
        .args(["-drive", &flash(seat.code, 0, true)])
        .args(["-drive", &flash(seat.vars, 1, false)])
        .args(["-drive", &drive(seat.overlay, "qcow2", false)])
        .args(["-drive", &drive(seat.state, "raw", false)])
        .args(["-drive", &drive(seat.media, "raw", true)])
        .args(["-drive", &drive(seat.seed, "raw", true)])
        .args(["-netdev", &network])
        .args(["-device", "virtio-net-pci,netdev=net0"])
        .args(["-display", "none", "-no-reboot"])
        .arg("-serial")
        .arg(format!("file:{}", seat.serial.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("QEMU seat")
}

fn drive(path: &Path, format: &str, readonly: bool) -> String {
    format!(
        "if=virtio,format={format},file={},readonly={}",
        path.display(),
        if readonly { "on" } else { "off" }
    )
}

fn flash(path: &Path, unit: u8, readonly: bool) -> String {
    format!(
        "if=pflash,format=raw,unit={unit},file={},readonly={}",
        path.display(),
        if readonly { "on" } else { "off" }
    )
}

fn reap(child: &mut Child, serial: &Path) {
    let start = Instant::now();
    loop {
        match child.try_wait().expect("QEMU status") {
            Some(status) => {
                assert!(
                    status.success(),
                    "QEMU failed: {}\n{}",
                    status,
                    fs::read_to_string(serial).unwrap_or_default()
                );
                return;
            }
            None if start.elapsed() < Duration::from_secs(600) => {
                thread::sleep(Duration::from_millis(250));
            }
            None => {
                child.kill().expect("kill expired QEMU seat");
                child.wait().expect("reap expired QEMU seat");
                panic!(
                    "QEMU seat expired\n{}",
                    fs::read_to_string(serial).unwrap_or_default()
                );
            }
        }
    }
}

fn clean(port: u16) {
    let pattern = format!("/bin/nc -q 0 127.0.0.1 {port}");
    for _ in 0..100 {
        let output = Command::new("pgrep")
            .args(["-f", &pattern])
            .output()
            .expect("bridge inspection");
        if !output.status.success() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("VM bridge remained for port {port}");
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
