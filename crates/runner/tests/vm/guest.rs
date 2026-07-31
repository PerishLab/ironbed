use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct Guest {
    child: Child,
    root: Option<tempfile::TempDir>,
    serial: std::path::PathBuf,
    cube: u16,
    ssh: u16,
    done: bool,
}

pub(super) struct Spec<'a> {
    pub(super) code: &'a Path,
    pub(super) vars: &'a Path,
    pub(super) overlay: &'a Path,
    pub(super) state: &'a Path,
    pub(super) media: &'a Path,
    pub(super) seed: &'a Path,
    pub(super) serial: &'a Path,
    pub(super) cube: u16,
    pub(super) ssh: u16,
}

impl Guest {
    pub(super) fn new(name: &str, root: tempfile::TempDir, spec: Spec<'_>) -> Self {
        let network = format!(
            "user,restrict=on,id=net0,guestfwd=tcp:10.0.2.100:{}-cmd:/bin/nc -q 0 127.0.0.1 {},guestfwd=tcp:10.0.2.100:{}-cmd:/bin/nc -q 0 127.0.0.1 {}",
            spec.cube, spec.cube, spec.ssh, spec.ssh
        );
        let child = Command::new("qemu-system-x86_64")
            .args(["-name", &format!("ironbed-{name}")])
            .args(["-machine", "q35", "-accel", "tcg,thread=multi"])
            .args(["-cpu", "qemu64"])
            .args(["-smp", "2", "-m", "2048"])
            .args(["-drive", &flash(spec.code, 0, true)])
            .args(["-drive", &flash(spec.vars, 1, false)])
            .args(["-drive", &drive(spec.overlay, "qcow2", false)])
            .args(["-drive", &drive(spec.state, "raw", false)])
            .args(["-drive", &drive(spec.media, "raw", true)])
            .args(["-drive", &drive(spec.seed, "raw", true)])
            .args(["-netdev", &network])
            .args(["-device", "virtio-net-pci,netdev=net0"])
            .args(["-display", "none", "-no-reboot"])
            .arg("-serial")
            .arg(format!("file:{}", spec.serial.display()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("QEMU seat");
        Self {
            child,
            root: Some(root),
            serial: spec.serial.to_path_buf(),
            cube: spec.cube,
            ssh: spec.ssh,
            done: false,
        }
    }

    pub(super) fn finish(mut self) {
        let status = wait(&mut self.child, &self.serial);
        assert!(
            status.success(),
            "QEMU failed: {}\n{}",
            status,
            fs::read_to_string(&self.serial).unwrap_or_default()
        );
        self.close();
    }

    pub(super) fn lose(mut self) {
        self.child.kill().expect("kill QEMU seat");
        let status = self.child.wait().expect("reap lost QEMU seat");
        assert!(!status.success(), "lost QEMU seat exited successfully");
        self.close();
    }

    fn close(&mut self) {
        assert!(clean(self.cube), "VM bridge remained for Cube");
        assert!(clean(self.ssh), "VM bridge remained for SSH");
        self.root.take();
        self.done = true;
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = clean(self.cube);
        let _ = clean(self.ssh);
        self.root.take();
    }
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

fn wait(child: &mut Child, serial: &Path) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        match child.try_wait().expect("QEMU status") {
            Some(status) => return status,
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

fn clean(port: u16) -> bool {
    let pattern = format!("/bin/nc -q 0 127.0.0.1 {port}");
    for _ in 0..100 {
        let output = Command::new("pgrep")
            .args(["-f", &pattern])
            .output()
            .expect("bridge inspection");
        if !output.status.success() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}
