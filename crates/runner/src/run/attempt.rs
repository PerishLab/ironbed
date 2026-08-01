use super::{
    artifact::{self, Request},
    seat::{Seat, Substrate},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::Path;

const SCHEMA: &str = "ironbed.rehearsal/v0";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Attempt {
    schema: String,
    pub(super) id: String,
    pub(super) surface: Requirement,
    pub(super) resources: Vec<String>,
    #[serde(default)]
    pub(super) artifacts: Vec<Request>,
    pub(super) process: Process,
    pub(super) limits: Limits,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Requirement {
    pub(super) system: String,
    pub(super) substrate: Substrate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Process {
    pub(super) program: String,
    pub(super) args: Vec<String>,
    pub(super) cwd: String,
    #[serde(default)]
    pub(super) env: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Limits {
    #[serde(rename = "timeout_ms")]
    pub(super) timeout: u64,
    #[serde(rename = "output_bytes")]
    pub(super) output: u64,
    #[serde(default, rename = "artifact_bytes")]
    pub(super) artifact: u64,
}

impl Attempt {
    pub(super) fn read() -> Result<Self, String> {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read attempt: {error}"))?;
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid attempt: {error}"))
    }

    pub(super) fn validate(&self, seat: &Seat) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("attempt schema must be {SCHEMA}"));
        }
        if self.id.is_empty() || self.id.len() > 128 {
            return Err("attempt id must contain 1 through 128 bytes".to_string());
        }
        let program = Path::new(&self.process.program);
        if !program.is_absolute() || !program.is_file() {
            return Err("program must be one existing absolute file".to_string());
        }
        let cwd = Path::new(&self.process.cwd);
        if !cwd.is_absolute() || !cwd.is_dir() {
            return Err("cwd must be one existing absolute directory".to_string());
        }
        if self.process.env.keys().any(|key| key.is_empty()) {
            return Err("environment keys cannot be empty".to_string());
        }
        if self.limits.timeout == 0 || self.limits.timeout > 86_400_000 {
            return Err("timeout_ms must be 1 through 86400000".to_string());
        }
        if self.limits.output == 0 || self.limits.output > 1_073_741_824 {
            return Err("output_bytes must be 1 through 1073741824".to_string());
        }
        artifact::validate(&self.artifacts, seat, &self.resources, self.limits.artifact)?;
        if self.surface.system != seat.surface.system {
            return Err(format!(
                "required system {} does not match provided {}",
                self.surface.system, seat.surface.system
            ));
        }
        if self.surface.substrate != seat.surface.substrate {
            return Err("required substrate does not match provided substrate".to_string());
        }
        seat.cover(&self.resources, program, cwd)
    }
}
