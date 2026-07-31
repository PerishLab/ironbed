#![cfg(target_os = "linux")]

#[path = "vm/guest.rs"]
mod guest;
#[path = "vm/media.rs"]
mod media;
#[path = "hardrig/model.rs"]
mod model;
#[path = "vm/qemu.rs"]
mod qemu;
#[path = "hardrig/server.rs"]
mod server;
#[path = "vm/tls.rs"]
mod tls;

use qemu::Qemu;
use serde_json::Value;
use server::{Cube, Sshd, sshd};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use tls::Tls;

struct Input<'a> {
    image: &'a Path,
    source: &'a str,
    digest: &'a str,
    hardrig: &'a Path,
    origin: &'a str,
    model: &'a Path,
    state: &'a Path,
    cert: &'a Path,
    cube: u16,
    ssh: u16,
}

struct Lab {
    cube: Cube,
    desired: tempfile::TempDir,
    before: Vec<u8>,
    rig: Qemu,
    _tls: Tls,
    _sshd: Sshd,
    _service: tempfile::TempDir,
    _private: tempfile::TempDir,
}

impl Lab {
    fn new(wait: Duration) -> Self {
        let image = PathBuf::from(std::env::var_os("IRONBED_VM_IMAGE").expect("VM image"));
        let source = std::env::var("IRONBED_VM_IMAGE_SOURCE").expect("VM image source");
        let digest = std::env::var("IRONBED_VM_IMAGE_SHA256").expect("VM image digest");
        let hardrig =
            PathBuf::from(std::env::var_os("IRONBED_HARDRIG_BIN").expect("Hardrig binary"));
        let origin = std::env::var("IRONBED_HARDRIG_SOURCE").expect("Hardrig source identity");
        let service = tempfile::tempdir().expect("service root");
        let (sshd, port, user) = sshd(service.path());
        let mut cube = Cube::guest(port, &user, wait);
        let tls = Tls::new(cube.port);
        cube.target.clone_from(&tls.target);
        let desired = tempfile::tempdir().expect("model root");
        model::fixture(desired.path(), &cube.target, port, &user);
        let before = fs::read(desired.path().join("hosts/example/one/host.toml")).expect("model");
        let private = tempfile::tempdir().expect("private parent");
        let state = private.path().join("state");
        model::state(&state);
        let rig = Qemu::new(Input {
            image: &image,
            source: &source,
            digest: &digest,
            hardrig: &hardrig,
            origin: &origin,
            model: desired.path(),
            state: &state,
            cert: &tls.cert,
            cube: tls.port,
            ssh: port,
        });
        Self {
            cube,
            desired,
            before,
            rig,
            _tls: tls,
            _sshd: sshd,
            _service: service,
            _private: private,
        }
    }

    fn finish(self) {
        assert_eq!(
            fs::read(self.desired.path().join("hosts/example/one/host.toml")).expect("model"),
            self.before
        );
        assert_eq!(self.cube.finish().len(), 7);
    }
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
#[ignore = "requires QEMU, OVMF, cloud-localds, genisoimage, mkfs.ext4, debugfs, netcat, OpenSSL, socat, a verified Linux x86-64 cloud image, a source-bound Hardrig binary, sshd, and ssh-keygen"]
fn recovers() {
    let lab = Lab::new(Duration::ZERO);
    let first = lab.rig.run("first", "apply");
    assert_eq!(first[0]["surface"]["required"]["substrate"], "vm");
    assert_eq!(first[0]["surface"]["provided"]["substrate"], "vm");
    assert_eq!(first[0]["surface"]["provided"]["image"], lab.rig.image);
    assert_eq!(first[0]["surface"]["authority"]["substrate"], "provider");
    assert_eq!(first[0]["provider"]["identity"], "root");
    assert_eq!(first[0]["provider"]["network"], "user_nat");
    assert_eq!(first[0]["provider"]["resources"][0]["digest"], lab.rig.seal);
    assert_eq!(
        first[0]["provider"]["resources"][0]["source"],
        lab.rig.origin
    );
    assert_eq!(first[0]["provider"]["resources"][2]["class"], "private");
    assert_eq!(code(&first), Some(2));
    let stdout = String::from_utf8(bytes(&first, "stdout")).expect("Hardrig stdout");
    let stderr = String::from_utf8(bytes(&first, "stderr")).expect("Hardrig stderr");
    assert!(stdout.contains("applied: root seed"), "{stdout}\n{stderr}");
    assert!(!lab.rig.read("/secrets/roots/liberte.top.json").is_empty());

    let second = lab.rig.run("second", "plan");
    assert_eq!(code(&second), Some(0));
    let stdout = String::from_utf8(bytes(&second, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("observed resource.seed: ready"), "{stdout}");
    assert!(stdout.contains("observed session.ssh: change"), "{stdout}");
    lab.finish();
}

#[test]
#[ignore = "requires QEMU, OVMF, cloud-localds, genisoimage, mkfs.ext4, debugfs, netcat, OpenSSL, socat, a verified Linux x86-64 cloud image, a source-bound Hardrig binary, sshd, and ssh-keygen"]
fn loss() {
    let mut lab = Lab::new(Duration::from_secs(30));
    let guest = lab.rig.spawn("lost", "apply");
    lab.cube.wait(4, Duration::from_secs(600));
    thread::sleep(Duration::from_secs(6));
    guest.lose();

    let recovered = lab.rig.run("recovered", "plan");
    assert_eq!(code(&recovered), Some(0));
    let stdout = String::from_utf8(bytes(&recovered, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("observed resource.seed: ready"), "{stdout}");
    assert!(stdout.contains("observed session.ssh: change"), "{stdout}");

    let lost = lab.rig.frames("lost");
    assert_eq!(lost[0]["kind"], "started");
    assert!(lost.iter().all(|frame| frame["kind"] != "finished"));
    let stdout = String::from_utf8(bytes(&lost, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("applied: root seed"), "{stdout}");
    assert!(!lab.rig.read("/secrets/roots/liberte.top.json").is_empty());
    lab.finish();
}

#[test]
#[ignore = "requires QEMU, OVMF, cloud-localds, genisoimage, mkfs.ext4, debugfs, netcat, OpenSSL, socat, a verified Linux x86-64 cloud image, a source-bound Hardrig binary, sshd, and ssh-keygen"]
fn cancels() {
    let mut lab = Lab::new(Duration::from_secs(30));
    let guest = lab.rig.spawn("cancelled", "apply");
    lab.cube.wait(4, Duration::from_secs(600));
    guest.cancel();

    let cancelled = lab.rig.frames("cancelled");
    let finished = cancelled.last().expect("finished frame");
    assert_eq!(finished["process"]["termination"], "cancelled");
    assert_eq!(finished["process"]["code"], Value::Null);
    assert_eq!(
        finished["cleanup"]["termination_signal"]["scope"],
        "process_group"
    );
    let stdout = String::from_utf8(bytes(&cancelled, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("applied: root seed"), "{stdout}");

    let recovered = lab.rig.run("after-cancel", "plan");
    assert_eq!(code(&recovered), Some(0));
    let stdout = String::from_utf8(bytes(&recovered, "stdout")).expect("Hardrig stdout");
    assert!(stdout.contains("observed resource.seed: ready"), "{stdout}");
    assert!(stdout.contains("observed session.ssh: change"), "{stdout}");
    assert!(!lab.rig.read("/secrets/roots/liberte.top.json").is_empty());
    lab.finish();
}
