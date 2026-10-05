use embers_core::redact::{cap_field, redact_argv, MAX_FIELD_BYTES};

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn secrets_are_masked() {
    let out = redact_argv(&args(&[
        "tool",
        "--api-key",
        "sk-abcdef0123456789",
        "TOKEN=abc123",
        "--password=hunter2",
        "-H",
        "Authorization: Bearer xyz.token",
        "--verbose",
        "8080",
    ]));
    assert_eq!(out[2], "***");
    assert_eq!(out[3], "TOKEN=***");
    assert_eq!(out[4], "--password=***");
    assert_eq!(out[6], "Authorization: Bearer ***");
    assert_eq!(out[7], "--verbose");
    assert_eq!(out[8], "8080");
    let joined = out.join(" ");
    for secret in ["sk-abcdef", "abc123", "hunter2", "xyz.token"] {
        assert!(!joined.contains(secret), "{secret} leaked: {joined}");
    }
}

#[test]
fn split_authorization_header_is_masked() {
    let out = redact_argv(&args(&["curl", "-H", "Authorization:", "Bearer", "xyz"]));
    assert!(!out.join(" ").contains("xyz"), "{out:?}");
}

#[test]
fn bare_token_shapes_are_masked() {
    // Built at runtime so secret scanners don't flag this test file.
    let github = format!("ghp_{}", "0123456789abcdefghijABCDEFGHIJ");
    let aws = format!("AKIA{}", "Z7XQ4TPRN3KD2WMB");
    let script = format!("echo {github} && echo {aws}");
    let out = redact_argv(&args(&["bash", "-c", &script]));
    assert!(
        !out[2].contains(&github) && !out[2].contains(&aws),
        "{}",
        out[2]
    );
}

#[test]
fn long_fields_are_capped() {
    let long = "a".repeat(10 * 1024 * 1024);
    let out = redact_argv(&[long]);
    assert!(out[0].len() <= MAX_FIELD_BYTES);
    assert!(out[0].ends_with('…'));
}

#[test]
fn cap_respects_char_boundaries() {
    let text = "é".repeat(2000); // 2 bytes each
    let out = cap_field(&text, 2049);
    assert!(out.len() <= 2049 && out.ends_with('…'));
}

#[test]
fn markup_and_escapes_stay_plain_text() {
    let nasty = format!("<script>alert(1)</script>\u{1b}[31mred{}", "x".repeat(4000));
    let out = redact_argv(&[nasty]);
    assert!(out[0].starts_with("<script>alert(1)</script>\u{1b}[31mred"));
    assert!(out[0].len() <= MAX_FIELD_BYTES);
}

#[test]
fn token_look_alikes_inside_paths_are_kept() {
    let path = format!("/srv/AKIA{}/bin/tool", "PROJECTARCHIVE24");
    let out = redact_argv(&args(&[&path, "--models", "/opt/sk-learn-models/v2"]));
    assert_eq!(out[0], path);
    assert_eq!(out[2], "/opt/sk-learn-models/v2");
}
