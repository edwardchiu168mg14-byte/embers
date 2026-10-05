use embers_core::config::{load_defaults, merge, ConfigError};

#[test]
fn defaults_load() {
    let cfg = load_defaults();
    assert_eq!(cfg.idle_minutes, 30);
    assert_eq!(cfg.hog_minutes, 10);
    assert!(!cfg.origins.is_empty() && !cfg.dev_tool_patterns.is_empty());
}

#[test]
fn user_overrides_apply_and_layer() {
    let cfg = merge(load_defaults(), "idle_minutes = 45").unwrap();
    assert_eq!(cfg.idle_minutes, 45);
    let cfg = merge(cfg, "hog_minutes = 3").unwrap();
    assert_eq!((cfg.idle_minutes, cfg.hog_minutes), (45, 3));
}

#[test]
fn protected_overrides_merge_per_key() {
    let cfg = merge(load_defaults(), "[protected]\nshells = [\"nu\"]").unwrap();
    assert_eq!(cfg.protected.shells, vec!["nu".to_string()]);
    assert!(cfg.protected.names_macos.iter().any(|n| n == "kernel_task"));
}

#[test]
fn unknown_key_is_an_error() {
    assert!(matches!(
        merge(load_defaults(), "idle_minuts = 5"),
        Err(ConfigError::Parse(_))
    ));
    assert!(matches!(
        merge(load_defaults(), "[protected]\nnamez = []"),
        Err(ConfigError::Parse(_))
    ));
}

#[test]
fn zero_or_negative_thresholds_are_errors() {
    assert!(matches!(
        merge(load_defaults(), "idle_minutes = 0"),
        Err(ConfigError::NotPositive("idle_minutes"))
    ));
    assert!(matches!(
        merge(load_defaults(), "hog_cpu_pct = -1.0"),
        Err(ConfigError::NotPositive("hog_cpu_pct"))
    ));
    assert!(merge(load_defaults(), "window_minutes = -5").is_err());
    assert!(merge(load_defaults(), "hog_cpu_pct = nan").is_err());
}

#[test]
fn bad_regex_is_an_error_not_a_panic() {
    let bad = "[[allowlist]]\nlabel = \"x\"\npattern = '('";
    assert!(matches!(
        merge(load_defaults(), bad),
        Err(ConfigError::Regex {
            field: "allowlist",
            ..
        })
    ));
}

#[test]
fn origin_without_pattern_is_rejected() {
    assert!(merge(load_defaults(), "[[origins]]\norigin = \"Terminal\"").is_err());
}

#[test]
fn infinite_threshold_is_an_error() {
    assert!(matches!(
        merge(load_defaults(), "hog_cpu_pct = inf"),
        Err(ConfigError::NotPositive("hog_cpu_pct"))
    ));
}

#[test]
fn emptied_protection_list_warns() {
    assert!(load_defaults().warnings.is_empty());
    let cfg = merge(load_defaults(), "[protected]\nnames_macos = []").unwrap();
    assert!(
        cfg.warnings.iter().any(|w| w.contains("names_macos")),
        "{:?}",
        cfg.warnings
    );
}
