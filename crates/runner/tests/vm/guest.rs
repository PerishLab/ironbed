use super::{FACTS, provider::Cleanup};
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct Guest {
    generation: String,
    child: Child,
    root: Option<tempfile::TempDir>,
    serial: std::path::PathBuf,
    control: std::path::PathBuf,
    channel: Option<UnixStream>,
    cube: u16,
    ssh: u16,
    private: std::path::PathBuf,
    done: bool,
}

pub(super) struct Spec<'a> {
    pub(super) code: &'a Path,
    pub(super) vars: &'a Path,
    pub(super) overlay: &'a Path,
    pub(super) state: &'a Path,
    pub(super) media: &'a Path,
    pub(super) descriptor: &'a Path,
    pub(super) seed: &'a Path,
    pub(super) serial: &'a Path,
    pub(super) cube: u16,
    pub(super) ssh: u16,
}

impl Guest {
    pub(super) fn new(name: &str, root: tempfile::TempDir, spec: Spec<'_>) -> Self {
        let control = root.path().join("control.sock");
        let network = format!(
            "user,restrict=on,id=net0,guestfwd=tcp:10.0.2.100:{}-cmd:/bin/nc -q 0 127.0.0.1 {},guestfwd=tcp:10.0.2.100:{}-cmd:/bin/nc -q 0 127.0.0.1 {}",
            spec.cube, spec.cube, spec.ssh, spec.ssh
        );
        let channel = format!(
            "socket,id=control,path={},server=on,wait=off",
            control.display()
        );
        let mut child = Command::new("qemu-system-x86_64")
            .args(["-name", &format!("ironbed-{name}")])
            .args(["-machine", "q35", "-accel", "tcg,thread=multi"])
            .args(["-cpu", "qemu64"])
            .args(["-smp", "2", "-m", "2048"])
            .args(["-drive", &flash(spec.code, 0, true)])
            .args(["-drive", &flash(spec.vars, 1, false)])
            .args(["-drive", &drive(spec.overlay, "qcow2", false)])
            .args(["-drive", &drive(spec.state, "raw", false)])
            .args(["-drive", &drive(spec.media, "raw", true)])
            .args(["-drive", &drive(spec.descriptor, "raw", true)])
            .args(["-drive", &drive(spec.seed, "raw", true)])
            .args(["-netdev", &network])
            .args(["-device", "virtio-net-pci,netdev=net0"])
            .args(["-chardev", &channel])
            .args(["-device", "virtio-serial-pci"])
            .args([
                "-device",
                "virtserialport,chardev=control,name=ironbed.cancel",
            ])
            .args(["-display", "none", "-no-reboot"])
            .arg("-serial")
            .arg(format!("file:{}", spec.serial.display()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("QEMU seat");
        let channel = connect(&control).unwrap_or_else(|| {
            let _ = child.kill();
            let _ = child.wait();
            panic!("VM cancellation channel did not listen");
        });
        Self {
            generation: name.to_string(),
            child,
            root: Some(root),
            serial: spec.serial.to_path_buf(),
            control,
            channel: Some(channel),
            cube: spec.cube,
            ssh: spec.ssh,
            private: spec.state.to_path_buf(),
            done: false,
        }
    }

    pub(super) fn generation(&self) -> &str {
        &self.generation
    }

    pub(super) fn finish(mut self) -> Cleanup {
        let status = wait(&mut self.child, &self.serial);
        assert!(
            status.success(),
            "QEMU failed: {}\n{}",
            status,
            fs::read_to_string(&self.serial).unwrap_or_default()
        );
        self.close()
    }

    pub(super) fn lose(mut self) -> Cleanup {
        self.child.kill().expect("kill QEMU seat");
        let status = self.child.wait().expect("reap lost QEMU seat");
        assert!(!status.success(), "lost QEMU seat exited successfully");
        self.close()
    }

    pub(super) fn cancel(mut self) -> Cleanup {
        self.channel
            .as_mut()
            .expect("VM cancellation channel")
            .write_all(b"cancel\n")
            .expect("cancel VM runner");
        let status = wait(&mut self.child, &self.serial);
        assert!(
            status.success(),
            "cancelled QEMU failed: {}\n{}",
            status,
            fs::read_to_string(&self.serial).unwrap_or_default()
        );
        self.close()
    }

    fn close(&mut self) -> Cleanup {
        assert!(clean(self.cube), "VM bridge remained for Cube");
        assert!(clean(self.ssh), "VM bridge remained for SSH");
        self.channel.take();
        assert!(
            UnixStream::connect(&self.control).is_err(),
            "VM cancellation channel remained"
        );
        assert!(
            self.private.is_file(),
            "consumer private medium was removed"
        );
        let root = self
            .root
            .as_ref()
            .expect("VM seat root")
            .path()
            .to_path_buf();
        self.root.take();
        assert!(!root.exists(), "VM attempt-local material remained");
        self.done = true;
        Cleanup::new(self.generation.clone(), FACTS)
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
        self.channel.take();
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

fn connect(path: &Path) -> Option<UnixStream> {
    for _ in 0..100 {
        if let Ok(stream) = UnixStream::connect(path) {
            return Some(stream);
        }
        thread::sleep(Duration::from_millis(10));
    }
    None
}
