//! Loads synthetic snapshots from JSON (tests and `embers scan --fixture`).
//!
//! The loader plays the sampler's role: it redacts and caps every command line before
//! the rules see it, exactly as a real sampler must.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::{self, Config, ConfigError};
use crate::model::{HostStates, Snapshot};
use crate::redact::{cap_field, redact_argv, redact_text, MAX_FIELD_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    #[serde(default)]
    pub prev: Option<Snapshot>,
    pub cur: Snapshot,
    #[serde(default)]
    pub hosts: HostStates,
    /// TOML overrides applied on top of the defaults for this fixture.
    #[serde(default)]
    pub config: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a valid fixture: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Config(#[from] ConfigError),
}

/// Fixture files under `path` (a file, or every `*.json` in a directory except
/// `*.expected.json`), sorted by name.
pub fn list(path: &Path) -> Result<Vec<PathBuf>, FixtureError> {
    let io = |source| FixtureError::Io {
        path: path.to_path_buf(),
        source,
    };
    if !path.is_dir() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(path)
        .map_err(io)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            name.ends_with(".json") && !name.ends_with(".expected.json")
        })
        .collect();
    files.sort();
    Ok(files)
}

pub fn load(path: &Path) -> Result<Fixture, FixtureError> {
    let text = std::fs::read_to_string(path).map_err(|source| FixtureError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut fixture: Fixture =
        serde_json::from_str(&text).map_err(|source| FixtureError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    for snap in fixture
        .prev
        .iter_mut()
        .chain(std::iter::once(&mut fixture.cur))
    {
        for p in &mut snap.procs {
            p.exe = cap_field(&redact_text(&p.exe), MAX_FIELD_BYTES);
            p.argv = redact_argv(&p.argv);
        }
    }
    Ok(fixture)
}

/// The fixture's config: defaults, then the fixture's own overrides.
pub fn config_for(fixture: &Fixture) -> Result<Config, ConfigError> {
    let defaults = config::load_defaults();
    match &fixture.config {
        Some(overrides) => config::merge(defaults, overrides),
        None => Ok(defaults),
    }
}
