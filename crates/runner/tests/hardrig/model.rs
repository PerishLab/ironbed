use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(super) fn fixture(root: &Path, target: &str, port: u16, user: &str) {
    let host = format!(
        r#"schema = "hardrig.host/v12"
id = "example.one"
resources = ["storage", "k3s"]

[provider]
kind = "cube"
target = "{target}"
credential = "example"
host = 42

[ssh]
dialect = "posix"
alias = "one.example"
user = "{user}"
port = {port}
identity = "ssh/keys/example.one"

[storage]
device = "/dev/vdb1"
path = "/mnt/k3s"
filesystem = "ext4"
options = ["defaults"]

[k3s]
version = "v1.0.0+k3s1"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
storage = "/mnt/k3s/local-path"
sans = []

[addresses.k3s]
source = "provider"
"#
    );
    fs::create_dir_all(root.join("hosts/example/one")).expect("host root");
    fs::write(root.join("hosts/example/one/host.toml"), host).expect("host model");
}

pub(super) fn state(root: &Path) {
    fs::create_dir(root).expect("state root");
    let cube = root.join("example.env");
    fs::write(
        &cube,
        "EXAMPLE_ACCOUNT=\"fixture\"\nEXAMPLE_API_KEY=\"fixture-secret\"\n",
    )
    .expect("Cube credential");
    mode(&cube);
    let dns = root.join("dnspod.env");
    fs::write(
        &dns,
        "TENCENT_SECRET_ID=\"fixture-id\"\nTENCENT_SECRET_KEY=\"fixture-key\"\n",
    )
    .expect("DNS credential");
    mode(&dns);
}

fn mode(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("credential mode");
}
