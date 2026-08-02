use super::fixture::{Export, Trial, launch, system};
use std::{fs, path::Path};

#[cfg(unix)]
#[test]
fn artifact() {
    let current = system();
    let target = tempfile::tempdir().expect("artifact target");
    let (status, frames) = launch(Trial {
        generation: "host-fixture",
        required: &current,
        provided: &current,
        program: Path::new("/bin/sh"),
        args: &["-c", "printf artifact > \"$IRONBED_ARTIFACT\""],
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 1_048_576,
        artifact: Some(Export {
            target: target.path(),
            limit: 8,
            name: "artifact",
        }),
    });
    assert!(status.success());
    let frame = frames
        .iter()
        .find(|frame| frame["kind"] == "artifact")
        .expect("artifact frame");
    assert_eq!(frame["artifact"]["id"], "artifact");
    assert_eq!(frame["artifact"]["name"], "artifact");
    assert_eq!(frame["artifact"]["target"], "artifact");
    assert_eq!(frame["artifact"]["bytes"], 8);
    assert_eq!(
        frame["artifact"]["digest"],
        "sha256:c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c"
    );
    assert_eq!(
        fs::read(target.path().join("artifact")).unwrap(),
        b"artifact"
    );
    assert!(
        fs::metadata(target.path().join("artifact"))
            .unwrap()
            .permissions()
            .readonly()
    );
    assert_eq!(frames[frames.len() - 2]["kind"], "artifact");
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["artifacts"]["complete"], true);
    assert_eq!(finished["artifacts"]["bytes"], 8);
    assert_eq!(finished["artifacts"]["limit_bytes"], 8);
}

#[cfg(unix)]
#[test]
fn independent() {
    let current = system();
    let target = tempfile::tempdir().expect("artifact target");
    let (status, frames) = launch(Trial {
        generation: "host-fixture",
        required: &current,
        provided: &current,
        program: Path::new("/bin/sh"),
        args: &[
            "-c",
            "printf artifact > \"$IRONBED_ARTIFACT\"; while :; do printf x; done",
        ],
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 16,
        artifact: Some(Export {
            target: target.path(),
            limit: 8,
            name: "artifact",
        }),
    });
    assert!(status.success());
    assert!(frames.iter().any(|frame| frame["kind"] == "artifact"));
    assert_eq!(
        fs::read(target.path().join("artifact")).unwrap(),
        b"artifact"
    );
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["process"]["termination"], "output_limit");
    assert_eq!(finished["artifacts"]["complete"], true);
}

#[cfg(unix)]
#[test]
fn immutable() {
    let current = system();
    let target = tempfile::tempdir().expect("artifact target");
    fs::write(target.path().join("artifact"), b"original").unwrap();
    let (status, frames) = launch(Trial {
        generation: "host-fixture",
        required: &current,
        provided: &current,
        program: Path::new("/bin/sh"),
        args: &["-c", "printf replacement > \"$IRONBED_ARTIFACT\""],
        resources: &["program", "cwd"],
        timeout: 5_000,
        output: 1_048_576,
        artifact: Some(Export {
            target: target.path(),
            limit: 11,
            name: "artifact",
        }),
    });
    assert!(status.success());
    assert!(!frames.iter().any(|frame| frame["kind"] == "artifact"));
    assert_eq!(
        fs::read(target.path().join("artifact")).unwrap(),
        b"original"
    );
    let finished = frames.last().expect("finished frame");
    assert_eq!(finished["artifacts"]["complete"], false);
    assert!(
        finished["artifacts"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("already exists"))
    );
}

#[cfg(unix)]
#[test]
fn names() {
    let current = system();
    for name in [
        "../artifact",
        "artifact/name",
        "artifact\\name",
        ".artifact",
        "artifact.",
        "artifact..bin",
        "con.json",
        "com1",
    ] {
        let target = tempfile::tempdir().expect("artifact target");
        let (status, frames) = launch(Trial {
            generation: "host-fixture",
            required: &current,
            provided: &current,
            program: Path::new("/bin/sh"),
            args: &["-c", "printf artifact > \"$IRONBED_ARTIFACT\""],
            resources: &["program", "cwd"],
            timeout: 5_000,
            output: 1_048_576,
            artifact: Some(Export {
                target: target.path(),
                limit: 8,
                name,
            }),
        });
        assert_eq!(status.code(), Some(2));
        assert!(frames.is_empty());
    }
}
