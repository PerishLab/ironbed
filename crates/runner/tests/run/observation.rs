#[path = "observation/fixture.rs"]
mod fixture;

use fixture::Fixture;

const ENABLED: &str = "[locus]\nenabled = true\nendpoint = 'http://127.0.0.1:0'\n";

#[test]
fn disabled() {
    let fixture = Fixture::new();
    let absent = fixture.invoke(None);
    fixture.configure("[locus]\nenabled = false\n");
    let disabled = fixture.invoke(None);
    assert_eq!(absent.status.code(), Some(0));
    assert_eq!(absent.stdout, disabled.stdout);
    assert_eq!(absent.stderr, disabled.stderr);
    assert!(!fixture.seat().join("state/locus").exists());
}

#[test]
fn buffered() {
    let fixture = Fixture::new();
    fixture.configure(ENABLED);
    assert_eq!(fixture.invoke(None).status.code(), Some(0));
    for args in [vec!["--version"], vec!["unknown"]] {
        let result = fixture.invoke(Some(&args));
        assert_eq!(result.status.code(), Some(0));
        let frames = fixture::frames(&result);
        assert_eq!(frames.first().unwrap()["kind"], "started");
        assert_eq!(frames.last().unwrap()["kind"], "finished");
        assert_eq!(
            frames.last().unwrap()["cleanup"]["direct_process_reaped"],
            true
        );
        assert_eq!(
            frames.last().unwrap()["process"]["code"],
            if args[0] == "unknown" { 2 } else { 0 }
        );
    }
    let refused = fixture.refuse();
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
    let atoms = fixture.records();
    assert!(atoms.iter().all(|atom| atom["producer"] == "ironbed"));
    for command in ["idle", "run"] {
        assert!(
            atoms
                .iter()
                .any(|atom| atom["context"]["ironbed.command"] == command)
        );
    }
    for edge in ["enter", "return"] {
        assert_eq!(
            atoms
                .iter()
                .filter(|atom| atom["source"]["edge"] == edge)
                .count(),
            10
        );
    }
    assert!(
        atoms
            .iter()
            .all(|atom| atom["context"].get("ironbed.fault").is_none())
    );
    let text = serde_json::to_string(&atoms).unwrap();
    for private in [
        "private-attempt",
        "private-provider",
        "private-generation",
        "private-environment",
        "private-malformed",
        "127.0.0.1",
    ] {
        assert!(!text.contains(private));
    }
}

#[test]
fn isolated() {
    let fixture = Fixture::new();
    let baseline = fixture.invoke(Some(&["unknown"]));
    for config in [
        ENABLED,
        "[locus]\nenabled = 'private-setting'\n",
        "[locus]\nenabled = true\nendpoint = ''\n",
    ] {
        fixture.configure(config);
        let result = fixture.invoke(Some(&["unknown"]));
        assert_eq!(result.status.code(), baseline.status.code());
        fixture::same(&result, &baseline);
        assert!(!result.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("private-setting"));
        let refused = fixture.refuse();
        assert_eq!(refused.status.code(), Some(2));
        assert!(refused.stdout.is_empty());
    }
    let blocked = Fixture::new();
    blocked.configure(ENABLED);
    std::fs::write(blocked.seat().join("state"), "occupied").unwrap();
    let result = blocked.invoke(Some(&["unknown"]));
    assert_eq!(result.status.code(), baseline.status.code());
    assert!(!result.stderr.is_empty());
    assert_eq!(
        fixture::frames(&result).last().unwrap()["cleanup"]["direct_process_reaped"],
        true
    );
}

#[test]
fn parity() {
    let fixture = Fixture::new();
    let idle = fixture.invoke(None);
    let run = fixture.invoke(Some(&["unknown"]));
    let refused = fixture.refuse();
    fixture.configure(ENABLED);
    let observed = fixture.invoke(None);
    assert_eq!(observed.status.code(), idle.status.code());
    assert_eq!(observed.stdout, idle.stdout);
    let observed = fixture.invoke(Some(&["unknown"]));
    assert_eq!(observed.status.code(), run.status.code());
    fixture::same(&observed, &run);
    let observed = fixture.refuse();
    assert_eq!(observed.status.code(), refused.status.code());
    assert_eq!(observed.stdout, refused.stdout);
}

#[cfg(unix)]
#[test]
fn supervised() {
    let fixture = Fixture::new();
    fixture.configure(ENABLED);
    for cancel in [false, true] {
        let result = fixture.supervised(cancel);
        assert_eq!(result.status.code(), Some(0));
        let frames = fixture::frames(&result);
        let terminal = frames.last().unwrap();
        assert_eq!(terminal["kind"], "finished");
        assert_eq!(
            terminal["process"]["termination"],
            if cancel { "cancelled" } else { "timeout" }
        );
        assert_eq!(terminal["cleanup"]["direct_process_reaped"], true);
        assert_eq!(
            terminal["cleanup"]["termination_signal"]["scope"],
            "process_group"
        );
    }
    let atoms = fixture.records();
    for edge in ["enter", "return"] {
        assert_eq!(
            atoms
                .iter()
                .filter(|atom| atom["source"]["edge"] == edge)
                .count(),
            6
        );
    }
}
