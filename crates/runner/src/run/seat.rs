use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const SCHEMA: &str = "ironbed.seat/v0";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Seat {
    schema: String,
    pub(super) generation: String,
    pub(super) surface: Surface,
    pub(super) provider: Provider,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Surface {
    pub(super) system: String,
    pub(super) substrate: Substrate,
    #[serde(default)]
    image: Option<String>,
}

#[derive(Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Substrate {
    Host,
    Container,
    Vm,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Provider {
    identity: String,
    network: String,
    pub(super) resources: Vec<Resource>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resource {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) class: Class,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Class {
    #[serde(rename = "read_only")]
    Input,
    Private,
    Temporary,
}

pub(super) fn read(path: &Path) -> Result<Seat, String> {
    if !path.is_absolute() || !path.is_file() {
        return Err("seat must be one existing absolute file".to_string());
    }
    let bytes = fs::read(path).map_err(|error| format!("cannot read seat: {error}"))?;
    let seat: Seat =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid seat: {error}"))?;
    if seat.schema != SCHEMA {
        return Err(format!("seat schema must be {SCHEMA}"));
    }
    if seat.generation.is_empty() || seat.generation.len() > 128 {
        return Err("seat generation must contain 1 through 128 bytes".to_string());
    }
    if seat.surface.image.as_ref().is_some_and(String::is_empty) {
        return Err("provided image cannot be empty".to_string());
    }
    validate(&seat.provider)?;
    Ok(seat)
}

fn validate(provider: &Provider) -> Result<(), String> {
    if provider.identity.is_empty() {
        return Err("provider identity cannot be empty".to_string());
    }
    if provider.network.is_empty() {
        return Err("provider network cannot be empty".to_string());
    }
    let mut ids = BTreeSet::new();
    for resource in &provider.resources {
        if resource.id.is_empty() || resource.id.len() > 128 {
            return Err("resource id must contain 1 through 128 bytes".to_string());
        }
        if !ids.insert(&resource.id) {
            return Err(format!("resource id {} is duplicated", resource.id));
        }
        let path = Path::new(&resource.path);
        if !path.is_absolute() || !path.exists() {
            return Err(format!(
                "resource {} path must be one existing absolute path",
                resource.id
            ));
        }
        if resource.digest.as_ref().is_some_and(String::is_empty) {
            return Err(format!("resource {} digest cannot be empty", resource.id));
        }
        if resource.source.as_ref().is_some_and(String::is_empty) {
            return Err(format!("resource {} source cannot be empty", resource.id));
        }
    }
    Ok(())
}

impl Seat {
    pub(super) fn cover(&self, ids: &[String], program: &Path, cwd: &Path) -> Result<(), String> {
        let required = require(&self.provider.resources, ids)?;
        if !within(&self.provider.resources, &required, program)? {
            return Err("program must be covered by one required resource".to_string());
        }
        if !within(&self.provider.resources, &required, cwd)? {
            return Err("cwd must be covered by one required resource".to_string());
        }
        Ok(())
    }
}

fn require<'a>(resources: &'a [Resource], ids: &'a [String]) -> Result<BTreeSet<&'a str>, String> {
    let mut required = BTreeSet::new();
    for id in ids {
        if !required.insert(id.as_str()) {
            return Err(format!("required resource {id} is duplicated"));
        }
        if !resources.iter().any(|resource| resource.id == *id) {
            return Err(format!("required resource {id} was not provided"));
        }
    }
    Ok(required)
}

fn within(resources: &[Resource], ids: &BTreeSet<&str>, path: &Path) -> Result<bool, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))?;
    for resource in resources {
        if !ids.contains(resource.id.as_str()) {
            continue;
        }
        let root = Path::new(&resource.path)
            .canonicalize()
            .map_err(|error| format!("cannot resolve resource {} path: {error}", resource.id))?;
        if path.starts_with(root) {
            return Ok(true);
        }
    }
    Ok(false)
}
