# Embers — Architecture Decision

Date: 2026-10-05. Decided by: owner. Recorded by: tech lead. Basis: `docs/PRE_DESIGN.md` (§5 options, §12 questions).
Status: **decided — Phase 0 may start** (Executor Goal below).

## 1. The decision (one sentence)

Build Embers as a **Rust shared core** (`embers-core` library + `embers` CLI) with a **native SwiftUI/AppKit notch overlay on macOS** and a **Tauri v2 tray-flyout app on Windows**, shipped **unsigned with honest install instructions**, **zero network**, and **security-hardened from the first commit** (rules in §3).

## 2. Why

- Only a native AppKit window can do the notch illusion (per-pixel click-through, `.popUpMenu` level, 120 Hz springs) at CodexIsland quality — PRE_DESIGN §5 Option C.
- One Rust core means the dangerous logic (classification, protection list, termination) exists once, is fixture-tested on ubuntu/macos/windows every push, and links in-process into the Tauri Windows UI.
- Tauri over WinUI 3: one extra toolchain (already Rust) instead of a third language; builds on the GitHub `windows-latest` runner; 40–80 MB idle is inside budget. Electron rejected on footprint.
- Unsigned + honest: the project has no signing budget yet; silently stripping Gatekeeper quarantine (as some casks do) teaches users to bypass OS protections — the opposite of a safety tool's job.
- Security as a hard requirement: the app will be downloaded by strangers and it **kills processes**. The threat model (§3.0) therefore treats both the supply chain and the running app as attack surface from day one.

## 3. Question-by-question record

| Q | Decision | Note |
|---|---|---|
| Q1 core language | **Rust** (mac UI in Swift, as CodexIsland) | `rustup` to be installed on the Mac and the Windows PC by the owner. |
| Q2 Windows UI | **Tauri v2** | Links `embers-core` in-process. |
| Q3 signing | **Unsigned for now; honest path** | Cask/installer **never** strips quarantine; README documents Gatekeeper "Open Anyway" and SmartScreen "More info → Run anyway". Revisit signing at v1.0. |
| Q4 network | **Zero network in v1** | "Check for updates" opens the Releases page in the browser; no updater, no telemetry, no crash reporting. |
| Q5 thresholds | **idle 30 min; runaway ≥ 8 GB footprint or ≥ 100 % CPU sustained 10 min** | All configurable in `config.toml`. |
| Q6 brand / fallback | **Amber primary; menu-bar pill fallback in Phase 2** | Exact hex to be set in `Theme/Palette.swift` during Phase 2 (owner picks from a swatch in the Phase 2 goal). |
| Q7 Claude desktop sessions | **End the process group directly (SIGTERM → wait → SIGKILL only if confirmed) and show the `claude --resume <id>` hint** | If Phase 2 test A2 shows the desktop app misbehaves, fall back to the "archive in app" instruction. |
| New: security | **Hard requirement** — see §3.0–3.12 | Every rule has an acceptance check and is wired into CI or a phase gate. |

## 3.0 Security engineering rules (binding for all phases)

Threat model in one paragraph: (T1) a malicious or compromised dependency / GitHub Action / release step ships code that is not what is in the repo; (T2) a downloaded binary is tampered with in transit or mirrored by a third party; (T3) the running app is tricked — via process command lines, config files, or UI content — into killing the wrong process, reading secrets, or executing remote content; (T4) Embers runs with more privilege than it needs. Rules below map to these.

| # | Rule | Acceptance (how we know it holds) | Lands in |
|---|---|---|---|
| S1 | **Private vulnerability reporting.** `SECURITY.md` points to GitHub Private Vulnerability Reporting (Security tab → "Report a vulnerability"); no personal e-mail; states supported versions and a 7-day acknowledgement target. | File exists; repo setting "Private vulnerability reporting" enabled (owner, Settings → Code security); `SECURITY.md` has no `@` address. | Phase 0 |
| S2 | **Dependency gate in CI.** `cargo-deny` (`deny.toml`: `advisories` deny unmaintained/yanked/vuln; `licenses` allow-list MIT/Apache-2.0/BSD-2/BSD-3/ISC/Zlib/Unicode-3.0/MPL-2.0 only; `bans` deny wildcard versions + duplicate major versions warn; `sources` allow only crates.io) runs on every push/PR; `cargo audit` runs daily on a schedule as a second opinion. | CI job `deny` is a required status check; a deliberately added yanked crate fails the job (tested once in Phase 0, then reverted). | Phase 0 |
| S3 | **Actions pinned to commit SHAs.** Every `uses:` in `.github/workflows/*` is `owner/repo@<40-hex-sha> # vX.Y.Z`; no floating tags. | `grep -E 'uses: .*@v' .github/workflows` returns nothing; Dependabot keeps the SHAs fresh (S4). | Phase 0 |
| S4 | **Dependabot** for `cargo`, `github-actions`, `npm` (Tauri UI) weekly, grouped minor/patch. | `.github/dependabot.yml` present; first Dependabot PRs appear after enabling. | Phase 0 (npm ecosystem added Phase 3) |
| S5 | **Least-privilege workflows.** Top-level `permissions: contents: read` in every workflow; the release workflow elevates `contents: write` **only** in the release job; no `pull_request_target`; secrets never echoed. | Reviewer greps for `permissions:` per workflow; `pull_request_target` absent. | Phase 0 (release job Phase 4) |
| S6 | **Releases are built only by CI** from a tag, with `SHA256SUMS.txt` attached; later add GitHub artifact attestations (`actions/attest-build-provenance`) once the release workflow exists. No binaries built on a laptop are ever uploaded. | Release assets include `SHA256SUMS.txt`; README shows the `shasum -a 256` / `Get-FileHash` verification step; the release workflow is the only path with `contents: write`. | Phase 4 (workflow), rule stated Phase 0 |
| S7 | **Dependency minimisation.** Core allow-list for direct deps: `sysinfo`, `windows` (win only), `libc`, `serde`/`serde_json`, `toml`, `clap`, `directories`, `regex`, `thiserror`/`anyhow`, `tracing` (no network features). Adding any other direct dependency requires a one-line justification in the PR and tech-lead approval. No `openssl`/`reqwest`/`hyper`/`tokio-net` anywhere. | `cargo tree -e no-dev --depth 1` matches the list in `docs/DEPENDENCIES.md`; `cargo tree | grep -E 'reqwest|hyper|openssl|rustls'` is empty. | Phase 0 (list), every phase (check) |
| S8 | **No elevation, ever.** Embers never requests admin/root, never installs helpers, daemons or privileged services; it only acts on processes owned by the current user. | macOS: no `SMJobBless`/`AuthorizationExecuteWithPrivileges`/`sudo` in source; Windows: manifest `requestedExecutionLevel="asInvoker"`; code path returns "access denied — not owned by you" for other users' pids (unit test). | Phase 1–3 |
| S9 | **No remote content.** Zero network sockets opened by Embers. Tauri: `tauri.conf.json` CSP = `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'none'`; `dangerousRemoteDomainIpcAccess` empty; no `http(s)://` URLs in `ui/`; only the minimal Tauri capabilities (`core:default`, `tray`, `notification`, `autostart`, window set-ignore-cursor/always-on-top) are granted. Swift app: no `URLSession`/`Network` imports. | `lsof -p <pid>`/Resource Monitor shows no sockets after 30 min; grep for `URLSession|NSURLConnection|Network` in Swift and `http://|https://|fetch(|XMLHttpRequest` in `ui/` is empty (CI lint step); capabilities file reviewed. | Phase 2 (Swift), Phase 3 (Tauri) |
| S10 | **Untrusted input handling.** Command lines, environment, window titles, config files and session-status files are *data*: rendered as plain text (never as HTML/markup in the Tauri UI — `textContent`, not `innerHTML`), redacted for secret patterns, length-capped (≤ 2 KB per field), and never executed or passed to a shell. Kill targets are re-validated by `(pid, start_time)` immediately before acting. | Unit tests: argv containing `<script>`/ANSI escapes/10 MB string renders safely and truncated; PID-reuse pre-flight test; `grep innerHTML ui/` empty. | Phase 1–3 |
| S11 | **Safe defaults for destructive actions.** Never auto-kill; force-kill requires an explicit per-action confirmation; protected list enforced in the core, not the UI; an optional local action log is **off** by default and never contains argv. | Fixture tests for protected list; UI cannot reach `extinguish --force` without the confirmation flag (integration test on the CLI/bridge). | Phase 1–3 |
| S12 | **Independent security review before every release.** Before tagging, run Claude Code `/security-review` (or an equivalent independent reviewer) on the release diff plus a manual pass over S8–S11; findings arbitrated and recorded in `docs/reviews/`. Also: `cargo deny check` and the no-network check are release-gate items. | `docs/reviews/<version>_security.md` exists and is referenced in the release notes; release workflow refuses to run if `deny` failed. | Phase 4 and every release |
| S13 | **Secret-free repo.** `.gitignore` covers `.env*`, `*.key`, `*.p12`, `*.pfx`, `*.mobileprovision`; gitleaks (pinned) runs in CI on PRs; no personal paths/accounts in committed files. | gitleaks job green; reviewer grep for `/Users/`, `C:\\Users\\`, `@` addresses in tracked files is empty. | Phase 0 |
| S14 | **Reproducible toolchain.** `rust-toolchain.toml` pins the channel + components; `Cargo.lock` committed for all binaries; Swift deployment target and `swiftc` flags live only in `build.sh`. | `cargo build --locked` succeeds in CI; lockfile changes appear only in dependency PRs. | Phase 0 |

Owner-side actions these rules need (not executor work): enable Private Vulnerability Reporting, Dependabot alerts, and branch protection requiring the `test`, `deny`, `gitleaks` checks on `main`.

## 4. Consequences / what this closes

- PRE_DESIGN §12 is fully answered; §13 unverified items remain test obligations in Phases 1–3 (A1, A2 especially).
- Rust must be installed before Phase 0's first build (owner): `rustup` stable on the Mac; stable + MSVC target + VS Build Tools + Node on the Windows PC (Phase 3).
- Any future proposal to add network, telemetry, auto-update, elevation, or a quarantine-stripping install step **reopens this decision** and requires the owner.

---

# Phase 0 Executor Goal (revised, supersedes PRE_DESIGN Phase 0)

- **References**: `docs/PRE_DESIGN.md` §8.1–8.2, §10; this file §3.0 (rules S1–S5, S7, S13, S14).
- **Goal**: a building, testing, CI-green, security-hardened skeleton with no product logic.
- **Exact file structure**:
  ```
  Cargo.toml                      # [workspace] members = crates/*; resolver = "2"
  Cargo.lock
  rust-toolchain.toml             # channel = "stable" (pin exact version, e.g. "1.9x.y"), components = ["clippy","rustfmt"]
  deny.toml                       # S2 policy as specified
  SECURITY.md                     # S1
  THIRD_PARTY_NOTICES.md          # CodexIsland MIT notice placeholder (filled when code is reused)
  README.md                       # structure, status "pre-alpha", honest install placeholders, privacy statement
  .gitignore                      # S13 patterns + build/, target/, dist/, node_modules/
  .github/dependabot.yml          # S4 (cargo + github-actions weekly, grouped)
  .github/workflows/ci.yml        # S3/S5; jobs: test (ubuntu/macos/windows), deny, gitleaks, macos-build
  .github/workflows/audit.yml     # S2 scheduled cargo-audit (daily), permissions read-only
  crates/embers-core/{Cargo.toml, src/lib.rs}          # lib with one pure fn + one test
  crates/embers-cli/{Cargo.toml, src/main.rs}          # clap skeleton: `embers --version`, `embers scan` prints "not implemented"
  apps/macos/build.sh             # swiftc universal build, Info.plist from template, ad-hoc codesign (CodexIsland pattern, attributed)
  apps/macos/Resources/Info.plist.template             # LSUIElement=true, LSMinimumSystemVersion=14.0, bundle id org.embers.app (placeholder, owner may change)
  apps/macos/Sources/App.swift    # accessory app, empty NSApplicationDelegate, no windows
  apps/windows/README.md          # "Phase 3" stub only
  docs/DEPENDENCIES.md            # S7 allow-list + justification table
  docs/reviews/INDEX.md           # append-only review index header
  ```
- **Interfaces expected**: `embers_core::version() -> &'static str`; CLI via `clap` derive with subcommands `scan`, `watch`, `explain`, `extinguish`, `config` all returning exit code 2 "not implemented" except `--version`.
- **Allowed files**: exactly the tree above. **Forbidden**: `docs/PRE_DESIGN.md`, `docs/REQUIREMENTS.md`, `docs/NEXT_STEPS.md`, this file, anything under `docs/reviews/` beyond the INDEX header, any other path.
- **Dependencies permitted in Phase 0**: `clap` (derive), nothing else in core. Dev-deps: none. (S7)
- **Performance constraints**: none functional; `cargo build --release --locked` < 2 min on the Mac.
- **Tests**: one unit test in `embers-core`; CI matrix proves it on three OSes. **Negative tests of the gates (do once, then revert in the same PR, documenting in the report)**: (a) add a yanked/advisory crate → `deny` job fails; (b) commit a fake `AKIA…`-style string in a scratch file → `gitleaks` fails; (c) change an action to `@v4` → reviewer grep catches it (S3).
- **Acceptance (action → verification)**:
  1. `cargo test --workspace --locked` → passes locally (output excerpt in report).
  2. `cargo deny check` locally → `advisories ok, licenses ok, bans ok, sources ok`.
  3. `bash apps/macos/build.sh` → `build/Embers.app` exists; `codesign -dv build/Embers.app 2>&1 | grep -q adhoc`; launching it shows no Dock icon and exits within 1 s of `pkill -x Embers` (CodexIsland `verify.sh` pattern).
  4. `grep -rE 'uses: .*@v[0-9]' .github/workflows` → no output; every workflow has top-level `permissions: contents: read`.
  5. `grep -rnE '/Users/|C:\\\\Users|@[a-z0-9.-]+\.[a-z]{2,}' --include='*' . ` (excluding `.git`, `target`) → no personal paths/e-mails (`SECURITY.md` must pass this too).
  6. Push to a branch → CI jobs `test` (3 OS), `deny`, `gitleaks`, `macos-build` all green; link to the run in the report.
  7. `docs/DEPENDENCIES.md` lists exactly what `cargo tree -e no-dev --depth 1` shows.
- **Stop conditions**: `rustup`/`cargo` missing on the Mac (owner action; do not install system-wide tools yourself); `swiftc` cannot link SwiftUI on the runner; `cargo-deny` or gitleaks pinned versions unavailable; any need for a dependency outside the permitted list; any need to touch a forbidden file.
- **Non-goals**: detection logic, UI, Windows app, release workflow, attestation (Phase 4), signing.
- **Assumptions (result-changing only)**: GitHub `macos-latest` runner's toolchain can run `build.sh` unmodified (if not, pin `macos-14`/`macos-15` and report). **Simplicity budget**: no SwiftPM, no XCTest, no Makefile, no pre-commit framework — shell scripts + CI only.
- **Verification map**: each acceptance item above is one `action → verification` pair; the report must quote evidence for all seven plus the three negative gate tests.
- **Report format**: institution `10` §4 — first line = conclusion; evidence as `file:line` / command excerpts; CI run URL; long outputs saved under `target/reports/` and referenced by path; list of "found but not touched".
- **Then**: independent adversarial Sonnet review (review only) → tech-lead arbitration → owner acceptance → Phase 1 goal issued.
