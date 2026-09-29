use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(super) fn fixture(root: &Path, target: &str, port: u16, user: &str) {
    let host = include_str!("../fixture/hosts/example/one/host.toml")
        .replace("http://127.0.0.1:9/", target)
        .replace("user = \"root\"", &format!("user = \"{user}\""))
        .replace("port = 22", &format!("port = {port}"));
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
