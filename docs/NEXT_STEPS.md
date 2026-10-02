# Next steps (handoff for the next working session)

Read `docs/REQUIREMENTS.md` first.

## Step 1 — Pre-Design (not started)
Run a tech-lead planning pass with the strongest model (Claude: Agent tool, `model: "fable"`).
Output `docs/PRE_DESIGN.md` containing: problem framing; constraints; ≥2 architecture options with
trade-offs (shared Rust/Go core + native SwiftUI/WinUI, vs Tauri/Flutter/Electron, vs other);
recommendation; per-OS detection heuristics; idle-session detection from local, read-only,
non-credential signals only; graceful termination; Embers' own resource budget; launch at login;
distribution (Homebrew tap + dmg; winget/scoop) with honest Gatekeeper/SmartScreen notes;
which platform ships Phase 1; a phased Executor Goal per phase (allowed files, action → verification
acceptance criteria, stop conditions, non-goals); Questions for the owner, each with a recommended answer.
**Stop after the brief and wait for the owner's decisions before writing code.**

## Working agreement
- Tech lead (Fable): plan, approve each phase, UI/UX direction.
- Executor (Opus): code + tests only.
- After **every** code change: an independent adversarial reviewer (Agent tool, `model: "sonnet"`) —
  review only, no edits; each finding needs a reproducible failure scenario; list what was verified correct.
- Owner does final acceptance (Windows hands-on testing on a real PC; Windows CI on GitHub Actions).
- Verify facts (APIs, flags, versions) instead of recalling them. Report with evidence, not "should work".
- Keep personal info and machine details out of committed files.
- Build constraint on the owner's Mac: Swift 6.3 via Command Line Tools only (no full Xcode).
- UX bar: as smooth as notch utilities like CodexIsland (MIT; study its techniques, attribute any reused code;
  don't copy its name or branding). Apple/iOS design language.
