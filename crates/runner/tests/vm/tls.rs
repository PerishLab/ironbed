use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

pub(super) struct Tls {
    child: Child,
    _root: tempfile::TempDir,
    pub(super) port: u16,
    pub(super) cert: PathBuf,
    pub(super) target: String,
}

impl Tls {
    pub(super) fn new(cube: u16) -> Self {
        let root = tempfile::tempdir().expect("TLS root");
        let ca = root.path().join("ca.pem");
        let cakey = root.path().join("ca.key");
        command(
            Command::new("openssl")
                .args([
                    "req",
                    "-x509",
                    "-newkey",
                    "rsa:2048",
                    "-noenc",
                    "-sha256",
                    "-days",
                    "1",
                    "-subj",
                    "/CN=Ironbed VM rehearsal CA",
                    "-addext",
                    "basicConstraints=critical,CA:TRUE",
                    "-addext",
                    "keyUsage=critical,keyCertSign",
                    "-keyout",
                ])
                .arg(&cakey)
                .arg("-out")
                .arg(&ca),
            "rehearsal CA",
        );
        let key = root.path().join("server.key");
        let request = root.path().join("server.csr");
        command(
            Command::new("openssl")
                .args([
                    "req",
                    "-new",
                    "-newkey",
                    "rsa:2048",
                    "-noenc",
                    "-sha256",
                    "-subj",
                    "/CN=10.0.2.100",
                    "-keyout",
                ])
                .arg(&key)
                .arg("-out")
                .arg(&request),
            "server request",
        );
        let policy = root.path().join("server.ext");
        fs::write(
            &policy,
            "subjectAltName=IP:10.0.2.100\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n",
        )
        .expect("server policy");
        let cert = root.path().join("server.pem");
        command(
            Command::new("openssl")
                .args(["x509", "-req", "-sha256", "-days", "1", "-in"])
                .arg(&request)
                .args(["-CA"])
                .arg(&ca)
                .args(["-CAkey"])
                .arg(&cakey)
                .args(["-CAcreateserial", "-extfile"])
                .arg(&policy)
                .arg("-out")
                .arg(&cert),
            "server certificate",
        );
        let listener = TcpListener::bind("127.0.0.1:0").expect("TLS port");
        let port = listener.local_addr().expect("TLS address").port();
        drop(listener);
        let mut child = Command::new("socat")
            .arg(format!(
                "OPENSSL-LISTEN:{port},bind=127.0.0.1,reuseaddr,fork,cert={},key={},verify=0",
                cert.display(),
                key.display()
            ))
            .arg(format!("TCP:127.0.0.1:{cube}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("TLS service");
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return Self {
                    child,
                    _root: root,
                    port,
                    cert: ca,
                    target: format!("https://10.0.2.100:{port}/"),
                };
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
        panic!("TLS service did not listen");
    }
}

impl Drop for Tls {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn command(command: &mut Command, purpose: &str) {
    let output = command.output().expect(purpose);
    assert!(output.status.success(), "{purpose}: {output:?}");
}
