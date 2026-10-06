> Stored by the tech lead on 2026-10-06. Reviewer original, verbatim (no personal identifiers present). Repro files referenced as `scratchpad/r3fx/*` lived in the reviewer's scratchpad.

# Phase 1a round 3 correction re-review (head 20aa6d4)

Verdict: PASS. No Blocking/Major/Minor-regression found. N1-N5, N7-N9 and the J1 supplement are fixed; 4 residual Suggestions (none gate merge).
CI (gh pr checks 2, head 20aa6d4, run 37407098510): deny, gitleaks, macos-build, test macos/ubuntu/windows all pass.
Reviewer worktree scratchpad/r3 (detached 20aa6d4), removed after; repro files scratchpad/r3fx/ (u0..u19.json, gen.py).

## Status of N1-N9, J1
- N1 fixed. CI test x3 green (ubuntu 34 s, windows 45 s). Timed region now ~15 ms alone (debug). Residual margin when the whole rules binary runs in parallel: 30 idle runs max 42 ms; 20 busy procs + 10 runs max 39 ms; 0 failures in ~56 binary runs. One transient red of classify_1000 was seen once in a `cargo test --workspace` run during my mutation loop (not reproducible in 56 further runs) -> S1.
- N2 fixed (redis.json: both leaks now `redis://:***@cache:6379/0`). 20 URL-grammar cases (r3fx/gen.py): empty user, `/`, `@`, `%40`, `#`, `?` in password, `[::1]`, two URLs in one token, `?password=`, empty password: no leak. Over-masking only (acceptable): `http://host:8080/@scope/pkg` -> `http://host:***@scope/pkg`; `https://user:S@host:99/a/b@c/d` -> `https://user:***@c/d` (host lost). `http://user@host/path`, `git@github.com`, `https://github.com/a/b@v1` untouched. Residual leaks: S2. Mutant (old regex) -> url_passwords_with_empty_user_or_reserved_chars_are_masked RED.
- N3 fixed. d62/d64/d65/d70.json all `Inform [900] Frontmost`, leaf shown as Inform. Mutant (BFS depth cap 60) -> protection_reaches_any_depth + every_fixture RED.
- N4 fixed. Mutant (drop `is_app_bundle || is_system_path` in session_group) -> app_bundle_never_leads_a_group_kill + every_fixture RED (without classify_1000 involvement).
- N5 fixed. `embers scan --config r2/empty.toml` prints `embers: warning: [protected] names_macos is empty...` to stderr; --json stdout stays clean. Caveats S3 (printed once per fixture file, 30x on a directory scan; no CLI test guards the eprintln).
- N6 judgement: not an actual risk, no upgrade (see below).
- N7 fixed: a9 text now `part of the system ... · 30 GB memory · running 5 h · ...` (hog fact survives the 160 cap).
- N8 fixed for the reported repro and for is_invisible chars (U+200B-200F, 2060-2064, 00AD, FEFF). Mutant (remove filter) -> invisible_characters_cannot_hide_a_secret_flag RED. Residual S4.
- N9 fixed: a18 hint `Resume: claude --resume 'x[2J token=*** ...' (in '/work/api_key=***')` keeps closing quote and `)`. Mutant (re-add redact_text in merge) -> quoted_hint_keeps_its_closing_quote RED. red.json/ctl.json/ctl2.json: only SECRETVAL36 and SECRETVAL45 (Deferred) remain; hints = 2048 bytes max.
- J1 supplement fixed: protect.rs doc table (walks-stop / inherit) and defaults.toml comment match code (Kernel/SystemDaemon/LiveTty not inherited; Frontmost/BusySession/NotOwned/SelfProcess/Allowlisted inherited). j1_child_of_launchd_still_actionable -> `Terminate [2100]`; mutant (make SystemDaemon inheritable) -> os_root_protection_is_not_inherited + 11 other tests RED.

## N6 (merged action Terminate but pids = TerminateGroup-sized)
Reproduced shape (r2/seed1010_idle_runaway_group.json) but no failing scenario: the extra pids are exactly what E1 validated for TerminateGroup (pgid>1, leader ancestor-or-self, members subset of leader's subtree, none protected); a protected/inherited-protected node never enters pids (E1 invariants + protection filter; 4500-tree differential in round 2). The user-visible scope is accurate: helper_count, footprint and cpu are computed from the merged pids, so the consent surface shows the true size. Only inconsistency is semantic ("Terminate" is documented as tree-of-root in the round 1 arbitration, 1c says "SIGTERM to pgid or tree"). Recommendation for 1c: act on `Ember.pids` only, never re-derive the tree from root_pid. Stays Suggestion.

## New suggestions (none block)
- S1 classify_1000_processes_is_fast bound 50 ms vs 28-42 ms observed when the binary's tests run in parallel (15 ms alone); one unexplained red seen locally. If it flakes in CI: raise the bound (e.g. 250 ms) or assert on a count/ratio instead of wall clock.
- S2 URL password containing a raw `'`, `"` or space is not masked: `postgres://u:pa'ss@h/db`, `postgres://u:pa ss@h/db` (as one argv element), `postgres://u:pa"SECRETO@h/db` -> unchanged in text and --json (r3fx/u12, u15, u19). `'` is a legal RFC 3986 sub-delim, so not purely non-conformant; the class excludes quotes (probably to avoid crossing a surrounding quote). Pre-existing grammar, not a regression of 20aa6d4.
- S3 N5 warning: emitted inside the per-fixture loop, so once per fixture file; no test covers the CLI print (deleting the 3 lines keeps all tests green). Fine for fixture mode; do not copy the pattern into 1b scan loop.
- S4 `is_invisible` (used by redact_argv/redact_text flag matching) is narrower than the CLI `needs_escape` set: flag names containing U+202E, U+2028, U+0085 or tag chars U+E0000-E007F (e.g. `--toke<U+202E>n SECRETVAL`) are not recognised by either redact_argv or redact_text and the value leaks (checked with a scratch test). The target program would not treat such a flag as `--token` either, so practical risk is nil; for consistency strip the same set.

## Verified correct
cargo test --workspace --locked all pass (35 rules, 15 redact, ...); clippy --all-targets -D warnings clean; cargo fmt --check clean; cargo deny check ok (advisories/bans/licenses/sources); gitleaks dir . no leaks; `grep -rn unsafe crates/` only forbid; every_fixture count 30 and all expectations match; --json stdout unaffected by warnings; worktree clean after each mutation and removed.

## Lesson
A timing test fixed by "warm-up outside the timer" still has only ~1.2x headroom when run in parallel with the other tests of its binary; evaluate wall-clock tests under the default parallel harness, not alone. And when a sanitizer set is introduced for one purpose (is_invisible), reuse the single canonical set (needs_escape) everywhere instead of keeping two lists.
