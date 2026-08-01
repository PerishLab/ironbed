use super::Input;
use serde_json::{json, to_vec};
use std::fs;
use std::path::Path;
use std::process::Command;

pub(super) struct Descriptor<'a> {
    pub(super) image: &'a str,
    pub(super) seal: &'a str,
    pub(super) origin: &'a str,
    pub(super) generation: &'a str,
}

pub(super) fn pack(media: &Path, input: &Input<'_>) {
    let root = tempfile::tempdir().expect("VM media");
    fs::copy(env!("CARGO_BIN_EXE_ironbed"), root.path().join("ironbed")).expect("Ironbed media");
    fs::copy(input.hardrig, root.path().join("hardrig")).expect("Hardrig media");
    let mut ca = fs::read("/etc/ssl/certs/ca-certificates.crt").expect("host CA");
    ca.push(b'\n');
    ca.extend(fs::read(input.cert).expect("rehearsal CA"));
    fs::write(root.path().join("ca.pem"), ca).expect("CA media");
    copy(input.model, &root.path().join("model"));
    fs::write(root.path().join("apply.json"), attempt("apply")).expect("apply attempt");
    fs::write(root.path().join("plan.json"), attempt("plan")).expect("plan attempt");
    command(
        Command::new("genisoimage")
            .args(["-quiet", "-R", "-V", "IRONMEDIA", "-o"])
            .arg(media)
            .arg(root.path()),
        "read-only media",
    );
}

pub(super) fn seat(media: &Path, input: Descriptor<'_>) {
    let root = tempfile::tempdir().expect("VM seat descriptor");
    fs::write(root.path().join("seat.json"), descriptor(input)).expect("seat descriptor");
    command(
        Command::new("genisoimage")
            .args(["-quiet", "-R", "-V", "IRONSEAT", "-o"])
            .arg(media)
            .arg(root.path()),
        "seat descriptor media",
    );
}

fn copy(source: &Path, target: &Path) {
    fs::create_dir(target).expect("media directory");
    for item in fs::read_dir(source).expect("media source") {
        let item = item.expect("media item");
        let destination = target.join(item.file_name());
        if item.file_type().expect("media item type").is_dir() {
            copy(&item.path(), &destination);
        } else {
            fs::copy(item.path(), destination).expect("media file");
        }
    }
}

fn descriptor(input: Descriptor<'_>) -> Vec<u8> {
    to_vec(&json!({
        "schema": "ironbed.seat/v0",
        "generation": input.generation,
        "surface": {
            "system": "linux-x86_64",
            "substrate": "vm",
            "image": input.image
        },
        "provider": {
            "identity": "root",
            "network": "user_nat",
            "resources": [
                {
                    "id": "hardrig",
                    "path": "/dut/hardrig",
                    "class": "read_only",
                    "digest": input.seal,
                    "source": input.origin
                },
                {
                    "id": "model",
                    "path": "/dut/model",
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
                },
                {
                    "id": "attempt-temporary",
                    "path": "/scratch",
                    "class": "temporary"
                }
            ]
        }
    }))
    .expect("seat descriptor")
}

fn attempt(action: &str) -> Vec<u8> {
    let mut args = vec![
        "--root".to_string(),
        "/dut/model".to_string(),
        "--state".to_string(),
        "/state".to_string(),
        action.to_string(),
        "liberte.top".to_string(),
    ];
    if action == "apply" {
        args.push("--yes".to_string());
    }
    to_vec(&json!({
        "schema": "ironbed.rehearsal/v0",
        "id": format!("hardrig-vm-{action}"),
        "surface": {
            "system": "linux-x86_64",
            "substrate": "vm"
        },
        "resources": [
            "hardrig",
            "model",
            "private-state",
            "ca-bundle",
            "attempt-temporary"
        ],
        "process": {
            "program": "/dut/hardrig",
            "args": args,
            "cwd": "/dut/model",
            "env": {
                "SSL_CERT_FILE": "/dut/ca.pem",
                "TMPDIR": "/scratch"
            }
        },
        "limits": {
            "timeout_ms": 60000,
            "output_bytes": 1048576
        }
    }))
    .expect("attempt")
}

fn command(command: &mut Command, purpose: &str) {
    let output = command.output().expect(purpose);
    assert!(output.status.success(), "{purpose}: {output:?}");
}
