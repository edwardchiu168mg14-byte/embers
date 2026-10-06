//! Settings: embedded defaults, user overrides, validation.

use regex::Regex;
use serde::Deserialize;

use crate::model::{Origin, Os};

const DEFAULTS: &str = include_str!("../defaults.toml");

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config is not valid TOML or has an unknown/mistyped key: {0}")]
    Parse(String),
    #[error("`{0}` must be greater than zero")]
    NotPositive(&'static str),
    #[error("invalid regex in `{field}`: {source}")]
    Regex {
        field: &'static str,
        #[source]
        source: Box<regex::Error>,
    },
}

/// Settings after defaults and user overrides are merged and validated.
/// Only obtainable through [`load_defaults`] or [`merge`], so regexes are always compiled.
#[derive(Debug, Clone)]
pub struct Config {
    /// The merged TOML this config was built from, so further overrides can layer on top.
    source: toml::Table,
    pub idle_minutes: u32,
    pub window_minutes: u32,
    pub hog_memory_bytes: u64,
    pub hog_cpu_pct: f32,
    pub hog_minutes: u32,
    pub origins: Vec<OriginPattern>,
    pub dev_tool_patterns: Vec<Regex>,
    pub protected: ProtectedLists,
    pub allowlist: Vec<AllowRule>,
    /// Non-fatal problems a front-end should show (e.g. an emptied protection list).
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OriginPattern {
    pub origin: Origin,
    pub exe: Option<Regex>,
    pub argv: Option<Regex>,
    pub session_root: bool,
}

#[derive(Debug, Clone)]
pub struct AllowRule {
    pub label: String,
    pub pattern: Regex,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedLists {
    pub names_macos: Vec<String>,
    pub names_windows: Vec<String>,
    pub names_linux: Vec<String>,
    pub path_prefixes_macos: Vec<String>,
    pub path_prefixes_windows: Vec<String>,
    pub path_prefixes_linux: Vec<String>,
    pub shells: Vec<String>,
}

impl ProtectedLists {
    pub fn names(&self, os: Os) -> &[String] {
        match os {
            Os::Macos => &self.names_macos,
            Os::Windows => &self.names_windows,
            Os::Linux => &self.names_linux,
        }
    }

    pub fn path_prefixes(&self, os: Os) -> &[String] {
        match os {
            Os::Macos => &self.path_prefixes_macos,
            Os::Windows => &self.path_prefixes_windows,
            Os::Linux => &self.path_prefixes_linux,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    idle_minutes: u32,
    window_minutes: u32,
    hog_memory_bytes: u64,
    hog_cpu_pct: f32,
    hog_minutes: u32,
    dev_tool_patterns: Vec<String>,
    origins: Vec<RawOrigin>,
    protected: ProtectedLists,
    #[serde(default)]
    allowlist: Vec<RawAllow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOrigin {
    origin: Origin,
    exe: Option<String>,
    argv: Option<String>,
    #[serde(default)]
    session_root: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAllow {
    label: String,
    pattern: String,
}

/// The built-in defaults. Panics only if the embedded file is broken, which tests catch.
pub fn load_defaults() -> Config {
    build(parse_table(DEFAULTS).expect("embedded defaults.toml parses"))
        .expect("embedded defaults.toml is valid")
}

/// Applies `user_toml` over `base`. A key the user sets replaces the base value
/// (inside `[protected]`, per sub-key). Unknown keys are errors, so typos are not ignored.
pub fn merge(base: Config, user_toml: &str) -> Result<Config, ConfigError> {
    let mut base = base.source;
    for (key, value) in parse_table(user_toml)? {
        match (key.as_str(), base.get_mut("protected"), value) {
            ("protected", Some(toml::Value::Table(dst)), toml::Value::Table(src)) => {
                dst.extend(src);
            }
            (_, _, value) => {
                base.insert(key, value);
            }
        }
    }
    build(base)
}

fn parse_table(text: &str) -> Result<toml::Table, ConfigError> {
    text.parse::<toml::Table>()
        .map_err(|e| ConfigError::Parse(e.to_string()))
}

fn build(table: toml::Table) -> Result<Config, ConfigError> {
    let raw: RawConfig = toml::Value::Table(table.clone())
        .try_into()
        .map_err(|e: toml::de::Error| ConfigError::Parse(e.to_string()))?;

    for (name, value) in [
        ("idle_minutes", u64::from(raw.idle_minutes)),
        ("window_minutes", u64::from(raw.window_minutes)),
        ("hog_memory_bytes", raw.hog_memory_bytes),
        ("hog_minutes", u64::from(raw.hog_minutes)),
    ] {
        if value == 0 {
            return Err(ConfigError::NotPositive(name));
        }
    }
    if !raw.hog_cpu_pct.is_finite() || raw.hog_cpu_pct <= 0.0 {
        return Err(ConfigError::NotPositive("hog_cpu_pct"));
    }

    let origins = raw
        .origins
        .into_iter()
        .map(|o| {
            if o.exe.is_none() && o.argv.is_none() {
                return Err(ConfigError::Parse(
                    "each [[origins]] entry needs `exe` or `argv`".into(),
                ));
            }
            Ok(OriginPattern {
                origin: o.origin,
                exe: o
                    .exe
                    .as_deref()
                    .map(|p| compile("origins", p))
                    .transpose()?,
                argv: o
                    .argv
                    .as_deref()
                    .map(|p| compile("origins", p))
                    .transpose()?,
                session_root: o.session_root,
            })
        })
        .collect::<Result<_, ConfigError>>()?;
    let dev_tool_patterns = raw
        .dev_tool_patterns
        .iter()
        .map(|p| compile("dev_tool_patterns", p))
        .collect::<Result<_, _>>()?;
    let allowlist = raw
        .allowlist
        .into_iter()
        .map(|a| {
            Ok(AllowRule {
                pattern: compile("allowlist", &a.pattern)?,
                label: a.label,
            })
        })
        .collect::<Result<_, ConfigError>>()?;

    let lists = &raw.protected;
    let warnings = [
        ("names_macos", &lists.names_macos),
        ("names_windows", &lists.names_windows),
        ("names_linux", &lists.names_linux),
        ("path_prefixes_macos", &lists.path_prefixes_macos),
        ("path_prefixes_windows", &lists.path_prefixes_windows),
        ("path_prefixes_linux", &lists.path_prefixes_linux),
        ("shells", &lists.shells),
    ]
    .into_iter()
    .filter(|(_, list)| list.is_empty())
    .map(|(name, _)| {
        format!("[protected] {name} is empty: Embers will not protect anything by that rule")
    })
    .collect();

    Ok(Config {
        source: table,
        warnings,
        idle_minutes: raw.idle_minutes,
        window_minutes: raw.window_minutes,
        hog_memory_bytes: raw.hog_memory_bytes,
        hog_cpu_pct: raw.hog_cpu_pct,
        hog_minutes: raw.hog_minutes,
        origins,
        dev_tool_patterns,
        protected: raw.protected,
        allowlist,
    })
}

fn compile(field: &'static str, pattern: &str) -> Result<Regex, ConfigError> {
    Regex::new(pattern).map_err(|source| ConfigError::Regex {
        field,
        source: Box::new(source),
    })
}
