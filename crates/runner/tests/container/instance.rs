use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) struct Instance {
    name: String,
}

impl Instance {
    pub(super) fn new(kind: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        Self {
            name: format!("ironbed-{kind}-{}-{nonce}", std::process::id()),
        }
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn remove(&self) {
        self.signal("KILL");
    }

    pub(super) fn cancel(&self) {
        self.signal("TERM");
    }

    pub(super) fn absent(&self) {
        for _ in 0..100 {
            let output = Command::new("docker")
                .args(["container", "inspect", &self.name])
                .output()
                .expect("inspect container seat");
            if !output.status.success() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("container seat {} was not removed", self.name);
    }

    fn signal(&self, signal: &str) {
        let output = Command::new("docker")
            .args(["kill", "--signal", signal, &self.name])
            .output()
            .expect("signal container seat");
        assert!(output.status.success(), "{output:?}");
    }
}
