# Phase 1a — Round 2/3 arbitration (correction re-reviews)

Date: 2026-10-06. Arbiter: tech lead. Subject: PR #2 corrections `dfae3f7` (round 2, FAIL) → `20aa6d4` (round 3, PASS; CI run 37407098510, 6/6 green).
Review files: `phase1a_round2_rereview.md`, `phase1a_round3_rereview.md` (read in full).

## Verdict

**Phase 1a accepted — ready for owner merge** at head `20aa6d4` plus one small pre-merge commit for S1 below (no further re-review required; it changes a test bound only). All round-1 findings (F1–F10, S1–S3) and round-2 findings (N1–N5, N7–N9) are fixed with mutation evidence; N6 is recorded as a 1c design constraint; four residual suggestions are dispositioned below.

## Round-2 findings (N1–N9) — recorded dispositions

| ID | Decision | Outcome |
|---|---|---|
| N1 Major — timing test red on ubuntu/windows (lazy regex compile inside timed region) | Accepted | Fixed in round 3 (warm-up outside the timer; CI green ×3). Residual margin → S1 below. |
| N2 Major — URL userinfo with empty user / reserved chars leaks | Accepted | Fixed; 20-case grammar sweep clean; mutant red. Over-masking of `/@scope/pkg` and host loss in `b@c` paths accepted as the safe direction. Residual → S2 below. |
| N3 Minor — inherited protection capped at 64 generations | Accepted | Fixed by top-down propagation, no depth cap; mutant red. |
| N4 Minor — F6 app-bundle/system-path leader clause untested | Accepted | Fixture b1 added (expected `Terminate [1001..1004]`); mutant red. |
| N5 Minor — config warnings never printed | Accepted | Printed to stderr; `--json` stdout clean. Residual → S3 below. |
| N6 Suggestion — merged `Terminate` keeps group-sized `pids` | **Accepted as a 1c constraint, no 1a change** | Reviewer re-examined: every pid in the set passed the E1 group invariants and protection filter; consent surface shows the true scope. **Binding for 1c: the action layer acts on `Ember.pids` exactly — never re-derives a tree or group from `root_pid`/`pgid`.** Goes into the 1c goal verbatim. |
| N7 Suggestion — hog fact dropped from system-path reason | Accepted | Fixed (system note first, then the hit-specific fact, within 160). |
| N8 Suggestion — `redact_argv` flag match on raw arg | Accepted | Fixed for the `is_invisible` set; mutant red. Residual → S4 below. |
| N9 Suggestion — second redact pass eats closing quote | Accepted | Fixed (redact pieces → quote → cap, no second pass); mutant red. |
| J1 supplement (doc table + launchd-child fixture) | Accepted | Done; mutant making SystemDaemon inheritable → 12 tests red. |

## Round-3 residual suggestions

| ID | Decision | Reason | Next action |
|---|---|---|---|
| **S1** timing test headroom (~1.2× under parallel harness; one unexplained local red) | **Accepted — relax before merge** | A wall-clock bound in a debug build on shared runners is a flake source, not a performance guarantee (institution `20`: a test that fails for reasons unrelated to the property it asserts is worse than no test). The real budget (< 5 ms release) is measured in 1b's `--self-stats`. | Executor, pre-merge commit on `phase-1a`, `crates/embers-core/tests/rules.rs` only: raise the debug bound to **250 ms** and add a comment that the authoritative budget check is the 1b `--self-stats` acceptance. No re-review needed (test-only change); CI must be green. |
| **S2** URL password containing raw `'`, `"` or space not masked | **Deferred → 1b** (recorded) | Pre-existing grammar edge; quotes are excluded to avoid crossing a surrounding shell quote, which is the more common and more damaging case. A fix needs a quote-aware tokenizer, not another regex tweak. | Tech lead adds to the 1b goal as a redaction item with the three repro shapes (u12, u15, u19) as fixtures; executor must not patch it ad hoc in 1a. |
| **S3** config warning printed once per fixture file, no CLI test | **Deferred → 1b** (recorded) | Fixture mode is a test harness; 1b's real `scan`/`watch` loop is where "once per run" matters. | 1b goal: warnings printed once at startup of `scan`/`watch`, covered by a CLI test asserting exactly one stderr line; fixture-mode behaviour left as is. |
| **S4** `is_invisible` narrower than CLI `needs_escape` (bidi/tag chars in flag names) | **Accepted → 1b** (recorded) | Zero practical risk (no program parses `--toke<U+202E>n` as `--token`) but two sanitizer lists will drift; the reviewer's lesson is right. | 1b goal: move the canonical "unsafe code point" set into `embers-core` (one function), used by `redact_*` and re-exported to the CLI's `needs_escape`; add the U+202E/U+2028/U+0085/tag-char flag-name cases as redact tests. Not done in 1a to keep the merge scoped. |

## Deferred items carried forward (consolidated backlog for 1b/1c)

1. F4 part 4 — flag-chain redaction (`--key a SECRET`): revisit only if a real case appears.
2. S2/S3/S4 above (1b).
3. N6 — 1c acts on `Ember.pids` only.
4. Phase 0 S3 — `--remap-path-prefix` + `strip` in the Phase 4 release job.
5. 1a report "Found but not touched": launchd/Homebrew-services jobs need `Proc.is_managed_service` in the 1b sampler (E2 parent-gone must exclude them).

## Process notes

- Three review rounds for 1a (1 full + 2 correction) is at the ceiling I want; the extra round came from an incomplete F2 fix (examples instead of grammar) and from moving lazy init into a timed test. Rule for 1b onward, added to the goal: **when a fix is justified by a grammar (URLs, flags, quoting), enumerate the grammar in the tests, not the reviewer's examples**; and **no wall-clock assertions in debug-build unit tests** — budgets are measured by `--self-stats` acceptance.
- Docs for 1a (`docs/PHASE1_GOAL.md`, the five review/arbitration files, `INDEX.md` rows) go into a separate docs PR opened by the executor after PR #2 merges; owner reviews and merges it.

## Phase 1b scheduling

1b goal is issued **after** PR #2 and the docs PR are merged (so 1b branches from a `main` containing both). Expected content: PRE_DESIGN Phase 1b outline + the deferred items above + the two process rules; allowed tree adds `crates/embers-core/src/snapshot/{mod,macos}.rs`, `src/hosts/{mod,claude,codex}.rs`, CLI `scan`/`watch`/`explain`/`--self-stats`, deps `libc`, `directories`, and the `unsafe` exemption confined to `snapshot/macos.rs`.
