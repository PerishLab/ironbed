use super::seat::{Class, Resource, Seat};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) id: String,
    pub(super) source: String,
    pub(super) grant: String,
    pub(super) target: String,
}

pub(super) struct Item {
    id: String,
    target: String,
    bytes: u64,
    digest: String,
}

pub(super) struct Outcome {
    pub(super) items: Vec<Item>,
    pub(super) bytes: u64,
    pub(super) limit: u64,
    pub(super) failed: Option<String>,
}

struct Stage<'a> {
    source: &'a Path,
    temporary: &'a Path,
    destination: &'a Path,
    limit: u64,
    id: &'a str,
}

pub(super) fn validate(
    requests: &[Request],
    seat: &Seat,
    required: &[String],
    limit: u64,
) -> Result<(), String> {
    if requests.is_empty() {
        return (limit == 0)
            .then_some(())
            .ok_or_else(|| "artifact_bytes must be zero without artifacts".to_string());
    }
    if limit == 0 || limit > 1_073_741_824 {
        return Err("artifact_bytes must be 1 through 1073741824".to_string());
    }
    let mut ids = BTreeSet::new();
    for request in requests {
        if !atom(&request.id) {
            return Err("artifact id must be one lowercase hyphenated atom".to_string());
        }
        if !ids.insert(&request.id) {
            return Err(format!("artifact id {} is duplicated", request.id));
        }
        let source = Path::new(&request.source);
        if !absolute(source) {
            return Err(format!("artifact {} source must be absolute", request.id));
        }
        let grant = resource(seat, &request.grant, Class::Temporary)?;
        if !required.iter().any(|id| id == &request.grant) {
            return Err(format!(
                "artifact {} source grant must be required",
                request.id
            ));
        }
        if !source.starts_with(&grant.path) {
            return Err(format!(
                "artifact {} source must stay within its grant",
                request.id
            ));
        }
        resource(seat, &request.target, Class::Artifact)?;
        if required.iter().any(|id| id == &request.target) {
            return Err(format!(
                "artifact {} target must not be a process resource",
                request.id
            ));
        }
    }
    Ok(())
}

pub(super) fn collect(requests: &[Request], seat: &Seat, limit: u64) -> Outcome {
    let mut outcome = Outcome {
        items: Vec::new(),
        bytes: 0,
        limit,
        failed: None,
    };
    for request in requests {
        let remaining = limit.saturating_sub(outcome.bytes);
        match transfer(request, seat, remaining) {
            Ok(item) => {
                outcome.bytes += item.bytes;
                outcome.items.push(item);
            }
            Err(error) => {
                outcome.failed = Some(error);
                break;
            }
        }
    }
    outcome
}

pub(super) fn frame(attempt: &str, sequence: u64, item: &Item) -> Value {
    json!({
        "schema": "ironbed.frame/v0",
        "attempt": attempt,
        "sequence": sequence,
        "kind": "artifact",
        "artifact": {
            "id": item.id,
            "target": item.target,
            "bytes": item.bytes,
            "digest": item.digest
        }
    })
}

fn transfer(request: &Request, seat: &Seat, limit: u64) -> Result<Item, String> {
    let grant = resource(seat, &request.grant, Class::Temporary)?;
    let target = resource(seat, &request.target, Class::Artifact)?;
    let root = Path::new(&grant.path)
        .canonicalize()
        .map_err(|error| format!("cannot resolve artifact {} grant: {error}", request.id))?;
    let source = Path::new(&request.source)
        .canonicalize()
        .map_err(|error| format!("cannot resolve artifact {} source: {error}", request.id))?;
    if !source.starts_with(root) {
        return Err(format!("artifact {} source escaped its grant", request.id));
    }
    let metadata = fs::symlink_metadata(&source)
        .map_err(|error| format!("cannot inspect artifact {}: {error}", request.id))?;
    if !metadata.file_type().is_file() {
        return Err(format!("artifact {} source must be one file", request.id));
    }
    if metadata.len() > limit {
        return Err(format!("artifact {} crosses the byte limit", request.id));
    }
    let root = Path::new(&target.path)
        .canonicalize()
        .map_err(|error| format!("cannot resolve artifact {} target: {error}", request.id))?;
    let destination = root.join(&request.id);
    if destination.exists() {
        return Err(format!("artifact {} target already exists", request.id));
    }
    let temporary = root.join(format!(".{}.tmp", request.id));
    let result = copy(Stage {
        source: &source,
        temporary: &temporary,
        destination: &destination,
        limit,
        id: &request.id,
    });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    let (bytes, digest) = result?;
    Ok(Item {
        id: request.id.clone(),
        target: request.target.clone(),
        bytes,
        digest,
    })
}

fn copy(stage: Stage<'_>) -> Result<(u64, String), String> {
    let id = stage.id;
    let mut input = File::open(stage.source)
        .map_err(|error| format!("cannot open artifact {id} source: {error}"))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.temporary)
        .map_err(|error| format!("cannot stage artifact {id}: {error}"))?;
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| format!("cannot read artifact {id}: {error}"))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        if bytes > stage.limit {
            return Err(format!("artifact {id} crosses the byte limit"));
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| format!("cannot write artifact {id}: {error}"))?;
        hash.update(&buffer[..read]);
    }
    output
        .sync_all()
        .map_err(|error| format!("cannot sync artifact {id}: {error}"))?;
    let mut permissions = output
        .metadata()
        .map_err(|error| format!("cannot inspect staged artifact {id}: {error}"))?
        .permissions();
    permissions.set_readonly(true);
    output
        .set_permissions(permissions)
        .map_err(|error| format!("cannot seal artifact {id}: {error}"))?;
    drop(output);
    fs::hard_link(stage.temporary, stage.destination)
        .map_err(|error| format!("cannot publish artifact {id}: {error}"))?;
    fs::remove_file(stage.temporary)
        .map_err(|error| format!("cannot retire artifact {id} staging: {error}"))?;
    sync(stage.destination, id)?;
    let digest = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok((bytes, format!("sha256:{digest}")))
}

#[cfg(unix)]
fn sync(path: &Path, id: &str) -> Result<(), String> {
    File::open(path.parent().expect("artifact target parent"))
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot sync artifact {id} target: {error}"))
}

#[cfg(not(unix))]
fn sync(path: &Path, id: &str) -> Result<(), String> {
    let _ = (path, id);
    Ok(())
}

fn resource<'a>(seat: &'a Seat, id: &str, class: Class) -> Result<&'a Resource, String> {
    let resource = seat
        .provider
        .resources
        .iter()
        .find(|resource| resource.id == id)
        .ok_or_else(|| format!("artifact resource {id} was not provided"))?;
    if resource.class != class {
        return Err(format!("artifact resource {id} has the wrong class"));
    }
    Ok(resource)
}

fn atom(value: &str) -> bool {
    value.len() <= 128 && head(value) && tail(value)
}

fn head(value: &str) -> bool {
    matches!(value.bytes().next(), Some(b'a'..=b'z'))
}

fn tail(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && !bytes.windows(2).any(|pair| pair == b"--")
}

fn absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|part| {
            !matches!(
                part,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
}
