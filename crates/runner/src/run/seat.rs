use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const SCHEMA: &str = "ironbed.seat/v0";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Seat {
    schema: String,
    pub(super) surface: Surface,
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
    if seat.surface.image.as_ref().is_some_and(String::is_empty) {
        return Err("provided image cannot be empty".to_string());
    }
    Ok(seat)
}
