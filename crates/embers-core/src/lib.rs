//! Embers core: scans processes, classifies leftovers, and ends them on request.
//!
//! Phase 0 skeleton — no detection logic yet.

/// Version of the core library, taken from the workspace manifest.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_manifest() {
        assert_eq!(version(), "0.0.1");
    }
}
