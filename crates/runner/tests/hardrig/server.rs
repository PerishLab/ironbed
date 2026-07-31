use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(super) struct Cube {
    pub(super) target: String,
    requests: Receiver<String>,
    handle: JoinHandle<()>,
    seen: Vec<String>,
}

pub(super) struct Sshd(Child);

struct Reply {
    body: String,
    wait: Duration,
}

impl Cube {
    pub(super) fn start(port: u16, user: &str, wait: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Cube listener");
        let target = format!("http://{}/", listener.local_addr().expect("Cube address"));
        let responses = responses(port, user, wait);
        let (sender, requests) = mpsc::channel();
        let handle = thread::spawn(move || {
            for reply in responses {
                let (mut stream, _) = listener.accept().expect("Cube connection");
                sender.send(receive(&stream)).expect("Cube request");
                thread::sleep(reply.wait);
                respond(&mut stream, &reply.body);
            }
        });
        Self {
            target,
            requests,
            handle,
            seen: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub(super) fn wait(&mut self, count: usize, timeout: Duration) {
        while self.seen.len() < count {
            self.seen.push(
                self.requests
                    .recv_timeout(timeout)
                    .expect("Cube request before deadline"),
            );
        }
    }

    pub(super) fn finish(mut self) -> Vec<String> {
        self.handle.join().expect("Cube server");
        self.seen.extend(self.requests);
        self.seen
    }
}

impl Drop for Sshd {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn responses(port: u16, user: &str, wait: Duration) -> Vec<Reply> {
    let login = json!({
        "jwt": "fixture.jwt",
        "status": 200,
        "msg": "login successful"
    })
    .to_string();
    let host = json!({
        "status": 200,
        "msg": "ok",
        "data": {
            "host": {
                "id": 42,
                "domain": "fixture",
                "domainstatus": "Active",
                "os": "Linux_x86_64",
                "dedicatedip": "127.0.0.1",
                "username": user,
                "port": port,
                "password": "unused"
            }
        }
    })
    .to_string();
    let power = json!({
        "status": 200,
        "msg": null,
        "data": {"status": "on", "des": "on"}
    })
    .to_string();
    let fence = json!({
        "status": 503,
        "msg": "rehearsal fence",
        "jwt": null
    })
    .to_string();
    [
        (login.clone(), Duration::ZERO),
        (host.clone(), Duration::ZERO),
        (power.clone(), Duration::ZERO),
        (fence, wait),
        (login, Duration::ZERO),
        (host, Duration::ZERO),
        (power, Duration::ZERO),
    ]
    .into_iter()
    .map(|(body, wait)| Reply { body, wait })
    .collect()
}

fn receive(stream: &TcpStream) -> String {
    let mut reader = BufReader::new(stream);
    let mut request = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("Cube header");
        request.push_str(&line);
        if line == "\r\n" {
            break;
        }
    }
    let size = request
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")?
                .parse()
                .ok()
        })
        .unwrap_or(0);
    let mut body = vec![0; size];
    reader.read_exact(&mut body).expect("Cube body");
    request.push_str(std::str::from_utf8(&body).expect("Cube UTF-8"));
    request
}

fn respond(stream: &mut TcpStream, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
}

pub(super) fn sshd(root: &Path) -> (Sshd, u16, String) {
    let key = root.join("host");
    let status = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&key)
        .status()
        .expect("ssh-keygen");
    assert!(status.success());
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve SSH port");
    let port = listener.local_addr().expect("SSH address").port();
    drop(listener);
    let user = std::env::var("USER").expect("current user");
    let config = root.join("sshd_config");
    fs::write(
        &config,
        format!(
            "ListenAddress 127.0.0.1:{port}\nHostKey {}\nPidFile {}\nPasswordAuthentication no\nPubkeyAuthentication no\nUsePAM no\nStrictModes no\nLogLevel ERROR\n",
            key.display(),
            root.join("sshd.pid").display()
        ),
    )
    .expect("sshd config");
    let mut child = Command::new("/usr/sbin/sshd")
        .args(["-D", "-e", "-f"])
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("sshd");
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return (Sshd(child), port, user);
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("sshd did not listen");
}
