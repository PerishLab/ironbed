#![cfg(unix)]

#[path = "container/docker.rs"]
mod docker;
#[path = "container/instance.rs"]
mod instance;
#[path = "hardrig/model.rs"]
mod model;
#[path = "hardrig/server.rs"]
mod server;

use docker::{Docker, bytes, code, result};
use instance::Instance;
use server::{Cube, sshd};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[test]
#[ignore = "requires Docker, a Linux image, a source-bound real Hardrig binary, sha256sum, sshd, and ssh-keygen"]
fn recovers() {
    let service = tempfile::tempdir().expect("service root");
    let (_sshd, port, user) = sshd(service.path());
    let cube = Cube::start(port, &user, Duration::ZERO);
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
    assert_eq!(frames[0]["provider"]["identity"], seat.user);
    assert_eq!(frames[0]["provider"]["network"], "host");
    assert_eq!(frames[0]["provider"]["resources"][2]["class"], "private");
    assert_eq!(frames[0]["provider"]["resources"][0]["digest"], seat.seal);
    assert_eq!(frames[0]["provider"]["resources"][0]["source"], seat.origin);
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

#[test]
#[ignore = "requires Docker, a Linux image, a source-bound real Hardrig binary, sha256sum, sshd, and ssh-keygen"]
fn loss() {
    let service = tempfile::tempdir().expect("service root");
    let (_sshd, port, user) = sshd(service.path());
    let mut cube = Cube::start(port, &user, Duration::from_secs(2));
    let desired = tempfile::tempdir().expect("model root");
    model::fixture(desired.path(), &cube.target, port, &user);
    let before = fs::read(desired.path().join("hosts/example/one/host.toml")).expect("model");
    let private = tempfile::tempdir().expect("private parent");
    let state = private.path().join("state");
    model::state(&state);
    let seat = Docker::new(&state);
    let instance = Instance::new("loss");

    let (_descriptor, child) = seat.spawn(desired.path(), &state, "apply", Some(instance.name()));
    cube.wait(4, Duration::from_secs(5));
    let seed = state.join("secrets/roots/liberte.top.json");
    assert!(seed.is_file());
    instance.remove();
    let (lost, frames) = result(child);
    assert!(!lost.status.success(), "{lost:?}");
    assert!(
        frames.iter().all(|frame| frame["kind"] != "finished"),
        "{frames:?}"
    );
    instance.absent();
    assert!(seed.is_file());

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

#[test]
#[ignore = "requires Docker, a Linux image, a source-bound real Hardrig binary, sha256sum, sshd, and ssh-keygen"]
fn cancels() {
    let service = tempfile::tempdir().expect("service root");
    let (_sshd, port, user) = sshd(service.path());
    let mut cube = Cube::start(port, &user, Duration::from_secs(2));
    let desired = tempfile::tempdir().expect("model root");
    model::fixture(desired.path(), &cube.target, port, &user);
    let before = fs::read(desired.path().join("hosts/example/one/host.toml")).expect("model");
    let private = tempfile::tempdir().expect("private parent");
    let state = private.path().join("state");
    model::state(&state);
    let seat = Docker::new(&state);
    let instance = Instance::new("cancel");

    let (_descriptor, child) = seat.spawn(desired.path(), &state, "apply", Some(instance.name()));
    cube.wait(4, Duration::from_secs(5));
    let seed = state.join("secrets/roots/liberte.top.json");
    assert!(seed.is_file());
    instance.cancel();
    let (cancelled, frames) = result(child);
    assert!(cancelled.status.success(), "{cancelled:?}");
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["process"]["termination"], "cancelled");
    assert_eq!(finished["process"]["code"], serde_json::Value::Null);
    assert_eq!(
        finished["cleanup"]["termination_signal"]["scope"],
        "process_group"
    );
    instance.absent();
    assert!(seed.is_file());

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
