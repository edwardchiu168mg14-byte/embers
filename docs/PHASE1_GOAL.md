# Phase 1 Executor Goal — core engine + CLI

Issued: 2026-10-05 by the tech lead. Tier: **Full**. Base: `main` @ `f0c56ec` (Phase 0 merged).
References: `docs/PRE_DESIGN.md` §6 (detection), §8.4 (budget), §10 (code-level), Phase 1 section; `docs/ARCHITECTURE_DECISION.md` §3.0 rules **S7, S8, S10, S11** (binding here) and S2/S13/S14 (already enforced by CI); `docs/reviews/phase0_round1_arbitration.md` "Process changes".

## Why Phase 1 is split

PRE_DESIGN's Phase 1 would be ~2.5–3 k lines in one review. It is split into three sub-phases, each independently reviewable (target ≤ ~800 lines of diff excluding fixtures), each ending with a Sonnet review → arbitration → owner go.

| Sub-phase | Scope | OS calls? | This round |
|---|---|---|---|
| **1a** | Model, config, redaction, **pure rule engine** on synthetic snapshots; `embers scan --fixture <file>` | **None** (pure Rust, identical on 3 OSes) | **← do now** |
| 1b | macOS sampler (libproc/sysctl), Claude/Codex host adapters, `scan`/`watch --json`/`explain`, `--self-stats`, budget measurement | macOS read-only | next |
| 1c | `extinguish` (SIGTERM→wait→SIGKILL, process-group, tree, pre-flight), Windows sampler + action compiling on the windows runner with PID-reuse tests | macOS write (kill), Windows read | after 1b |

Rules that apply to all three: executor = code + tests only; **never stage a file outside the sub-phase's allowed tree** (ask first); branch + PR is a handoff artifact, executor never merges; every report has the two lists **"Owner actions outstanding"** and **"Not yet exercised in CI"**; no `extinguish` against a real process until 1c's human QA, with the owner present.

---

## Phase 1a — pure engine on fixtures (this round)

### Goal (one sentence)
Given two synthetic process snapshots and a config, `embers-core` returns the correct list of embers (category, origin, confidence, human-readable reason, suggested action, recovery hint) and never returns a protected process — proven by fixture tests on ubuntu/macos/windows — and `embers scan --fixture <path>` prints that list as text or NDJSON.

### Exact module / file structure (all new unless noted)

```
crates/embers-core/Cargo.toml              # MODIFY: add serde, serde_json, toml, regex, thiserror (see "Dependencies")
crates/embers-core/src/lib.rs              # MODIFY: pub mod declarations + re-exports; keep version()
crates/embers-core/src/model.rs            # Snapshot, Proc, Origin, Category, Confidence, Action, Ember, Outcome
crates/embers-core/src/config.rs           # Config + Defaults (embedded TOML) + merge(user over defaults) + validation
crates/embers-core/src/redact.rs           # redact_argv(&[String]) -> Vec<String>; cap_field(&str, 2048)
crates/embers-core/src/protect.rs          # is_protected(&Proc, &Config, &Context) -> Option<ProtectReason>
crates/embers-core/src/origin.rs           # attribute(&Snapshot, &Config) -> HashMap<Pid, Origin> (ancestor walk + pattern match)
crates/embers-core/src/rules/mod.rs        # classify(prev, cur, cfg, hosts) -> Vec<Ember>; composes rules; dedups by (pid,start_time)
crates/embers-core/src/rules/e1_idle_session.rs
crates/embers-core/src/rules/e2_orphan.rs
crates/embers-core/src/rules/e3_idle_server.rs
crates/embers-core/src/rules/e4_loop.rs
crates/embers-core/src/rules/e5_runaway.rs
crates/embers-core/src/reason.rs           # fact → sentence builder ("idle 2 h 14 m · 0 % CPU · 3 helpers · host says idle")
crates/embers-core/src/fixture.rs          # load Snapshot/HostStates/Config triplets from JSON (used by tests and --fixture)
crates/embers-core/defaults.toml           # embedded via include_str!: thresholds, patterns, protected lists for macos/windows
crates/embers-core/tests/fixtures/*.json   # see "Fixtures"
crates/embers-core/tests/rules.rs          # integration tests over fixtures
crates/embers-core/tests/redact.rs
crates/embers-core/tests/config.rs
crates/embers-cli/Cargo.toml               # MODIFY: add serde_json (for --json output)
crates/embers-cli/src/main.rs              # MODIFY: scan gets --fixture <path>, --json; others stay "not implemented" (exit 2)
docs/DEPENDENCIES.md                       # MODIFY: add the new crates with reasons + new heading "Manual pins" (cargo-audit 0.22.2, gitleaks 8.24.3)
Cargo.lock                                 # regenerated
```

**Allowed files**: exactly the tree above. **Forbidden**: everything else — in particular `apps/**`, `.github/**`, `deny.toml`, `rust-toolchain.toml`, `docs/**` other than `docs/DEPENDENCIES.md`, `docs/reviews/**`, `README.md`, `SECURITY.md`. If a forbidden file seems necessary, stop and report.

### Dependencies (S7)

Add **only** these (all on the pre-approved list in `docs/DEPENDENCIES.md`): `serde` (derive), `serde_json`, `toml`, `regex`, `thiserror`. Default-features off where a crate pulls optional extras; no `tokio`, no `tracing` yet, no `anyhow`. `cargo deny check` must stay green; `cargo tree -e no-dev --depth 1` must match the updated `docs/DEPENDENCIES.md` table.

### Interfaces expected (signature level)

```rust
// model.rs
pub type Pid = u32;
pub struct Snapshot { pub taken_at_ms: u64, pub os: Os, pub procs: Vec<Proc>, pub frontmost_pid: Option<Pid>, pub self_pids: Vec<Pid>, pub current_uid: u32 }
pub enum Os { Macos, Windows, Linux }
pub struct Proc {
    pub pid: Pid, pub ppid: Option<Pid>,            // ppid = None means "parent gone / invalid" (sampler already validated start times)
    pub pgid: Option<Pid>, pub start_time_ms: u64, pub uid: u32,
    pub exe: String, pub argv: Vec<String>,          // argv already redacted by the sampler; rules never un-redact
    pub cpu_time_ms: u64, pub footprint_bytes: u64,
    pub listeners: Vec<u16>, pub established: u32,
    pub is_app_bundle: bool, pub has_tty: bool, pub is_system_path: bool,
}
pub enum Origin { ClaudeDesktop, ClaudeCli, CodexApp, CodexCli, Terminal, Unknown }
pub enum Category { IdleSession, Orphan, IdleServer, Loop, Runaway }
pub enum Confidence { High, Medium }
pub enum Action { TerminateGroup, Terminate, QuitApp, Inform }
pub struct Ember { pub id: String /* "<root_pid>-<start_time_ms>" */, pub root_pid: Pid, pub pids: Vec<Pid>, pub categories: Vec<Category>,
    pub origin: Origin, pub confidence: Confidence, pub reason: String, pub metrics: Metrics, pub action: Action, pub recovery_hint: Option<String>, pub protected: Option<ProtectReason> }
pub struct Metrics { pub idle_secs: Option<u64>, pub cpu_pct: f32, pub footprint_bytes: u64, pub age_secs: u64, pub helper_count: u32, pub listeners: Vec<u16> }
pub enum ProtectReason { Kernel, SystemDaemon, NotOwned, SelfProcess, Frontmost, BusySession, LiveTty, Allowlisted(String) }

// hosts (fed by adapters in 1b; in 1a comes from fixtures)
pub enum SessionStatus { Idle, Busy }
pub struct SessionState { pub pid: Pid, pub status: SessionStatus, pub status_since_ms: u64, pub session_id: Option<String>, pub cwd: Option<String>, pub last_transcript_write_ms: Option<u64> }
pub struct HostStates { pub sessions: Vec<SessionState> }

// rules/mod.rs
pub fn classify(prev: Option<&Snapshot>, cur: &Snapshot, cfg: &Config, hosts: &HostStates) -> Vec<Ember>;

// config.rs
pub struct Config { pub idle_minutes: u32 /*30*/, pub window_minutes: u32 /*5*/, pub hog_memory_bytes: u64 /*max(8 GiB, 50% RAM) resolved by caller*/, pub hog_cpu_pct: f32 /*100*/, pub hog_minutes: u32 /*10*/,
    pub origins: Vec<OriginPattern>, pub dev_tool_patterns: Vec<String>, pub protected: ProtectedLists, pub allowlist: Vec<AllowRule> }
pub fn load_defaults() -> Config;                    // from embedded defaults.toml
pub fn merge(defaults: Config, user_toml: &str) -> Result<Config, ConfigError>;  // user keys override; unknown keys = error
```

Rule semantics are exactly PRE_DESIGN §6.2–6.3. Clarifications that remove ambiguity:
- `cpu_pct` over the window = `(cur.cpu_time_ms − prev.cpu_time_ms) / (cur.taken_at_ms − prev.taken_at_ms) × 100`; with `prev = None`, CPU-based conditions evaluate to **unknown → rule does not fire** (never assume idle on a single snapshot). E5's memory clause may fire on a single snapshot; its CPU clause may not.
- E1 requires a `SessionState` for the root pid with `Idle` for ≥ `idle_minutes` **and** cpu < 1 % **and** no descendant with `start_time_ms` inside the window. Without a `SessionState`, E1 may still fire at **Medium** when `last_transcript_write_ms` is older than `idle_minutes` and cpu < 1 %; with neither signal it does not fire.
- E2 "parent gone": `ppid == None`, or `ppid == Some(1)` on macOS with `!is_app_bundle && !is_system_path`; **and** (`exe`/`argv` matches `dev_tool_patterns` **or** `!listeners.is_empty()`).
- E5 default action is `QuitApp` for app bundles and `Inform` for anything with `is_system_path` (never `Terminate`); a protected process may still be *reported* with `protected: Some(_)` and `action: Inform` so the UI can show the lock and explanation — **but** `pids` of a protected ember must never be fed to any terminate call (1c enforces; 1a documents it in `Ember`'s doc comment).
- Protected check runs **first** in `classify`; a protected process can only produce an `Inform` ember.
- `reason` is built from facts only (reason.rs), ASCII middle-dot separated, ≤ 160 chars; `recovery_hint` for Claude sessions = `"Resume: claude --resume <session_id> (in <cwd>)"` when both are known; for dev tools = the redacted argv joined by spaces.

### Fixtures (anonymised shapes; all pids/paths synthetic, no real user names)

1. `claude_desktop_idle.json` — `disclaimer --pgroup` → `claude …` (pgid = disclaimer pid) → 3 helpers; host says `Idle` for 2 h; cpu delta 0 → **one E1 ember** at High, `pids` = all 5, `action = TerminateGroup`, hint contains `--resume`.
2. `claude_desktop_busy.json` — same tree, host `Busy` → **no ember**; `BusySession` protection recorded.
3. `claude_cli_no_hostfile.json` — `claude` under a live terminal shell with TTY, transcript mtime 3 h old, cpu 0 → **E1 Medium**; same with cpu 5 % → none.
4. `orphan_http_server.json` — `python3 -m http.server 8765`, ppid None, listener 8765 → **E2 High**, hint = argv.
5. `idle_vite_server.json` — `node vite` listening :5173, 0 established, cpu 0 for 40 min, origin session gone → **E3 Medium**.
6. `poll_loop.json` — `bash -c 'until …; do sleep 30; done'` → `sleep 30`, 50 min old, parent session gone → **E4 Medium**, both pids.
7. `runaway_mirroring.json` — system-path app, footprint 30 GiB, 6 h → **E5**, `action = Inform`, reason mentions "30 GB"; `kernel_task`-like pid 0 and `WindowServer` present → appear only as protected `Inform` or not at all (choose: protected list items are **not** emitted unless they are also E5 — document the choice).
8. `protected_never.json` — frontmost app with 20 GiB footprint → protected `Frontmost`, `Inform` only; a process with `uid != current_uid` → never emitted as actionable.
9. `windows_pid_reuse.json` — `os = Windows`, child whose `ppid` sampler set to `None` (reuse detected) and `exe = python.exe` → E2; sibling with valid parent → nothing.
10. `allowlist.json` — `http.server` matching an allow rule → not emitted (or emitted as `Allowlisted` Inform — pick one and test it).
11. `single_snapshot.json` — `prev = None`: E1/E3/E4 do not fire; E5 memory clause fires.

Fixtures carry `expected.json` beside them (list of `{root_pid, categories, confidence, action}`); the test asserts set equality and that **no expected-protected pid** appears in any actionable ember.

### Tests (must exist and pass on 3 OSes)

- `tests/rules.rs`: one test per fixture above (11), plus **mutation checks** (institution `20`, 2026-08-25): the executor must demonstrate in the report that inverting (a) the E2 parent-gone condition and (b) the protected-first ordering each makes at least one test fail, then restore. Tests must assert user-visible properties (category set, action, pid membership, reason contains "idle"/"GB"), not internal calls.
- `tests/redact.rs`: `--api-key sk-…`, `TOKEN=abc`, `Authorization: Bearer x`, `--password=…` → values replaced by `***`; non-secret flags untouched; field > 2048 bytes truncated with `…`; a string containing `<script>` and ANSI escapes survives as plain text (no interpretation) and is length-capped (S10).
- `tests/config.rs`: defaults load; user TOML overrides `idle_minutes`; unknown key → `ConfigError`; negative/zero thresholds → error; allow rule regex that fails to compile → error (never panic).
- `cargo clippy --workspace --all-targets --locked -- -D warnings` clean; `cargo fmt --check` clean; `cargo deny check` clean; **no `unsafe`** in 1a (`#![forbid(unsafe_code)]` in `embers-core/src/lib.rs` — 1b will relax it only inside the sampler module).

### CLI behaviour (1a)

- `embers scan --fixture <dir-or-file> [--json] [--config <toml>]` → text table (name · origin · reason · metrics · action) or NDJSON (one `Ember` per line, `serde_json`). Exit 0 if no embers, 0 with list otherwise (exit code is not a signal in 1a). Without `--fixture`: still `not implemented yet`, exit 2 (real sampling is 1b).
- Output must never contain unredacted argv (fixtures include a secret-looking token to prove it end-to-end).

### Performance constraints

`classify` over a 1,000-process synthetic snapshot < 5 ms in release (one `#[test]` with a generated snapshot asserts < 50 ms in debug to stay flake-free). No allocation hot spots that are obviously quadratic (ancestor walk uses a `HashMap<Pid, &Proc>`).

### Acceptance (action → verification)

1. `cargo test --workspace --locked` → all fixture/redact/config tests pass; report quotes the summary line.
2. Mutation evidence → report shows the two inverted-rule failures (test names + assertion lines) and the restore.
3. `cargo run -q -- scan --fixture crates/embers-core/tests/fixtures/claude_desktop_idle.json` → one line/row mentioning idle, `TerminateGroup`, and `--resume`; `--json | jq -c .action` → `"TerminateGroup"`.
4. `cargo run -q -- scan --fixture …/runaway_mirroring.json` → `Inform`, reason contains `GB`, no `Terminate*` anywhere.
5. `cargo run -q -- scan --fixture …/orphan_http_server.json --json | grep -c 'sk-'` → `0` (secret redacted end-to-end).
6. `cargo deny check` → all ok; `cargo tree -e no-dev --depth 1` equals the table in `docs/DEPENDENCIES.md` (both crates), and that file now has a **"Manual pins"** section listing `cargo-audit 0.22.2` (audit.yml) and `gitleaks 8.24.3` (ci.yml).
7. `grep -rn "unsafe" crates/` → only the `forbid(unsafe_code)` attribute.
8. PR opened from branch `phase-1a` → CI `test` ×3, `deny`, `gitleaks`, `macos-build` green; report includes the run URL.
9. `git diff --stat main` lists **only** files in the allowed tree.

### Stop conditions

- A needed crate is not on the pre-approved list, or `cargo deny` flags one of the approved crates.
- A fixture can only be made to pass by changing its `expected.json` (report the disagreement instead; the fixture expectation is the spec).
- Rule text in PRE_DESIGN §6 turns out ambiguous for a fixture → stop and ask (do not pick silently; the clarifications above are the only sanctioned ones).
- Any temptation to touch `.github/**`, `apps/**`, or docs other than `DEPENDENCIES.md`.
- Diff (excluding `tests/fixtures/**` and `Cargo.lock`) heading past ~900 lines → stop, report, and propose a cut.

### Non-goals (1a)

Real process sampling, reading any file under `~/.claude` or `~/.codex`, Claude/Codex adapters, `watch`/`explain`/`extinguish`, Windows API calls, `--self-stats`, persistence of allow-list edits, any UI, any networking.

### Assumptions (result-changing only) and simplicity budget

- Fixture-driven design is sufficient to specify the engine; 1b only swaps the data source. If 1b reveals a missing field, it is added in 1b with a new fixture, not anticipated now.
- Simplicity budget: plain structs + free functions; no trait objects for rules (a `fn(&Ctx) -> Option<RuleHit>` per module is enough); no async; no logging framework.

### Verification map

Each acceptance item above is one `action → verification` pair; mutation checks (item 2) are the only items whose evidence is a *deliberate failure* — show it, then show the restored green run.

### Report format

Institution `10` §4. First line = conclusion. Then: acceptance items 1–9 each with evidence; mutation evidence; **Owner actions outstanding** (expected: none for 1a except merging); **Not yet exercised in CI** (expected: none — `audit.yml` run 37259315162 result to be confirmed by the owner and noted here); "Found but not touched" list; files changed (`git diff --stat`). Long outputs go under `target/reports/phase1a/` and are referenced by path.

### After 1a

Sonnet adversarial review (fresh context; receives this file, the diff, the CI URL, and the fixture directory; must try to construct a snapshot that yields an actionable ember for a protected process, and a secret that survives redaction) → arbitration in `docs/reviews/phase1a_round1_*.md` → owner merge → **1b goal issued** (macOS sampler + host adapters; will add `libc` and the `unsafe` exception for `snapshot/macos.rs`, and a read-only file adapter for `~/.claude/sessions/*.json` validated by pid + start time, with `*.key` files explicitly never opened).

---

## Phase 1b / 1c — outline only (goals issued later)

- **1b (macOS sampler + adapters + CLI)**: `snapshot/macos.rs` via `proc_listpids` / `proc_pidinfo(PROC_PIDTBSDINFO)` / `proc_pid_rusage` / `sysctl KERN_PROCARGS2` / `PROC_PIDLISTFDS` (candidates only); `hosts/claude.rs` (sessions dir + transcript mtime; validates `pid` alive and `procStart`), `hosts/codex.rs` (transcript mtime only); `scan`, `watch --interval --json` (NDJSON `snapshot|ember_found|ember_updated|ember_resolved`), `explain <pid>`, `--self-stats`. Acceptance adds the budget run (10 min, < 0.5 % CPU, < 15 MB RSS, zero child processes) and the owner's hands-on checklist from PRE_DESIGN Phase 1 QA items 1, 3, 4, 5 (no extinguish). New deps: `libc`, `directories`; `unsafe` allowed only in `snapshot/macos.rs` with a safety comment per block.
- **1c (actions + Windows)**: `action/macos.rs` (pre-flight re-validation by `(pid, start_time)`, SIGTERM to pgid or tree, wait `grace_seconds`, SIGKILL only with `--force`, per-pid `Outcome`), `snapshot/windows.rs` + `action/windows.rs` (`sysinfo`/`windows` crates; creation-time PPID validation; WM_CLOSE → TerminateProcess), S8 tests (other-uid pid → `NotOwned`, no elevation APIs referenced), S11 CLI test (force unreachable without flag + confirmation). Owner QA: PRE_DESIGN Phase 1 item 2 (`http.server` extinguish) on the Mac; Windows compile + unit tests on the runner only (hands-on Windows QA is Phase 3).
