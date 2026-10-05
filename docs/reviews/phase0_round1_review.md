> Stored by the tech lead on 2026-10-05. Content is the reviewer's original except that the owner's GitHub account name and e-mail address were replaced with `<owner>` / `<owner e-mail>` (rule S13: no personal identifiers in tracked files). No other edits.

# Phase 0 Round 1 review — embers `phase-0` @ 16117ec

Reviewer: single Sonnet reviewer. Review only; no repo file modified (all runs in a detached worktree, removed afterwards; original checkout clean at 16117ec).

## Verdict: CAN MERGE after owner-side checklist (no Blocking, no Major). 3 Minor, 4 Suggestion.

I tried to break it on CI, gates, build.sh, SIGTERM, deny.toml, privacy. All seven acceptance items reproduce. CI run 37253158056 is green on all 6 jobs (`gh pr checks 1`).

## Findings

### Minor-1 — Repo/owner state contradicts the committed docs (SECURITY.md S1, S2)
- Evidence: `gh api repos/<owner>/embers/private-vulnerability-reporting` -> `{"enabled":false}`; `gh api .../branches/main/protection` -> 404 "Branch not protected". Repo is public.
- Failure scenario: merge PR now -> SECURITY.md (line 7-8) tells a reporter "do not open a public issue; Security tab -> Report a vulnerability". The button does not exist, so a reporter has no private channel (and no email is offered). `deny`/`gitleaks`/`test` are not required checks, so S2 "deny is a required status check" does not hold.
- Both are listed in ARCHITECTURE_DECISION.md as owner-side actions, so not executor fault. Fix: owner enables PVR and branch protection (`test (ubuntu/macos/windows)`, `deny`, `gitleaks`) before or immediately at merge. The executor report should have listed this as an open item.

### Minor-2 — audit.yml installs an unpinned tool and relies on deprecated rustup auto-install (S3 spirit, T1)
- Evidence: `.github/workflows/audit.yml:16` `cargo install cargo-audit --locked` has no `--version`; no "Install pinned toolchain" step as in ci.yml:24-25.
- Scenario A (supply chain): a malicious new cargo-audit release (or a new top-level release changing its lockfile) is compiled and its build scripts run on the next daily cron with no human review and no Dependabot visibility (Dependabot cannot see this line). Blast radius is small (contents:read, persist-credentials:false, no secrets), which is why this is Minor and not Major. Author disclosed this deviation (3).
- Scenario B (fragility): the pinned toolchain 1.99.0 is not preinstalled, so the job depends on rustup's auto-install through the `cargo` proxy. I reproduced it locally with rustup 1.29.1 in a fresh RUSTUP_HOME (`cargo --version` in a dir with rust-toolchain.toml channel=1.99.0 -> "warn: the missing active toolchain ... has been auto-installed ... you may opt out"). The CI log of `test (ubuntu-latest)` prints "auto-installation is deprecated ... may stop working in the future (rustup#4836)". Today it works; when rustup removes it, the daily audit job turns red with "toolchain not installed". The workflow has never run (schedule/dispatch only run from the default branch), so it is unverified in CI. I ran the same two commands locally in the worktree: `cargo install cargo-audit --locked` -> cargo-audit v0.22.2 installed; `cargo audit` -> "Scanning Cargo.lock for vulnerabilities (23 crate dependencies)", exit clean.
- Fix options: `cargo install cargo-audit --version <x.y.z> --locked` and add the same `rustup toolchain install` step (or `RUSTUP_TOOLCHAIN=stable` for that job).

### Minor-3 — Two files on the forbidden list are in the commit
- Evidence: `git show --stat 16117ec` lists `docs/ARCHITECTURE_DECISION.md (+106)` and `docs/PRE_DESIGN.md (+373)` as ADDED by this commit (they did not exist in 3aea3ce). Goal: "Allowed files: exactly the tree above. Forbidden: docs/PRE_DESIGN.md, ... this file".
- Scenario: the executor committed the tech lead's two untracked docs. I cannot prove they were altered (no original to diff). Risk: if the executor edited either while committing, nothing in the repo shows it. Needs tech-lead arbitration: confirm byte-identical to the tech-lead originals (or accept). Not a code defect.

### Suggestion-1 — personal GitHub account name in tracked file (S13 "no personal ... accounts in committed files")
- `Cargo.toml:9` `repository = "https://github.com/<owner>/embers"`. Acceptance grep 5 does not catch it (no `@`, no `/Users/`), and the URL is the real repo location, so I do not call it a violation. Also note git history carries author `<owner e-mail>` in every commit's metadata (public, permanent) — not a tracked-file issue, but relevant to the intent of S13 and cannot be fixed after publishing without a history rewrite. Owner awareness only.

### Suggestion-2 — lib.rs test hardcodes the version
- `crates/embers-core/src/lib.rs:16` `assert_eq!(version(), "0.0.1")`. Any version bump (Cargo.toml:6 is the single source; build.sh:11 reads it) fails `cargo test` until lib.rs is also edited. A bump PR (Phase 4 release) will hit this. Not wrong now; the alternative is comparing to a manifest read, but that is the author's call.

### Suggestion-3 — release binary will embed the builder's home path
- `strings -a target/release/embers | grep /Users/` -> `/Users/<owner>/.cargo/registry/src/.../clap_builder-4.6.7/...` (local build). CI-built Phase 4 releases will embed `/home/runner/...` instead, so not a problem for CI-built releases (S6), but any locally built binary shared around leaks a username. Optional: `--remap-path-prefix` in release job later. The Swift binary has 0 `/Users/` hits (`strings -a ... | grep -c /Users/` = 0).

### Suggestion-4 — gitleaks version is pinned only implicitly; macos-build does not assert both slices
- ci.yml:55-58 relies on the action's hard-coded default (`GITLEAKS_VERSION || "8.24.3"`, src/index.js:138 at the pinned SHA). That is pinned (good) but old and invisible; setting `GITLEAKS_VERSION` explicitly would make "gitleaks (pinned)" visible and Dependabot-independent. Also ci.yml:72-74 verifies adhoc signature but not `lipo -archs` containing x86_64 and arm64; if the x86_64 slice build were dropped from build.sh, CI stays green (it is correct today: `lipo -archs` -> `x86_64 arm64`).

## Acceptance 1-7 against the goal (all run by me)

| # | Check | Result |
|---|---|---|
| 1 | `cargo test --workspace --locked` | 1 passed (embers-core), others 0 tests; fmt --check OK; `clippy --workspace --all-targets --locked -D warnings` clean |
| 2 | `cargo deny check` (0.20.2) | `advisories ok, bans ok, licenses ok, sources ok` (only license-not-encountered warnings) |
| 3 | `bash apps/macos/build.sh` | Embers.app built in ~1s; `codesign -dv` -> `Signature=adhoc`, `Identifier=org.embers.app`; `lipo -archs` -> `x86_64 arm64`; `codesign --verify --strict` valid; `open -n` then `pkill -x Embers` -> process gone within 0.5 s; plutil lint passes |
| 4 | `grep -rE 'uses: .*@v[0-9]' .github/workflows` | no output (rc=1); both workflows have top-level `permissions: contents: read`; no `pull_request_target` |
| 5 | literal grep from goal | hits only in `docs/ARCHITECTURE_DECISION.md:49,98` (the rule text quoting the pattern itself) and `.git` gitdir of my worktree; SECURITY.md clean. Tracked-file grep for `@` addresses: none |
| 6 | CI | run 37253158056: test (ubuntu/macos/windows), deny, gitleaks, macos-build all `pass` |
| 7 | DEPENDENCIES.md vs `cargo tree -e no-dev --depth 1` | exactly `clap v4.6.7`, `embers-core`; `cargo tree | grep -E 'reqwest|hyper|openssl|rustls'` empty |

## Already verified correct (what I actually ran or read)

- Negative gate tests, redone by me in a scratch copy: `time@=0.1.45` -> `error[vulnerability] RUSTSEC-2020-0071`, advisories FAILED; `atty@0.2.14` -> `error[unsound]` + `error[unmaintained]` (so `unmaintained = "all"` is accepted by 0.20.2 and works); `reqwest` -> `error[banned] crate 'hyper' / 'reqwest' is explicitly banned` (so the `deny = [{ crate = ... }]` list format is valid and works); `gmp-mpfr-sys` (LGPL) -> `licenses FAILED`; git dependency -> `error[source-not-allowed]`. Yanked path not exercised by me (no known yanked crate on hand); syntax `yanked = "deny"` accepted.
- Deviation (2) `allow-wildcard-paths = true` is necessary and narrow: removing it gives `error[wildcard]: found 1 wildcard dependency for crate 'embers-cli'`. It only exempts path deps of publish=false crates (all workspace crates have `publish = false`, Cargo.toml:10).
- All three action SHAs are real and match the stated tags: checkout v7.0.1 = 3d3c42e5… (commit); cargo-deny-action v2.1.1 -> annotated tag c3bbe7e4 -> 3c634983… ; gitleaks-action v3.0.0 = e0c47f4f… (via `gh api repos/*/git/ref/tags/*`). The cargo-deny action's Dockerfile bundles cargo-deny 0.20.2 (same as local), and its entrypoint contains the rustup-1.28 workaround, which is why `deny` passes on a repo with rust-toolchain.toml 1.99.0.
- gitleaks-action for PR events: reads PR commits through public-repo API (works with read-only token, repo is public; fork PRs also get read-only token); needs no secrets besides GITHUB_TOKEN; no license needed for personal account (README). If a leak is found it tries to post a PR review comment, which needs `pull-requests: write` and will fail with the read-only token, but the job still fails on the detection, so the gate holds (reading src/gitleaks.js; I did not trigger it).
- Local `gitleaks detect --source .` -> 5 commits scanned, no leaks.
- S3/S5 workflow hygiene: `persist-credentials: false` on every checkout; `${{ }}` only in `matrix.os`, `github.ref` (concurrency) and the gitleaks `GITHUB_TOKEN` env — no expression inside `run:` so no script-injection path; no secret echoed.
- Smoke-launch script logic: ran the exact ci.yml lines under `bash -ec` locally -> rc 0; `kill -0` on a dead pid fails the step under `-e` as intended; the bash loop reaps the child so `kill -0` really goes false (not a zombie); exit paths and the final `exit 1` are right. CI macos-build step passed.
- SIGTERM handling in App.swift: built a copy with markers at `-O`: stderr showed `DID_LAUNCH` then `GOT_TERM`, process gone within 1 s. So (a) the delegate is not released early despite NSApplication.delegate being weak, and (b) exit goes through the DispatchSource handler, not the default signal action. Before `applicationDidFinishLaunching` runs SIGTERM uses the default action and still kills. `signal(SIGTERM, SIG_IGN)` + `DispatchSource` is the correct pattern.
- build.sh shell correctness: ran from `/` with absolute path containing a space -> built fine; arrays non-empty so macOS /bin/bash 3.2 `set -u` empty-array bug does not apply; `find ... -print0 | sort -z` OK on BSD; `swiftc -target x86_64-apple-macos14.0` works on arm64 host and in CI; VERSION parse guarded by regex; `rm -rf "$APP_DIR"` scoped to `$ROOT/build`.
- Info.plist.template: LSUIElement true, LSMinimumSystemVersion 14.0, bundle id org.embers.app, output passes `plutil -lint` (build.sh:46).
- Windows runner concerns checked from the real run: `rustup show active-toolchain || rustup toolchain install` works in pwsh on windows-latest (job passed); `cargo fmt --check` passes there despite CRLF checkout.
- Toolchain/lock: rust-toolchain.toml exact `1.99.0` + clippy/rustfmt; Cargo.lock committed; CI uses `--locked` on clippy and test; `cargo build --release --locked` 3 s locally (goal limit 2 min).
- CLI: `embers --version` -> `embers 0.0.1`; `embers scan` -> stderr "not implemented yet", exit 2; bare `embers` prints help, exit 2 (clap usage error, same code).
- THIRD_PARTY_NOTICES.md claim checked: `gh api repos/ericjypark/codex-island/license` -> MIT, "Copyright (c) 2026 Eric Park". No Swift networking symbols in the binary (`strings | grep -iE 'URLSession|Network'` empty).
- dependabot.yml: cargo + github-actions, weekly, grouped minor/patch, schema valid on read; no npm (allowed until Phase 3). `gh api .../actions/permissions/workflow` -> default_workflow_permissions "read", can_approve_pull_request_reviews false.
- .gitignore covers `.env`, `.env.*`, `*.key`, `*.pem`, `*.p12`, `*.pfx`, `*.mobileprovision`, build/, target/, dist/, node_modules/. (It also drops the old `/.build/` line — harmless, no SwiftPM here.)

## Not verified
- audit.yml has never executed on GitHub (see Minor-2); commands verified locally only.
- Dependabot actually opening PRs (needs enabling/time).
- Yanked-crate deny path; the author's claim of testing it is not reproduced by me.
- Whether docs/PRE_DESIGN.md and docs/ARCHITECTURE_DECISION.md are byte-identical to the originals (Minor-3).

## Lesson
For Phase 0 style "CI skeleton" work, the executor's own green PR run already covered most risk; the real gaps sit in things CI cannot show before merge: scheduled workflows (never run until on default branch), repository settings the docs rely on (PVR, branch protection), and files committed outside the allow-list. Future Executor Goals should require a "not yet exercised in CI" list and an owner-actions checklist in the report.
