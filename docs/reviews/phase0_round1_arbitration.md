# Phase 0 — Round 1 arbitration

Date: 2026-10-05. Arbiter: tech lead. Subject: PR #1, branch `phase-0`, commit `16117ec`; CI run 37253158056 (6/6 green).
Review file: `docs/reviews/phase0_round1_review.md` (read in full, not the summary).

## Verdict

**Accept with corrections.** No Blocking/Major. Five findings accepted (two owner-side, three executor-side code changes, all inside the Phase 0 allowed tree), two deferred with a home, one rejected with reason. Merge is gated on: (1) the executor correction commit below passing CI, (2) the owner-side checklist in M1 being done, (3) a short correction re-review limited to the new diff.

## Findings

| ID | Decision | Reason | Next action (who / file) |
|---|---|---|---|
| **M1** SECURITY.md promises a private channel and required checks that the repo does not yet have | **Accepted (owner action)** | The committed text is correct policy; the repo settings lag. Merging without the settings would leave a reporter with no channel. | **Owner**, before merging PR #1: Settings → Code security → enable *Private vulnerability reporting* and *Dependabot alerts*; Settings → Branches → protect `main` requiring `test (ubuntu-latest)`, `test (macos-latest)`, `test (windows-latest)`, `deny`, `gitleaks`, `macos-build`. **Executor**: add an "Owner actions outstanding" section to the PR description listing exactly these. |
| **M2** `audit.yml` installs `cargo-audit` unpinned and relies on deprecated rustup auto-install | **Accepted** | Unpinned `cargo install` on a cron is an unreviewed code-execution path (T1), and the job has never run on GitHub. Small blast radius, but it violates the spirit of S3/S14. | **Executor**, `.github/workflows/audit.yml`: (a) insert the same `Install pinned toolchain` step as `ci.yml` (`rustup show active-toolchain \|\| rustup toolchain install`) before the install step; (b) change to `cargo install cargo-audit --version 0.22.2 --locked` (version the reviewer verified installs and runs clean) with a comment `# Dependabot cannot see this pin; bump at each release (S12 checklist)`; (c) after merge, **owner** runs the workflow once via *Run workflow* so it is exercised; result goes in the Phase 1 report's "not yet exercised in CI" list until then. |
| **M3** `docs/ARCHITECTURE_DECISION.md` and `docs/PRE_DESIGN.md` committed although on the forbidden list | **Accepted into the repo; scope breach recorded** | I verified the committed files against my originals: `git diff --stat 3aea3ce 16117ec -- docs/` shows pure additions (+373 / +106 lines), line counts match the originals exactly (373 incl. the two-line "Decided 2026-10-05" marker at `PRE_DESIGN.md:279`; 106 incl. row S14 at `ARCHITECTURE_DECISION.md:50`), and spot-checked passages are verbatim. Content is mine; publishing the design docs is what the owner wants. The breach is procedural: forbidden files were added without asking first (rubric R3). | **Executor**: no file change. **Process rule from now on** (goes into every Executor Goal): files outside the allowed tree are never staged, even untouched ones; if a tech-lead document should ride along, stop and ask. Commits/pushes stay a handoff artifact only (branch + PR); the executor never merges. |
| **S1** `Cargo.toml:9` `repository` URL contains the account name; commit author e-mail in history | **Rejected** (URL) / **Deferred to owner** (git author) | The repository URL is the project's public identity, not personal data; S13 targets paths, e-mails and private account references. The commit author e-mail is real personal metadata, outside tracked files, and cheap to fix **only now** (6 commits, no external clones yet). | **Owner decision**: either (a) rewrite the 6 commits' author to a GitHub no-reply address (`<id>+<user>@users.noreply.github.com`) before anyone clones, or (b) accept it. In both cases set `git config user.email` to the no-reply address in this repo before the next commit. Recorded as owner backlog item OB-1. |
| **S2** `lib.rs:16` hardcodes `"0.0.1"` | **Accepted** | Any version bump breaks `cargo test` for no safety gain; a shape assertion keeps the test meaningful (non-empty, `X.Y.Z` numeric). | **Executor**, `crates/embers-core/src/lib.rs`: replace the assertion with a check that `version()` is non-empty and splits into exactly three numeric components (e.g. `version().split('.').filter(\|p\| p.parse::<u32>().is_ok()).count() == 3`). No new dependency. |
| **S3** Locally built release binary embeds the builder's home path | **Deferred → Phase 4** | S6 says only CI-built binaries are released, so no exposure in distributed artifacts. The remap is a release-workflow concern. | **Tech lead**: add to the Phase 4 goal: release job sets `RUSTFLAGS="--remap-path-prefix=$HOME=/build --remap-path-prefix=$PWD=/src"` (and `[profile.release] strip = "symbols"`), with acceptance `strings -a <binary> \| grep -E '/Users/\|/home/'` empty. Until then, never share locally built binaries. |
| **S4a** gitleaks version pinned only implicitly | **Accepted** | Making the pin visible costs one line and removes a hidden dependency on the action's default. | **Executor**, `.github/workflows/ci.yml` gitleaks step: add `GITLEAKS_VERSION: 8.24.3` under `env:` (same version the action currently defaults to — do not jump versions in a correction commit), plus the same "Dependabot cannot see this pin" comment. |
| **S4b** `macos-build` does not assert both architectures | **Accepted** | `build.sh` could silently drop a slice and CI would stay green; the universal binary is a stated property of the build. | **Executor**, `.github/workflows/ci.yml` after *Verify ad-hoc signature*: add `- name: Verify universal binary` → `run: lipo -archs build/Embers.app/Contents/MacOS/Embers \| tr ' ' '\n' \| sort \| diff - <(printf 'arm64\nx86_64\n')`. |

## Reviewer's "already verified correct" items

Accepted as evidence for acceptance items 1–7 (all independently re-run by the reviewer, including the negative gate tests for advisories, unmaintained, bans, licenses and sources). The yanked-crate path remains **not independently reproduced**; acceptable for Phase 0 because `yanked = "deny"` is syntax-checked by `cargo deny` and the other four advisory/ban paths were exercised.

## Correction round instructions

- Executor makes **one** correction commit on `phase-0` touching only: `.github/workflows/audit.yml`, `.github/workflows/ci.yml`, `crates/embers-core/src/lib.rs`, and the PR description. Nothing else.
- Correction re-review (Sonnet, fresh context) receives only: this file, the correction diff, and the CI run URL. It checks M2, S2, S4a, S4b and nothing more (institution `10`, 2026-08-13 lesson).
- Owner completes M1 and decides S1/OB-1; then merges PR #1.

## Process changes recorded (apply from Phase 1 onward)

1. Every executor report must contain two lists: **"Owner actions outstanding"** and **"Not yet exercised in CI"** (scheduled workflows, repo settings, release paths). Source: reviewer's lesson.
2. Files outside the allowed tree are never staged; ask first (M3).
3. Version pins that Dependabot cannot see (`cargo install --version`, `GITLEAKS_VERSION`) are listed in `docs/DEPENDENCIES.md` under a "manual pins" heading and reviewed at every release (S12) — **executor adds this heading in the Phase 1 goal, not now** (`docs/DEPENDENCIES.md` is allowed in Phase 1).
