# Embers — product requirements (v0, pre-design)

Status: requirements gathered; **Pre-Design not started**. Next step is a tech-lead design brief (see "Open design questions").

## Problem

AI coding tools (Claude Code desktop/CLI, Codex app/CLI, etc.) start helper processes — dev servers
(`python -m http.server`, uvicorn, vite…), headless browsers, MCP servers, `npx` servers, polling loops
(`until …; do sleep …; done`), whole idle agent sessions. When the session ends, some of them keep
running and eat CPU/RAM. Separately, ordinary apps can run away with resources (real example:
a system mirroring app at ~150% CPU and a 22 GB memory footprint on a 16 GB machine, pushing swap
to ~95% and making `kernel_task` busy — users then mistake `kernel_task` for the culprit).

## What Embers does

1. Periodically scans processes and flags **leftovers** (likely abandoned) and **runaways** (resource hogs).
2. For each, shows *why* it was flagged (e.g. "parent session gone", "idle 2h, 0% CPU, listening on :8000",
   "22 GB memory, 6h"), source (Claude / Codex / other), age, CPU, memory.
3. **Asks before acting.** Per item: **End** / **Keep** / **Always ignore**. Never kills on its own.

## Hard rules

- Never touch OS processes (kernel_task, WindowServer, launchd, csrss, smss, explorer, …) or the
  session the user is actively working in. Explain when something can't be ended (e.g. kernel_task).
- Graceful first: ask apps to quit / SIGTERM (Windows: WM_CLOSE / graceful) → wait → force only if confirmed.
  For app-managed sessions prefer the app's own close/archive path where possible.
- Local only: zero telemetry, no network calls, **never read credentials / keychains / tokens**.
- Light: Embers itself must have a tiny CPU/RAM budget.
- Detection rules generic and configurable (public tool — nothing hard-coded to one machine).

## Platforms

- **macOS** (Apple Silicon first): notch overlay UI on notch MacBooks, menu-bar pill fallback.
- **Windows**: system-tray / top-of-screen equivalent.
- Owner has a Mac (CommandLineTools only, no full Xcode) and a physical Windows PC for hands-on testing;
  Windows CI via GitHub Actions windows runners.

## UX bar

"Slick and butter-smooth" — on par with notch utilities like CodexIsland (black notch pill, hover to peek,
click to expand, spring animations, click-through outside the shape). Apple/iOS design language,
brand color as accent. Must NOT copy CodexIsland's name or branding.

## Distribution

Public MIT repo. macOS: Homebrew tap + .dmg. Windows: winget/scoop/installer (TBD).
Unsigned-build warnings (Gatekeeper / SmartScreen) must be explained honestly in the README —
do not silently strip quarantine in install scripts without an explicit owner decision.

## Open design questions (for the tech-lead brief)

1. Architecture for Mac + Windows: (A) shared core (Rust/Go) + native UI per OS (SwiftUI/AppKit, WinUI 3);
   (B) cross-platform UI (Tauri / Flutter / Electron — be honest about memory footprint);
   (C) other. Judge on: native-feel animation, own resource use, solo maintainability, build chain.
2. Leftover heuristics per OS (orphan/reparenting semantics differ; Windows PPID reuse, Job Objects,
   Toolhelp32/WMI), idle detection (CPU-time delta, listeners, transcript/log mtimes), false-positive control.
3. How to tell an AI-tool session is idle vs active using only local, read-only, non-credential signals.
4. Scan cadence and Embers' own resource budget; launch at login (SMAppService / Run key / Task Scheduler).
5. Which platform ships Phase 1; phased plan with acceptance criteria per phase.
