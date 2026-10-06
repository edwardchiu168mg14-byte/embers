//! Fixture-driven tests of the rule engine. `*.expected.json` beside each fixture is the spec.

use std::collections::HashSet;
use std::path::PathBuf;

use embers_core::fixture::{self, Fixture};
use embers_core::rules::Context;
use embers_core::{classify, Action, Ember, Pid, ProtectReason};
use serde::Deserialize;

#[derive(Deserialize)]
struct Expected {
    embers: Vec<ExpectedEmber>,
    protected_pids: Vec<Pid>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash)]
struct ExpectedEmber {
    root_pid: Pid,
    categories: Vec<String>,
    confidence: String,
    action: String,
    /// When given, the ember's pids must be exactly this set.
    #[serde(default)]
    pids: Option<Vec<Pid>>,
}

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn load(name: &str) -> Fixture {
    fixture::load(&path(&format!("{name}.json"))).expect("fixture loads")
}

fn run(fx: &Fixture) -> Vec<Ember> {
    let cfg = fixture::config_for(fx).expect("fixture config is valid");
    classify(fx.prev.as_ref(), &fx.cur, &cfg, &fx.hosts)
}

fn summary(e: &Ember) -> ExpectedEmber {
    let s = |v: &dyn erased::Debugish| v.text();
    ExpectedEmber {
        root_pid: e.root_pid,
        categories: e.categories.iter().map(|c| s(c)).collect(),
        confidence: s(&e.confidence),
        action: s(&e.action),
        pids: None,
    }
}

mod erased {
    pub trait Debugish {
        fn text(&self) -> String;
    }
    impl<T: std::fmt::Debug> Debugish for T {
        fn text(&self) -> String {
            format!("{self:?}")
        }
    }
}

/// Runs a fixture and checks it against its `.expected.json`; returns the embers.
fn check(name: &str) -> Vec<Ember> {
    let fx = load(name);
    let embers = run(&fx);
    let text =
        std::fs::read_to_string(path(&format!("{name}.expected.json"))).expect("expected file");
    let expected: Expected = serde_json::from_str(&text).expect("expected json");

    for want in &expected.embers {
        if let Some(pids) = &want.pids {
            let e = embers.iter().find(|e| e.root_pid == want.root_pid);
            let mut got = e.map(|e| e.pids.clone()).unwrap_or_default();
            got.sort_unstable();
            assert_eq!(&got, pids, "{name}: pids of ember {} differ", want.root_pid);
        }
    }
    let got: HashSet<ExpectedEmber> = embers.iter().map(summary).collect();
    let want: HashSet<ExpectedEmber> = expected
        .embers
        .into_iter()
        .map(|e| ExpectedEmber { pids: None, ..e })
        .collect();
    assert_eq!(got, want, "{name}: embers differ\n{embers:#?}");

    for e in embers.iter().filter(|e| e.action != Action::Inform) {
        for pid in &expected.protected_pids {
            assert!(
                !e.pids.contains(pid),
                "{name}: protected pid {pid} is in actionable ember {}",
                e.id
            );
        }
        assert!(
            e.protected.is_none(),
            "{name}: actionable ember {} is marked protected",
            e.id
        );
    }
    embers
}

#[test]
fn claude_desktop_idle_ends_whole_session_group() {
    let embers = check("claude_desktop_idle");
    let e = &embers[0];
    assert_eq!(e.pids, vec![1000, 1001, 1002, 1003, 1004]);
    assert!(e.reason.contains("idle"), "{}", e.reason);
    assert!(e.reason.contains("host says idle"), "{}", e.reason);
    assert!(e
        .recovery_hint
        .as_deref()
        .unwrap_or_default()
        .contains("--resume sess-1111"));
    assert_eq!(e.id, "1001-1799989200000");
}

#[test]
fn claude_desktop_busy_is_protected() {
    check("claude_desktop_busy");
    let fx = load("claude_desktop_busy");
    let cfg = fixture::config_for(&fx).unwrap();
    let ctx = Context::new(fx.prev.as_ref(), &fx.cur, &cfg, &fx.hosts);
    assert_eq!(ctx.protection(1001), Some(&ProtectReason::BusySession));
}

#[test]
fn recently_idle_session_is_still_protected() {
    let mut fx = load("claude_desktop_idle");
    fx.hosts.sessions[0].status_since_ms = Some(fx.cur.taken_at_ms - 60_000);
    assert!(run(&fx).is_empty());
}

#[test]
fn claude_cli_without_host_file_is_medium() {
    let embers = check("claude_cli_no_hostfile");
    assert!(
        embers[0].reason.contains("no transcript activity"),
        "{}",
        embers[0].reason
    );
}

#[test]
fn claude_cli_using_cpu_is_not_idle() {
    let mut fx = load("claude_cli_no_hostfile");
    let p = fx.cur.procs.iter_mut().find(|p| p.pid == 702).unwrap();
    p.cpu_time_ms += 15_000; // 5 % of a 5-minute window
    assert!(run(&fx).is_empty());
}

#[test]
fn orphan_http_server_redacts_secret_in_hint() {
    let embers = check("orphan_http_server");
    let hint = embers[0].recovery_hint.clone().unwrap();
    assert!(hint.contains("http.server 8765"), "{hint}");
    assert!(!hint.contains("sk-"), "{hint}");
    assert!(embers[0].reason.contains("parent gone") && embers[0].reason.contains(":8765"));
}

#[test]
fn idle_vite_server() {
    let embers = check("idle_vite_server");
    assert!(
        embers[0].reason.contains("session ended"),
        "{}",
        embers[0].reason
    );
}

#[test]
fn poll_loop_includes_its_sleep_child() {
    let embers = check("poll_loop");
    assert_eq!(embers[0].pids, vec![1400, 1401]);
}

#[test]
fn runaway_system_app_is_inform_only() {
    let embers = check("runaway_mirroring");
    assert!(embers[0].reason.contains("30 GB"), "{}", embers[0].reason);
    assert!(embers.iter().all(|e| e.action == Action::Inform));
}

#[test]
fn protected_processes_never_actionable() {
    let embers = check("protected_never");
    assert_eq!(embers[0].protected, Some(ProtectReason::Frontmost));
}

#[test]
fn windows_pid_reuse_orphan() {
    check("windows_pid_reuse");
}

#[test]
fn allowlisted_process_never_shown() {
    let embers = check("allowlist");
    assert!(embers
        .iter()
        .all(|e| e.root_pid != 1700 && !e.pids.contains(&1700)));
}

#[test]
fn single_snapshot_never_judges_idleness() {
    check("single_snapshot");
}

#[test]
fn runaway_plain_processes_can_be_ended() {
    let embers = check("runaway_plain_process");
    let cpu = embers.iter().find(|e| e.root_pid == 1801).unwrap();
    assert!(cpu.reason.contains("150 % CPU"), "{}", cpu.reason);
}

#[test]
fn cpu_hog_needs_a_long_enough_window() {
    let mut fx = load("runaway_plain_process");
    let prev = fx.prev.as_mut().unwrap();
    prev.taken_at_ms = fx.cur.taken_at_ms - 60_000; // 1 minute < hog_minutes
    for p in &mut prev.procs {
        if p.pid == 1801 {
            p.cpu_time_ms = fx
                .cur
                .procs
                .iter()
                .find(|c| c.pid == 1801)
                .unwrap()
                .cpu_time_ms
                - 90_000;
        }
    }
    assert!(run(&fx).iter().all(|e| e.root_pid != 1801));
}

#[test]
fn every_fixture_matches_its_expectation() {
    let files = fixture::list(&path("")).unwrap();
    assert_eq!(files.len(), 30);
    for file in files {
        let name = file.file_stem().unwrap().to_str().unwrap().to_string();
        check(&name);
    }
}

// Phase 1a round 1 — protection covers whole subtrees (F1), process groups only when safe (F6).

#[test]
fn allowlisted_server_keeps_its_worker() {
    check("a1_allowlisted_child_grandchild");
}

#[test]
fn busy_nested_session_keeps_its_helper() {
    check("a2_nested_busy_session");
}

#[test]
fn frontmost_app_keeps_its_children() {
    check("a3c_frontmost_descendant");
}

#[test]
fn other_users_process_keeps_its_children() {
    check("a7_notowned");
}

#[test]
fn protected_system_process_keeps_its_children() {
    check("a16_linux_sysd");
}

#[test]
fn unrelated_group_member_blocks_group_kill() {
    check("a3b_pgid_unrelated");
}

#[test]
fn pgid_zero_never_group_killed() {
    check("a13_pgid0");
}

#[test]
fn protected_group_leader_blocks_group_kill() {
    check("a19_leader_tty");
    check("a19b_leader_frontmost");
}

#[test]
fn allowlisted_memory_hog_never_shown() {
    check("allowlisted_memory_hog");
}

// F5 — a system binary is never offered for ending, whatever the rules say.
#[test]
fn system_path_process_is_inform_only() {
    let embers = check("a9_syspath_e2_e5");
    assert!(embers[0].reason.contains("Embers will not end it"));
    check("a9b_syspath_e2");
}

// S1 — a session status older than the process is ignored.
#[test]
fn stale_session_state_is_ignored() {
    check("a17_stale_state_new_proc");
}

// F7 — host-supplied hint fields are redacted, capped and quoted.
#[test]
fn hint_fields_are_redacted_capped_and_quoted() {
    let mut fx = load("a18_hint_fields");
    fx.hosts.sessions[0].cwd = Some(format!(
        "/work/api_key=SECRETCWD1/{}",
        "d".repeat(5_000_000)
    ));
    let embers = run(&fx);
    let hint = embers[0].recovery_hint.as_deref().unwrap();
    assert!(hint.len() <= 2048, "hint is {} bytes", hint.len());
    assert!(
        !hint.contains("SECRETCWD1") && !hint.contains("SECRETSID1"),
        "{hint}"
    );
    assert!(
        hint.contains("--resume '"),
        "session id with spaces must be quoted: {hint}"
    );
}

#[test]
fn joined_command_hint_is_capped() {
    let mut fx = load("orphan_http_server");
    let p = fx.cur.procs.iter_mut().find(|p| p.pid == 1200).unwrap();
    p.argv = std::iter::once("python3".to_string())
        .chain((0..3000).map(|_| "b".repeat(2000)))
        .collect();
    let embers = run(&fx);
    assert!(embers[0].recovery_hint.as_ref().unwrap().len() <= 2048);
}

// F10 — the reason is never empty.
#[test]
fn many_ports_still_give_a_reason() {
    let embers = check("a10_e3_manyports");
    assert!(
        embers[0]
            .reason
            .starts_with("listening on 40 ports (5000, 5001, 5002, …)"),
        "{}",
        embers[0].reason
    );
}

#[test]
fn classify_1000_processes_is_fast() {
    let mut fx = load("claude_desktop_idle");
    let template = fx.cur.procs[3].clone();
    for i in 0..1000u32 {
        let mut p = template.clone();
        p.pid = 50_000 + i;
        p.ppid = Some(if i == 0 { 1001 } else { 50_000 + i / 2 });
        fx.cur.procs.push(p);
    }
    let cfg = fixture::config_for(&fx).unwrap();
    // Warm-up run: compiles the lazily-built regexes outside the timed region.
    let _ = classify(fx.prev.as_ref(), &fx.cur, &cfg, &fx.hosts);
    let start = std::time::Instant::now();
    let _ = classify(fx.prev.as_ref(), &fx.cur, &cfg, &fx.hosts);
    // Generous on purpose: this only catches gross regressions (e.g. quadratic walks) in a
    // debug build next to parallel tests. The real budget check is `--self-stats` (phase 1b).
    assert!(
        start.elapsed().as_millis() < 250,
        "took {:?}",
        start.elapsed()
    );
}

// Round 2 (re-review N3, N4, N9).

#[test]
fn app_bundle_never_leads_a_group_kill() {
    check("b1_bundle_leader");
}

#[test]
fn protection_reaches_any_depth() {
    let embers = check("d70_deep_frontmost_chain");
    assert_eq!(embers[0].protected, Some(ProtectReason::Frontmost));
}

#[test]
fn quoted_hint_keeps_its_closing_quote() {
    let embers = check("a18_hint_fields");
    let hint = embers[0].recovery_hint.as_deref().unwrap();
    assert!(hint.ends_with("')"), "{hint}");
}

#[test]
fn os_root_protection_is_not_inherited() {
    check("j1_child_of_launchd_still_actionable");
}
