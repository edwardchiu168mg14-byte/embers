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
    assert_eq!(out[6], "Authorization: ***");
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

// Phase 1a round 1: F2 (URL credentials), F3 (vocabulary), F4 (tails), S3 (ports).

/// Each case: one argv; none of the returned text may contain `SECRET`.
fn assert_masked(cases: &[Vec<String>]) {
    for argv in cases {
        let out = redact_argv(argv).join(" ");
        assert!(!out.contains("SECRET"), "leaked: {argv:?} -> {out}");
    }
}

fn v(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

#[test]
fn url_credentials_are_masked() {
    let pat = format!("ghp_{}", "a".repeat(36));
    assert_masked(&[
        v(&["srv", "https://user:SECRET13@example.com/x"]),
        v(&["srv", "postgres://admin:SECRET14@db:5432/app"]),
        v(&["srv", "DATABASE_URL=postgres://u:SECRET16@h/db"]),
        v(&["srv", "--url=https://x/sk-SECRET31abcdefgh"]),
    ]);
    let out = redact_argv(&v(&["git", &format!("https://{pat}@github.com/o/r")]));
    assert_eq!(out[1], "https://***@github.com/o/r");
    let out = redact_argv(&v(&["psql", "postgres://admin:pw@db:5432/app"]));
    assert_eq!(out[1], "postgres://admin:***@db:5432/app");
}

#[test]
fn url_without_password_is_kept() {
    let out = redact_argv(&v(&["git", "clone", "ssh://git@github.com/o/r.git"]));
    assert_eq!(out[2], "ssh://git@github.com/o/r.git");
}

#[test]
fn common_tool_secrets_are_masked() {
    let stripe = format!("sk_live_{}", "SECRET28abcdefgh");
    let gitlab = format!("glpat-{}", "SECRET29abcdefghij");
    let npm = format!("npm_{}", "SECRET30abcdefghijklmnopqrstuvwxyz0123");
    let slack = format!(
        "--webhook=https://hooks.slack.com/services/{}",
        "T000/B000/SECRET50"
    );
    assert_masked(&[
        v(&["mysql", "-pSECRET3"]),
        v(&["srv", "--pass=SECRET7"]),
        v(&["srv", "--pwd=SECRET8"]),
        v(&["srv", "MYSQL_PWD=SECRET9"]),
        v(&["srv", "--passphrase=SECRET6"]),
        v(&["srv", "pass=SECRET46"]),
        v(&["curl", "-u", "admin:SECRET17", "http://x"]),
        v(&["curl", "--user", "admin:SECRET18", "http://x"]),
        v(&["srv", "sshpass -p SECRET47 ssh x"]),
        v(&["srv", "bearer SECRET51"]),
        v(&["srv", &stripe]),
        v(&["srv", &gitlab]),
        v(&["srv", &npm]),
        v(&["srv", &slack]),
        v(&["srv", "--secret-access-key", "SECRET33"]),
        v(&["srv", "-pSECRET\u{200b}VAL43"]),
        v(&["srv", "--to\u{200b}ken=SECRET44"]),
    ]);
}

#[test]
fn user_without_password_is_kept() {
    let out = redact_argv(&v(&["curl", "--user", "alice", "http://x"]));
    assert_eq!(out[2], "alice");
    let out = redact_argv(&v(&["curl", "-u", "alice:pw"]));
    assert_eq!(out[2], "alice:***");
}

#[test]
fn whole_secret_is_masked_not_just_its_start() {
    assert_masked(&[
        v(&["bash", "-c", "API_KEY=\"SECRET VAL12 more\" srv"]),
        v(&["srv", "--password=p@ss&SECRET26"]),
        v(&["srv", "--token=ab;SECRET27"]),
        v(&["curl", "-H", "Authorization: token SECRET19"]),
        v(&["curl", "-H", "Authorization: Digest SECRET20"]),
        v(&[
            "bash",
            "-c",
            "curl -H 'Authorization: Bearer SECRET37' http://x",
        ]),
    ]);
    let out = redact_argv(&v(&["bash", "-c", "DB_PASSWORD='hunter2' srv"]));
    assert_eq!(out[2], "DB_PASSWORD='***' srv");
}

#[test]
fn port_numbers_are_not_secrets() {
    assert_eq!(redact_argv(&v(&["vite", "-p", "3000"]))[2], "3000");
    assert_eq!(redact_argv(&v(&["ssh", "-p", "22", "host"]))[2], "22");
    assert_eq!(redact_argv(&v(&["srv", "--port", "8080"]))[2], "8080");
    assert_eq!(
        redact_argv(&v(&["bash", "-c", "vite -p 3000"]))[2],
        "vite -p 3000"
    );
    assert_eq!(redact_argv(&v(&["mysql", "-p", "hunter2"]))[2], "***");
}
