> Stored by the tech lead on 2026-10-06. Reviewer original, verbatim (no personal identifiers present). Repro files referenced as `scratchpad/r2/*` lived in the reviewer's scratchpad; the ones named in the arbitration were ported into `crates/embers-core/tests/fixtures/` by the round-3 correction.

# Phase 1a round 2 correction re-review (dfae3f7)

Verdict: FAIL (not mergeable yet). 1 Major CI-red, 1 Major redaction hole (same class as F2), 3 Minor, 3 Suggestion.
Reviewer worktree: scratchpad/rr (detached dfae3f7). Repro files: scratchpad/r2/.

## Findings (new)

### N1 Major - CI red on ubuntu + windows (head dfae3f7)
`classify_1000_processes_is_fast` (tests/rules.rs:356-373, <50 ms) fails: CI "took 52.4 ms" (ubuntu), "53.3 ms" (windows); macOS passes.
Cause: the correction makes `merge` call `redact_text`, so the one-time lazy compile of the new ~12 regexes (OnceLock) lands inside the timed region: first redact_text call = 11.5 ms in a debug build (second call 33 us). Locally 1eca66d 17 ms vs dfae3f7 18-19 ms in isolation; fails under parallel load (it failed in my full `cargo test --workspace` run, passed alone).
Fix: warm `redact_text("")` before starting the timer (or raise the bound / use release-like measure). Arbitration gate "correction commit green in CI" is not met.

### N2 Major - F2 not fully fixed: URL with empty user (Redis form) leaks the password
`url_userinfo` requires a non-empty user (`[^\s/@:'"]+`). Repro (end-to-end via CLI, scratchpad/r2/redis.json):
 `node worker.js --redis redis://:SECRETVALR@cache:6379/0` -> hint unchanged, secret printed.
 `env REDIS_URL=redis://:SECRETVALS@cache:6379 ...` -> unchanged (name has no keyword).
Related shapes also leak: `postgres://u:pa/ss@h/db` (password containing `/`) -> unchanged; `postgres://admin:p@ssw0rd@db/app` -> `admin:***@ssw0rd@db` (tail `ssw0rd` leaks).
Fix: user `[^\s/@:'"]*`; for the password take everything up to the LAST `@` before the next whitespace (greedy `[^\s'"]*@`) when the scheme is followed by `userinfo@`.

### N3 Minor - inherited protection silently lapses beyond 64 generations (F1 class)
`with_inherited_protection` walks up at most MAX_DEPTH=64 per process while `descendants()` has no depth cap. Repro (scratchpad/r2/d64.json, d65.json, d70.json): Frontmost app -> chain of N unprotected procs -> 20 GB hog leaf:
 N<=62: `Inform` (Frontmost); N=64,65,70: `Terminate [leaf]`, protected=null.
Unrealistic depth but it is a bypass of "protection covers all descendants". Fix: propagate top-down (BFS from protected nodes), O(n), no depth cap.

### N4 Minor - F9 recurrence: the leader app-bundle / system-path clause of F6 condition (2) has no failing test
Mutant: drop `leader.is_app_bundle || leader.is_system_path` in `session_group` -> all tests green. Repro scratchpad/r2/b1_bundle_leader.json (claude_desktop_idle with Claude.app main pid 900 as pgid leader of the session group, not frontmost): correct code -> `Terminate [1001..1004]`; mutant -> `TerminateGroup [900,1000..1004]` (kills the whole app). Add this fixture with expected Terminate [1001-1004].

### N5 Minor - S2 warning is never surfaced
`Config.warnings` is filled (config.rs) and unit-tested, but nothing prints it: `embers scan --config empty-protected.toml` writes nothing to stderr (arbitration S2: "emit a warning line (stderr)"). Fix: print `cfg.warnings` to stderr in `scan_fixtures` (once per run).

### N6 Suggestion - merged action Terminate keeps TerminateGroup-sized pids
When E1 (TerminateGroup) merges with E5 on the same session, action becomes Terminate (more cautious per caution()) but `pids` is still the union incl. group leader above the root and sibling subtree. Property test: 6/4500 random cases (seeds 846, 1010, 3100, 10518, 10610, 10881; all IdleSession+Runaway). Not a protection violation (all pids unprotected, in leader's truncated subtree) but the "conservative" downgrade does not narrow scope. Suggest: when final action is Terminate, use only that root's tree.

### N7 Suggestion - J3 side effect: the hog fact is dropped from reason for system-path hogs
a9 text: `part of the system - Embers will not end it ... - parent gone - listening :8080 - running 5 h - no connections - 0 % CPU - up 5 h`: "30 GB memory" (the reason it was flagged) is cut by the 160-char cap. Consider putting the system note after the first hit-specific fact, or shortening the note.

### N8 Suggestion - `redact_argv` does not strip invisible chars before testing `secret_flag`
`redact_argv(["srv","--to​ken","SECRETVALX"])` -> value NOT masked (flag test runs on the raw arg). End-to-end scan is safe today because the joined command line is re-redacted by `redact_text`, but 1b sampler output (redact_argv alone) would leak. Strip invisibles at the top of the loop.

### N9 Suggestion - F7: final redact pass swallows the closing quote/paren of the shell-quoted cwd
a18 text: `(in '/work/api_key=***` (unterminated quote, missing `)`). Because `merge` redacts the whole hint after `shell_quote`. Cosmetic/copy-paste only (the id part is intact); redact the pieces, then quote, then cap without a second redact.

## Status of accepted findings
- F1 fixed (a1,a2,a3c,a7,a16 exact pid sets; random differential: 4500 random forests, 0 protection violations; oracle proven sensitive: M1 mutant -> 97/300 violations). Caveat N3.
- F2 NOT fixed fully (N2). F3 fixed (red.json all new vocabulary masked). F4 parts 1-3 fixed; part 4 (SECRETVAL36, SECRETVAL45) remain, as Deferred.
- F5 fixed (a9, a9b -> Inform). F6 fixed (a13, a3b, a19, a19b; rev. conditions implemented, caveat N4 test gap). F7 fixed (a18; N9 cosmetic). F8 fixed (own fixture with U+202E/200B/0085/009B/2028/2029/00AD/FEFF/E0041/ESC: no raw char in text or --json; lines intact). F9 fixed for the 3 required mutants (below). F10 fixed (a10: "listening on 40 ports (5000, 5001, 5002, ...)").
- S1 fixed (a17 -> nothing smoldering). S2 partial (N5; inf rejected OK). S3 fixed (`-p 3000` kept, `-pSECRET` masked).

## F9 mutants (each reverted afterward; worktree clean)
- M1 remove protected filter in `descendants()`: 6 tests fail (busy_nested_session..., allowlisted_server_keeps_its_worker, frontmost_app..., other_users..., protected_system..., every_fixture...).
- M2 session_group protection: single-clause removals survive (member-protection, leader-protection, intermediate check, leader-ancestor check) but removal of member-protection + leader-protection together -> protected_group_leader_blocks_group_kill + every_fixture fail; removing the subtree-membership clause -> unrelated_group_member_blocks_group_kill fails.
- M3 merge Allowlisted check -> allowlisted_memory_hog_never_shown + every_fixture fail.
- J5: equivalence holds. (a) member-protected is implied by member-in-truncated-subtree for non-leader members and by the leader check for the leader; (c) intermediate check is implied by root-in-subtree; leader-ancestor is implied by root-in-subtree. I could not construct a fixture that separates any single clause from its redundant partner; only N4 (bundle/system clause) is truly unguarded.

## Judgements J1-J5
- J1 acceptable (two-layer design; walk stops at any protected node; non-inheriting Kernel/SystemDaemon/LiveTty needed for runaway_plain_process). Note: a LiveTty shell below a Frontmost app still inherits Frontmost because the inheritance walk passes through non-inheritable ancestors - consistent with spec. Only defect is N3.
- J2 acceptable (leader Frontmost makes root inherit Frontmost -> no ember; a19 covers non-inheriting LiveTty -> Terminate).
- J3 acceptable with N7.
- J4 acceptable for redact_text (`--to<U+200B>ken=` masked, output loses the invisible chars); gap N8 in redact_argv.
- J5 acceptable (see above).

## Verified correct
cargo clippy --all-targets -D warnings clean; cargo deny: advisories/bans/licenses/sources ok; gitleaks dir . no leaks; `grep -rn unsafe crates/` only `#![forbid(unsafe_code)]`; cargo test --workspace --locked: all pass except the timing test noted in N1 (flaky under load, red in CI on 2 OS); red.json: only SECRETVAL36 and SECRETVAL45 remain (both Deferred) in --json and text; ctl.json hints <= 2048 bytes, no SECRETCWD1; own control-character fixture clean in both modes; 3 required F9 mutants killed; DEFAULT truncation of reason (<=160) holds; E1 pids never include protected/inherited nodes in 4500 random trees; TerminateGroup invariants (pgid>1, leader ancestor-or-self, members subset, no protected on path) held in all random cases.

## Lesson
A correction that moves lazy initialisation (regex compile) into a path covered by a wall-clock test will turn a green timing test red on slower runners: warm up before timing, or do not time-bound a debug-build test. Also: when a redaction fix is justified by "URL userinfo", enumerate the grammar (empty user, reserved chars in password) not only the examples in the review.
