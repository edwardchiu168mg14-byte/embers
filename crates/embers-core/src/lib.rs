//! Embers core: scans processes, classifies leftovers, and ends them on request.
#![forbid(unsafe_code)]

pub mod config;
pub mod fixture;
pub mod model;
pub mod origin;
pub mod protect;
pub mod reason;
pub mod redact;
pub mod rules;

pub use config::{load_defaults, merge, Config, ConfigError};
pub use model::*;
pub use rules::classify;

/// Version of the core library, taken from the workspace manifest.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_shaped() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(parts.len(), 3, "expected X.Y.Z, got {}", version());
        for part in parts {
            assert!(
                !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()),
                "expected X.Y.Z, got {}",
                version()
            );
        }
    }
}
