use super::Input;
use serde_json::{json, to_vec};
use std::fs;
use std::path::Path;
use std::process::Command;

pub(super) fn pack(media: &Path, input: &Input<'_>, seal: &str) {
    let root = tempfile::tempdir().expect("VM media");
    fs::copy(env!("CARGO_BIN_EXE_ironbed"), root.path().join("ironbed")).expect("Ironbed media");
    fs::copy(input.hardrig, root.path().join("hardrig")).expect("Hardrig media");
    let mut ca = fs::read("/etc/ssl/certs/ca-certificates.crt").expect("host CA");
    ca.push(b'\n');
    ca.extend(fs::read(input.cert).expect("rehearsal CA"));
    fs::write(root.path().join("ca.pem"), ca).expect("CA media");
    copy(input.model, &root.path().join("model"));
    fs::write(root.path().join("seat.json"), descriptor(input, seal)).expect("seat descriptor");
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

fn descriptor(input: &Input<'_>, seal: &str) -> Vec<u8> {
    to_vec(&json!({
        "schema": "ironbed.seat/v0",
        "surface": {
            "system": "linux-x86_64",
            "substrate": "vm",
            "image": input.digest
        },
        "provider": {
            "identity": "root",
            "network": "user_nat",
            "resources": [
                {
                    "id": "hardrig",
                    "path": "/dut/hardrig",
                    "class": "read_only",
                    "digest": seal,
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
        "resources": ["hardrig", "model", "private-state", "ca-bundle"],
        "process": {
            "program": "/dut/hardrig",
            "args": args,
            "cwd": "/dut/model",
            "env": {
                "SSL_CERT_FILE": "/dut/ca.pem"
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
