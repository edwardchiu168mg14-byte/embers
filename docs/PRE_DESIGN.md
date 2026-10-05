# Embers — Implementation Design Brief (Pre-Design)

Status: **brief complete, awaiting owner decisions** (see §12). No code yet.
Date: 2026-10-05. Author role: tech lead. Process tier: **Full** (new project, two OSes, new toolchain, process-killing = high failure cost).
Inputs: `docs/REQUIREMENTS.md`, `docs/NEXT_STEPS.md`, read-only probes on the owner's Mac (2026-10-02), CodexIsland 0.3.0 source (MIT, `github.com/ericjypark/codex-island`, commit `326f980`), web checks listed in §13.

Evidence tags used throughout: **[measured]** = observed on the owner's Mac on 2026-10-02; **[header]** = read from the macOS SDK headers shipped with Command Line Tools; **[source]** = read in CodexIsland source; **[web]** = web search/docs, date 2026-10-02; **[unverified]** = not confirmed, must be tested before relying on it.

---

## 0. Executive summary

- **Problem**: AI coding sessions leave processes behind (idle agent sessions + their MCP helpers, dev servers, headless browsers, polling loops), and unrelated apps occasionally run away with RAM/CPU. Nothing on the machine notices; the user blames `kernel_task`.
- **Recommended architecture (Option C)**: a **shared Rust core** (`embers-core` library + `embers` CLI: scan → classify → explain → extinguish) and **one UI per OS**: a **native SwiftUI/AppKit notch overlay on macOS** (CodexIsland-grade motion is only reachable natively) and a **Tauri v2 tray flyout on Windows** (links the same Rust core in-process; WebView2 is already on Windows 11). Electron is rejected on footprint grounds; a single cross-platform UI is rejected because the macOS notch tricks are AppKit-specific.
- **Ship order**: Phase 0 bootstrap → **Phase 1 core CLI (macOS-validated, Windows-compiled in CI)** → **Phase 2 macOS notch app** → **Phase 3 Windows adapter + tray app (owner tests on real PC)** → Phase 4 distribution/autostart/polish.
- **Key detection insight** [measured]: Claude Code sessions launched from the desktop app write `~/.claude/sessions/<pid>.json` containing `"status":"idle"|"busy"` and `statusUpdatedAt` — a read-only, non-credential, *authoritative* idle signal. Each session is also its own **process group**, so one `kill -TERM -<pgid>` ends the session and all its MCP helpers together.
- **Hard non-negotiables**: never auto-kill; never touch OS/system processes or the active session; local-only, zero telemetry, zero network; never read credentials; Embers itself ≤ ~1 % CPU average and ≤ ~60 MB total.

---

## 1. Problem framing

The real problem is not "processes exist", it is **nobody is accountable for a process after its creator forgets it**. AI agents are prolific, short-lived creators: they spawn a dev server to test, a headless browser to screenshot, an `until … sleep` loop to poll, then the conversation ends. The processes survive because (a) the agent harness keeps the session alive but idle, (b) the shell that spawned them has gone away and `launchd` adopted them, or (c) the thing is a normal app that quietly grew to 30 GB.

Embers' job is therefore **attribution + explanation + consent**:
1. attribute each suspicious process to an origin (which AI tool / which session / which shell / none),
2. explain in one sentence why it is probably abandoned or harmful,
3. ask the human, and act only on an explicit answer, gracefully first.

It is *not* an Activity Monitor clone, not a general task manager, and not an auto-cleaner.

## 2. Verified facts, key assumptions, minimum sufficient solution

### 2.1 Verified facts (owner's Mac, anonymised) [measured unless tagged]

Machine: Apple Silicon (M-series, 10 cores) MacBook with notch, 16 GB RAM, macOS 27.0 (Darwin 27). Toolchain: Swift 6.3.3 via Command Line Tools only (`xcode-select -p` → `/Library/Developer/CommandLineTools`); `xcodebuild`, `actool`, `ibtool` **absent**; `codesign`, `plutil`, `lipo` present; Node/npx present; **Rust (`cargo`/`rustc`) and Go absent**.

Process landscape at probe time:
- No true orphans among AI-related processes; every `claude`/`codex` process hung under a living app. **The leftovers were idle sessions, not orphans.**
- The Claude desktop app spawns per Code-tab session: `Claude.app/Contents/Helpers/disclaimer --pgroup -- …/claude --output-format stream-json …` → child `claude` → children `mcp-server-…` (Pencil), `npm exec better-icons mcp` → `node …/better-icons mcp`. **All members share one PGID (= the `disclaimer` pid)**; `sess` 0, no TTY. An idle session's `claude` process was ~80 MB RSS; a busy one ~280 MB.
- `~/.claude/sessions/<pid>.json` exists per live `claude` process. Observed fields: `pid`, `sessionId`, `cwd`, `startedAt`, `procStart`, `version` (`2.1.286`), `entrypoint` (`"claude-desktop"`), `name`, `status` (`"idle"` / `"busy"`), `statusUpdatedAt` (ms epoch), `messagingSocketPath` (`/tmp/cc-socks/<pid>.sock`). A sibling `<pid>.<hex>.key` file exists — **treat as secret, never open**. Undocumented format; version-specific.
- Resumed sessions carry `--resume=<sessionId>` in argv; fresh ones do not. Transcripts live at `~/.claude/projects/<slug>/<sessionId>.jsonl` where `<slug>` = cwd with `/` and `_` replaced by `-`; mtime tracks activity. A `<sessionId>.desktop-released.json` (`{"v":1,"releasedAt":…,"reason":"delete"}`) appears when the desktop app releases a session.
- Codex app (`ChatGPT.app`) runs `codex app-server`, `codex exec-server --remote …`, several `Codex (Renderer)` helpers, a Computer Use service; ~700 MB total. Codex CLI transcripts: `~/.codex/sessions/**/*.jsonl` (≈1,200 files). No idle-status file observed for Codex.
- Runaway example: **iPhone Mirroring** (system app, parent = launchd) at 177 % CPU, `footprint` **30 GB** (28 GB "Malloc Small"), running 6 h; `rapportd` 89 % CPU; swap **12.6 / 13.3 GB used**; `kernel_task` 24–35 % CPU — the latter is the kernel doing compression/swap and thermal work, **not killable and not the cause**.
- Listening sockets at probe time were only system/other-app ones (ControlCenter, a Git client, OneDrive, rapportd) — i.e. no stray dev servers *at that moment*; the dev-server pattern is from the owner's reported history, not measured.
- A SwiftUI + AppKit + ServiceManagement + libproc probe **compiles and runs with CLT-only `swiftc`** (22 s cold compile). `SMAppService.mainApp.status` works (raw 3 = `.notFound` for an unbundled binary [header]); `proc_pid_rusage` returns `ri_phys_footprint`; `proc_listpids`/`proc_listchildpids` work; `NSScreen.safeAreaInsets.top` = 32 pt, `auxiliaryTopLeftArea/RightArea` give the notch geometry (notch width = 1470 − 645.5 − 645.5 = 179 pt on this display).
- CodexIsland builds with **bare `swiftc` + hand-written Info.plist + `lipo` + ad-hoc `codesign`** (`build.sh`) — no Xcode, no SwiftPM, no XCTest; tests are plain `swiftc` harnesses. Its Homebrew cask postflight runs `xattr -dr com.apple.quarantine` (the behaviour the owner does not want copied silently).

### 2.2 Key assumptions (only those that change the design)

| # | Assumption | If wrong → |
|---|---|---|
| A1 | `~/.claude/sessions/<pid>.json` `status` is maintained for CLI-launched sessions too, and on Windows (`pidDomain` field hints at cross-platform intent). | Fall back to CPU-delta + transcript-mtime heuristic (confidence drops from High to Medium). Tested in Phase 1 (mac CLI) and Phase 3 (Windows). |
| A2 | Killing the session's process group (SIGTERM) is tolerated by the Claude desktop app (tab shows "ended", transcript stays resumable). | Default action for app-managed sessions becomes "open the app and archive" (instruction only), kill becomes opt-in. Tested in Phase 2 human QA. |
| A3 | The owner accepts installing the Rust toolchain (`rustup`) on the Mac and on the Windows PC. | Core language changes to Go (see Option C′) — same architecture, different crate ecosystem. |
| A4 | A native SwiftUI overlay is required for the macOS motion bar (CodexIsland parity). | If the owner accepts "good enough" motion, Tauri could cover both OSes (Option B2) and drop the Swift codebase. |

### 2.3 Minimum sufficient solution

A CLI that prints a classified, explained list of leftovers and can end one of them gracefully — usable from a terminal on day one — plus the thinnest UI per OS that renders that list and asks. Everything else (always-ignore rules, snooze, stats, auto-update) is Phase 4 or later.

## 3. Current constraints (hard)

- **No Xcode on the Mac**: no `.xcodeproj`, no asset catalogs, no Interface Builder, no XCTest reliability. Build = `swiftc` script (proven by CodexIsland and by the probe). App icon must be a pre-made `.icns` (CodexIsland has `scripts/make-icns.sh` using `iconutil`/`sips` — both ship with macOS [source]).
- **No Rust/Go installed yet** → any shared-core option adds a toolchain install (owner decision Q1).
- **16 GB RAM, swap already saturated** on the owner's Mac → Embers' own footprint matters and a Windows VM on the Mac is not realistic; Windows work goes to CI + the owner's physical PC.
- **Unsigned builds**: no Apple Developer ID, no Windows code-signing certificate (owner decision Q3).
- **Public MIT repo**: nothing machine- or person-specific may be hard-coded; all paths/patterns are defaults in a config file.
- **Zero network**: no telemetry, no update pings unless the owner opts in (Q4).

## 4. Relevant files inspected

CodexIsland (scratch clone, commit `326f980`):
- `build.sh` (whole file) — swiftc/lipo/Info.plist/codesign recipe. `release.sh` — ad-hoc sign + DMG via `create-dmg` + Sparkle appcast. `CLAUDE.md` — release hazards (Sparkle, quarantine).
- `Sources/Window/IslandWindowController.swift:1-275` — borderless `NSWindow`, level `.popUpMenu`, `collectionBehavior [.canJoinAllSpaces, .stationary, .ignoresCycle]`, global+local `mouseMoved` monitors toggling `ignoresMouseEvents`, occlusion observer, screen-lock fade.
- `Sources/Window/IslandHostingView.swift:1-134` — `hitTest` limited to the island rect, `acceptsFirstMouse`, two-finger swipe paging.
- `Sources/Model/NotchInfo.swift:1-77` — notch geometry from `safeAreaInsets` + `auxiliaryTop*Area`, menu-bar height rule.
- `Sources/Model/IslandModel.swift:1-200` — `compact / peek / expanded` state machine and size rules.
- `Sources/Theme/Animations.swift:1-83` — spring/curve constants (`openMorph` spring response 0.42 damping 0.82; `closeMorph` 0.30/0.88; `strongEaseOut` cubic-bezier(0.23,1,0.32,1) 280 ms; press scale 0.94 @110 ms).
- `Sources/Views/IslandShape.swift` — `UnevenRoundedRectangle` with `.continuous` bottom corners r=14.
- `Sources/Model/LaunchAtLoginStore.swift` — `SMAppService.mainApp.register()/unregister()`.
- `Sources/Model/WindowOcclusionStore.swift`, `ExpandedFrameRate.swift`, `docs/PERFORMANCE.md` — pause work when occluded; 120/60/30 Hz pacing; main-thread stall benchmarking.

Local machine (read-only): `~/.claude/sessions/*.json` (fields only; `.key` files not opened), `~/.claude/projects/<slug>/` listing, `~/Library/Application Support/Claude/` listing (config read for **key names only**; it contains `oauth:tokenCache` → Embers must never read this file), `~/.codex/` listing (`auth.json` not opened), `ps`/`lsof`/`footprint`/`vm_stat`/`launchctl list` output, SDK headers `SMAppService.h`, `libproc.h`, `sys/resource.h`, `NSRunningApplication.h`, `NSScreen.h`.

## 5. Architecture options and trade-offs

Evaluation axes (from REQUIREMENTS §Open question 1): (i) macOS motion parity with CodexIsland, (ii) Windows equivalent UX, (iii) Embers' own footprint, (iv) solo maintainability, (v) build chain given CLT-only Mac + Windows via CI/owner PC.

### Option A — Shared core (Rust) + fully native UI on both OSes (SwiftUI/AppKit + WinUI 3)

- (i) **Best possible** on macOS. (ii) Best possible on Windows (Fluent, Mica, native toasts).
- (iii) Core ~5–10 MB RSS; Swift app ~30–50 MB; WinUI 3 app ~40–80 MB [unverified estimate].
- (iv) **Three codebases in three languages** (Rust, Swift, C#/XAML). WinUI 3 + Windows App SDK packaging is the most ceremony-heavy desktop stack Microsoft has; unpackaged WinUI apps are possible but fiddly. Solo maintainer with no daily Windows machine → the Windows UI will rot first.
- (v) Mac: swiftc script. Windows: MSBuild/.NET SDK on `windows-latest` works, but every UI fix needs a CI round-trip or the owner's PC.

### Option B — One cross-platform UI framework for everything

**B1 Electron** — rejected. 150–400 MB idle across 3–5 processes [web, vendor-neutral comparisons]; a tool whose purpose is to complain about memory hogs cannot itself be one. Also no notch/click-through primitives without native addons.

**B2 Tauri v2 (Rust + system WebView)** — plausible. (iii) ~40–80 MB idle [web]; macOS uses WKWebView, Windows uses WebView2 (pre-installed on Windows 11; Tauri's bundler can embed the bootstrapper for Windows 10 [web]). Tray API, `setAlwaysOnTop`, `setIgnoreCursorEvents`, transparent windows exist [web, docs.rs/tauri + v2.tauri.app]; `alwaysOnTop` on macOS needs the `macOSPrivateApi` flag [web]. (i) The notch illusion needs per-pixel click-through (hit-test only inside the pill) and a global mouse monitor — Tauri exposes only a whole-window `setIgnoreCursorEvents`, so the CodexIsland trick must be re-implemented in Rust via `objc2` against the raw `NSWindow`, and animation runs in CSS inside WKWebView (120 Hz ProMotion rendering in WKWebView **[unverified]**). Achievable but it is the *hardest* part of the app done in the *least* ergonomic way, and "feels like a web page" risk is real. (iv) One UI codebase (HTML/CSS/TS) + Rust — attractive for a solo dev. (v) Builds on both runners; `cargo tauri build` is well-trodden.

**B3 Flutter desktop** — own renderer (Impeller) gives smooth custom animation, but notch overlay/click-through again requires platform channels + native code on both OSes, adds Dart as a third language, ~80–150 MB idle [unverified estimate], and Apple-feel must be hand-built. No advantage over B2 here. Rejected.

### Option C (recommended) — Shared Rust core + native SwiftUI on macOS + Tauri v2 on Windows

- **Core** `embers-core` (Rust library) + `embers` (CLI binary): process snapshot, origin attribution, rule engine with human-readable reasons, graceful termination, TOML config, NDJSON output. Pure logic is unit-tested against **synthetic process-tree fixtures** so classification tests run identically on `ubuntu`/`macos`/`windows` runners; OS adapters are thin and separately tested.
- **macOS UI** (Phase 2): SwiftUI/AppKit notch overlay, spawns `embers watch --json` as a child process and reads NDJSON over stdio; sends actions via CLI invocations (`embers extinguish <pid> …`). No FFI, trivially debuggable (`embers watch --json | jq` in a terminal reproduces exactly what the UI sees).
- **Windows UI** (Phase 3): Tauri v2 app with tray icon + flyout + toasts; **links `embers-core` in-process** (same crate, no IPC), web UI in Fluent-leaning styling.
- (i) Native on macOS — the only path that guarantees parity. (ii) Windows gets the idiomatic tray-flyout experience (no notch to imitate). (iii) Core ≤ 10 MB; Swift overlay ≤ 50 MB; Tauri ≤ 80 MB — within budget. (iv) Two UIs, but each in the stack best suited to its platform, and the hard logic lives once in Rust with the strongest test story. (v) Mac builds with swiftc + cargo locally; Windows builds in CI and is hand-tested on the owner's PC; the Rust core cross-checks on all three runners every push.

**C′ variant — core in Go instead of Rust.** Go cross-compiles to Windows from the Mac with zero extra tooling (`GOOS=windows go build`), and the language is simpler. Costs: Go binary ~10 MB and ~15–25 MB RSS vs Rust's few MB; `gopsutil` on darwin historically shelled out to `ps` for some process metrics **[unverified for current versions]**, which would violate the "no subprocess per poll" budget; and the Windows UI could no longer link the core in-process (Tauri is Rust) — it would fall back to the same stdio protocol the Mac uses (acceptable). Choose Go only if the owner strongly prefers it (Q1).

### Decision matrix

| | A native×2 | B1 Electron | B2 Tauri×2 | B3 Flutter | **C Rust core + Swift + Tauri(Win)** |
|---|---|---|---|---|---|
| macOS motion parity | ●●● | ● | ●● (risky) | ●● | **●●●** |
| Windows UX | ●●● | ●● | ●● | ●● | **●●** |
| Own footprint | ●●● | ✗ | ●● | ● | **●●●/●●** |
| Solo maintainability | ● | ●● | ●●● | ● | **●●** |
| Build chain (CLT Mac + CI Win) | ● | ●● | ●●● | ●● | **●●●** |

**Recommended: Option C.** Reason: the only axis where C is not top is maintainability, and that cost is bounded (the Swift app is ~2–3 k lines by CodexIsland's precedent, and all risky logic is shared and tested once). A/B2/B3 each sacrifice a non-negotiable (B: macOS parity or footprint; A: maintainability).

## 6. Detection design (platform-neutral rules, per-OS signals)

### 6.1 Vocabulary

- **Ember**: a process (or process group) Embers proposes to end. Each ember has `category`, `origin`, `confidence`, `reason`, `suggested_action`, `recovery_hint`.
- **Origin**: `claude-desktop`, `claude-cli`, `codex-app`, `codex-cli`, `terminal`, `unknown`. Determined by walking ancestors and matching **configurable exe-path / cmdline patterns** (defaults ship for Claude and Codex; users add their own, e.g. Cursor, Windsurf).
- **Protected**: never shown as actionable; may be shown as *informational* ("heavy, cannot be ended by Embers").

### 6.2 Categories (defaults; all thresholds in config)

| Cat | Name | Rule (all conditions) | Default action | Confidence |
|---|---|---|---|---|
| **E1** | Idle AI session | Process is an AI-session root (e.g. `claude`, `codex`) whose host reports `status=idle` for ≥ `idle_minutes` (default 30) **and** CPU-time delta over the last `window` (default 5 min) < 1 % **and** no child spawned in the window. Includes its whole process group / descendant tree (MCP helpers, shells). | Terminate group (graceful) or "archive in app" instruction (A2) | High if status file present; Medium on heuristics only |
| **E2** | Orphaned dev process | Parent is gone (macOS: PPID = 1 but exe is **not** a launchd-managed service and not an `.app` bundle; Windows: PPID not alive **or** parent created *after* child → PID reuse) **and** exe matches `dev_tool_patterns` (python/node/uvicorn/vite/http.server/headless Chrome/gcloud/ngrok/`sleep`/`bash -c` …) **or** owns a localhost `LISTEN` socket. | Terminate (graceful) | High |
| **E3** | Idle dev server | Owns a `LISTEN` socket, zero established connections and < 1 % CPU for ≥ `idle_minutes`, and its origin session is E1 or gone. | Terminate (graceful) | Medium |
| **E4** | Background loop | Shell/`sleep` chain (e.g. `bash -c 'until …; do …; sleep N; done'`) older than `idle_minutes` whose origin is E1 or gone. | Terminate (graceful) | Medium |
| **E5** | Runaway (informational first) | Any user process (AI or not) with footprint > `hog_memory` (default: max(8 GB, 50 % physical RAM)) **or** CPU > `hog_cpu` (default 100 %) sustained ≥ `hog_minutes` (default 10). | **Quit app gracefully** (never force by default); for system-owned apps show "cannot be ended by Embers — quit it yourself" | High (facts), action is advisory |

Rules compose: a process may be E1 *and* E5. Reasons are generated from the facts that fired ("idle 2 h 14 m · 0 % CPU · 3 MCP helpers · host says idle"), never from the rule name alone.

### 6.3 Never-touch list (protected), enforced in the core *before* any kill call

- pid 0/1 and anything the OS marks as kernel/system: macOS `kernel_task`, `launchd`, `WindowServer`, `loginwindow`, `Finder`, `Dock`, `SystemUIServer`, `coreaudiod`, anything under `/System/Library/` or `/usr/libexec/` (user-facing `/System/Applications/*.app` are E5-eligible for *graceful quit only*); Windows `System`, `smss`, `csrss`, `wininit`, `winlogon`, `services`, `lsass`, `svchost`, `explorer`, `dwm`, `fontdrvhost`, anything under `%SystemRoot%\System32` [unverified list completeness — treat as config default, extend].
- Processes not owned by the current user (never escalate privileges).
- Embers' own processes.
- The **frontmost app** and any AI session with `status=busy` or `statusUpdatedAt` within `idle_minutes`.
- Interactive shells attached to a live TTY (user is typing in them).
- The user's allow-list (by exe path, bundle id, or cmdline regex).

### 6.4 macOS signals (read-only, non-credential)

| Signal | API / file | Status |
|---|---|---|
| Process list, PPID, PGID, start time, exe path, argv | `proc_listpids`, `proc_pidinfo(PROC_PIDTBSDINFO)`, `proc_pidpath`, `sysctl KERN_PROCARGS2` (argv) | [header]/[measured] via probe + `ps` |
| CPU time, physical footprint | `proc_pid_rusage(RUSAGE_INFO_V6)` → `ri_user_time`, `ri_system_time`, `ri_phys_footprint` (same number `footprint(1)` shows) | [header]/[measured] |
| Orphan semantics | Child of a dead parent is re-parented to `launchd` (PPID 1); distinguish from launchd-managed jobs via `launchctl print`-equivalent: simpler heuristic = PPID 1 **and** not an `.app` bundle **and** not in `/System`, `/usr/libexec`, `/Library/PrivilegedHelperTools`, `~/Library/LaunchAgents` labels | [measured] (all current PPID-1 user processes were daemons/agents/apps) |
| Listening sockets | `proc_pidinfo(PROC_PIDLISTFDS)` + `PROC_PIDFDSOCKETINFO` (what `lsof -iTCP -sTCP:LISTEN` does) | [header] / **[unverified]** that the FD walk is cheap enough per poll → only run for candidate pids |
| Claude session idle | `~/.claude/sessions/<pid>.json` (`status`, `statusUpdatedAt`, `sessionId`, `cwd`); validate `pid` alive **and** `procStart` matches the live process start time (guards stale files) | [measured]; format **undocumented** → parse defensively, feature-detect |
| Claude transcript activity | mtime of `~/.claude/projects/<slug>/<sessionId>.jsonl` | [measured] |
| Codex activity | mtime of newest `~/.codex/sessions/**/*.jsonl`; process names `codex app-server`, `codex exec-server`, `codex` | [measured]; no idle-status file → heuristic only |
| App identity / graceful quit | `NSRunningApplication` (`bundleIdentifier`, `isActive`, `terminate()`, `forceTerminate()`) | [header] |
| Frontmost app | `NSWorkspace.shared.frontmostApplication` | [header] |
| Forbidden | `~/.claude/sessions/*.key`, `~/Library/Application Support/Claude/config.json` (contains OAuth token cache), `~/.codex/auth.json`, Keychain, any `*.env` | policy |

### 6.5 Windows signals (to be verified on the owner's PC in Phase 3)

| Signal | API | Status |
|---|---|---|
| Process list, PPID, exe, cmdline | `CreateToolhelp32Snapshot` + `Process32First/Next` (`th32ParentProcessID`), `QueryFullProcessImageNameW`, `NtQueryInformationProcess`/WMI for cmdline (Rust `sysinfo` wraps these) | [web] |
| PPID reuse / orphan | **No re-parenting on Windows**: orphan keeps a stale PPID. Rule: parent is valid only if it exists **and** `GetProcessTimes(parent).creation ≤ child.creation`; otherwise treat as orphan | [web: Old New Thing 2015-04-03; trainsec process-tree] |
| CPU, memory | `GetProcessTimes`, `GetProcessMemoryInfo` (`PrivateUsage` as footprint analogue) | [web/docs] |
| Listening sockets | `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_LISTENER)` | [docs] **[unverified in Rust ecosystem; may hand-roll via `windows` crate]** |
| Job Objects | Could classify trees if the AI host used jobs; unknown → not relied upon | [unverified] |
| Claude Code | Native installer places `%USERPROFILE%\.local\bin\claude.exe` [web]; npm installs run `node.exe` with `…\@anthropic-ai\claude-code\cli.js` [inference]; Desktop app has a Code tab on Windows [web] but its child-process layout is **[unverified]**; `%USERPROFILE%\.claude\sessions\<pid>.json` presence/format **[unverified]** (A1) |
| Codex | Native `codex.exe` (PowerShell installer / winget msstore) or `codex.cmd` shim → node for npm installs [web]; app on Windows [web]; process tree **[unverified]** |
| Frontmost | `GetForegroundWindow` → `GetWindowThreadProcessId` | [docs] |

### 6.6 Termination protocol (core, both OSes)

1. **Pre-flight**: re-snapshot the target; abort if pid/start-time changed (PID reuse), if protected, if now busy/frontmost.
2. **Graceful**: macOS — for app bundles `NSRunningApplication.terminate()` (equivalent to Quit); for AI session groups `kill(-pgid, SIGTERM)`; for plain processes `SIGTERM` to the tree (children first, then parent, by validated PPID). Windows — GUI processes: enumerate top-level windows of the pid and `PostMessage(WM_CLOSE)`; console/CLI processes: there is no safe graceful signal from another console (`GenerateConsoleCtrlEvent` requires attaching to the target console and hits every process in it) → go to step 4 after the grace period, and say so in the UI.
3. **Wait** `grace_seconds` (default 5), polling liveness.
4. **Force** only if the user confirmed "force if needed" (checkbox default **on** for E1–E4, **off** for E5): macOS `SIGKILL` / `forceTerminate()`; Windows `TerminateProcess` on the validated tree.
5. **Report** outcome per pid (ended gracefully / forced / failed: reason e.g. access denied) and the **recovery hint**: for Claude sessions "resume with `claude --resume <sessionId>` in `<cwd>`" (transcript persists on disk); for dev servers the exact command line so the user can relaunch.

Mis-kill recoverability is therefore: transcripts and sessions survive (resumable); dev servers are re-runnable from the shown command; apps are only ever *asked* to quit unless the user ticks force.

### 6.7 Secret hygiene

Command lines can carry secrets (`--api-key`, `TOKEN=`). The core redacts values after `(?i)(key|token|secret|password|passwd|auth)[=\s:]` before emitting NDJSON; nothing is ever persisted to disk except the TOML config and an optional local action log (off by default). Observed Claude argv contained only tool allow-lists, no secrets — redaction is defence in depth.

## 7. UI/UX direction

### 7.1 macOS — notch overlay (Phase 2)

States (mirrors the proven CodexIsland state machine [source]): **compact** (black silhouette flush with the notch, width = notch + 2×38 pt tabs) → **peek** on hover (widens by two pill slots; shows an ember count glyph and one-line headline, e.g. "3 embers · 1.2 GB") → **expanded** on click (panel ≈ 420–520 pt wide, intrinsic height, max ~6 rows then scroll).

Signal language: **calm by default** — no ember: silhouette only, no glow. Embers found: a warm amber hairline glow that *breathes* (opacity 0.25↔0.45 over 2.4 s, ease-in-out) — never strobes; E5 runaway: ember-red tint. Respect **Reduce Motion** (no breathing, static glow) and **Low Power Mode** (glow only on hover). Colour tokens: background `#000`, ember amber as the brand accent (owner to pick exact hex — Q6), red for runaway, system grey for secondary text; SF Pro / `.rounded` for numerals; `contentTransition(.numericText())` for counters.

Row anatomy (expanded): leading origin glyph (Claude / Codex / terminal / generic), **name** (e.g. "claude — session 'Line data retrieval'"), one-line **reason**, trailing metrics "2 h 14 m · 0 % · 81 MB". Tapping a row reveals an inline detail (argv redacted, children count, listening ports, recovery hint) with the three actions as capsule buttons: **End** (primary, amber), **Keep** (snooze: default 1 h, long-press for "for today"), **Always ignore** (adds allow-list rule by exe path; confirm with a one-line toast "Ignoring `python … http.server` — undo"). "End" on E5 shows a confirm sheet: "Ask iPhone Mirroring to quit? It's using 30 GB." with a **Force if it doesn't quit** toggle (off). Protected items render with a lock and the explanation ("kernel_task is macOS itself — it's busy because memory is full; end the 30 GB app instead").

Motion spec (SwiftUI, values from CodexIsland, MIT-attributed in `THIRD_PARTY_NOTICES`): open morph `spring(response: 0.42, dampingFraction: 0.82)`; close `spring(response: 0.30, dampingFraction: 0.88)`; content fade `timingCurve(0.23, 1, 0.32, 1, duration: 0.28)`; press scale 0.94 @ 110 ms; row detail expand `spring(0.36, 0.94)`, collapse 200 ms ease-out; use `matchedGeometryEffect` for the row → detail card; `.transition(.opacity.combined(with: .scale(0.96)).combined(with: blur 3))` for list diffs.

Implementation points (AppKit): borderless `NSWindow` subclass (`canBecomeKey = true`, `canBecomeMain = false`), `level = .popUpMenu`, `collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]`, `isOpaque = false`, `hasShadow = false`; **click-through** = `NSHostingView.hitTest` returns nil outside the island rect **plus** global+local `mouseMoved` monitors flipping `ignoresMouseEvents` [source — both are needed]; `acceptsFirstMouse = true`; re-detect geometry on `didChangeScreenParametersNotification`; **pause all timers/animations when `occlusionState` lacks `.visible`** and fade out on `com.apple.screenIsLocked`; no `TimelineView` in the compact state; 120 Hz comes free from SwiftUI springs as long as per-frame work is zero (no layout in `onChange` of hover, no shadows on moving layers; `drawingGroup()` only on the static silhouette). Non-notch Macs / external displays: fall back to a **menu-bar pill** (`NSStatusItem`) with the same expanded panel as an `NSPopover`-like panel.

UI-skill routing (institution `75`): use `npx ui-skills get dimillian/swiftui-ui-patterns` (the `swiftui-pro` name in `75` **no longer exists** in the registry — update `75`) for view structure/perf, and `emilkowalski/emil-design-eng` for motion taste; `better-icons` for glyphs (no emoji). Single taste engine per surface — do not also load Impeccable for the Swift app.

### 7.2 Windows — tray + flyout (Phase 3)

No notch; imitating one is foreign and collides with maximised title bars, so the **primary** surface is a **system-tray icon** whose glyph changes state (dim ember → lit amber ember with a count badge → red for runaway). **Left-click** opens a **flyout** (Tauri undecorated transparent window, 360 × up to 520 px, anchored above the tray like the Wi-Fi/Volume flyouts, 160 ms ease-out slide+fade, closes on focus loss). Same row anatomy and actions as macOS, Fluent spacing, Segoe UI Variable, Mica-like layered background in CSS. **Discovery** of new embers also raises a **Windows toast** with "End" / "Keep" buttons (unpackaged apps need an AUMID + shortcut registration for toasts **[unverified detail; Tauri notification plugin documents it]**). Optional **"top pill" mode** (centred overlay at the top edge, same model as the Mac peek) as an off-by-default setting for users who want the island feel.

## 8. Build, packaging, autostart, distribution

### 8.1 Repo layout (monorepo)

```
embers/
  Cargo.toml                 # workspace
  crates/embers-core/        # lib: snapshot, attribution, rules, actions, config
  crates/embers-cli/         # bin `embers`: scan | watch | explain | extinguish | config
  apps/macos/                # Swift sources + build.sh (swiftc, Info.plist, icns, ad-hoc sign)
  apps/windows/              # Tauri v2 app (src-tauri links embers-core; ui/ in TS)
  docs/                      # REQUIREMENTS, NEXT_STEPS, PRE_DESIGN, reviews/
  .github/workflows/ci.yml   # matrix: ubuntu/macos/windows → cargo test; macos → build.sh; windows → cargo tauri build
```

### 8.2 Toolchains

- Mac (local): `rustup` (stable) — new install (Q1); `swiftc` from CLT (proven). SwiftPM is **not** required; follow CodexIsland's bare-swiftc `build.sh` (universal binary via two `-target` passes + `lipo`, hand-written `Info.plist` with `LSUIElement=true`, `LSMinimumSystemVersion=14.0` so `UnevenRoundedRectangle` is available, `codesign --force --deep --sign -`). Swift unit tests, if any, as plain `swiftc` harnesses; keep logic in Rust so `cargo test` carries the test burden.
- Windows (owner's PC): `rustup` with `x86_64-pc-windows-msvc`, Visual Studio Build Tools (C++ workload), WebView2 runtime (present on Win 11), Node for the Tauri UI build.
- CI: `macos-latest` runs `cargo test` + `apps/macos/build.sh` (CLT-equivalent `swiftc` is present on runners); `windows-latest` runs `cargo test` + `cargo tauri build` (NSIS installer + portable `.exe`); `ubuntu-latest` runs core tests only. Artifacts uploaded per push; releases on tags.

### 8.3 Autostart

- macOS: `SMAppService.mainApp.register()` (macOS 13+) [header]; works only when the app is in `/Applications` and ad-hoc signed (CodexIsland does exactly this [source]); user toggle in settings, default **off** until the user enables.
- Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value (no admin) — `tauri-plugin-autostart` implements this [web]; default off.

### 8.4 Embers' own resource budget (acceptance-tested in Phase 1/2)

- Poll interval default **5 s** (config 2–60 s). Each tick: full pid list + BSD info for all (~600 pids ≈ ms-scale), **rusage/footprint/sockets only for candidates** (AI-origin trees, PPID-1 non-system, prior embers); a full footprint sweep for E5 every **60 s**.
- Budget: core **< 0.5 % CPU average, < 15 MB RSS**; macOS overlay **< 0.3 % CPU idle (0 % when occluded), < 50 MB**; Windows app **< 80 MB**. Zero timers while compact except the poll. Measured with `embers --self-stats` and `footprint`/Task Manager during Phase 1/2/3 acceptance.
- No subprocess spawning per tick on any OS (no `ps`/`lsof`/`tasklist` shell-outs).

### 8.5 Distribution and the unsigned-build truth

- macOS: GitHub Release `.dmg` (via `create-dmg` or `hdiutil`) + Homebrew tap cask. Ad-hoc signature only → Gatekeeper shows "Apple could not verify…". README states plainly: *Right-click → Open → Open Anyway, or `xattr -d com.apple.quarantine /Applications/Embers.app`*; the cask **does not** strip quarantine in `postflight` unless the owner decides so (Q3). The proper fix is an Apple Developer ID + notarization (US$99/yr).
- Windows: portable `.zip` + NSIS `.exe` from Tauri; **scoop** bucket first (JSON manifest, no signing needed); **winget** accepts unsigned installers but SmartScreen will show "Unknown publisher / Windows protected your PC" until reputation builds [web] — README documents "More info → Run anyway". Signing options: OV certificate (~US$200–400/yr), Azure Trusted Signing, or SignPath Foundation's free OSS signing **[unverified eligibility]** (Q3).
- **No auto-update** in v1 (an updater is a network call and a signing trust chain). "Check for updates" = open the Releases page in the browser (Q4).
- Privacy statement in README: local only, no network sockets opened by Embers at all (verifiable with `lsof -p`/Resource Monitor).

## 9. Performance, maintainability, extensibility (brief)

- **Performance**: see §8.4; the architecture keeps hot work in Rust and renders only diffs (ember list keyed by `(pid, start_time)` so SwiftUI/TS keep stable identity).
- **Maintainability**: rules are data (TOML defaults embedded, user overrides merged); every rule yields `reason` strings from facts, so adding a category never touches UI code. Origin adapters are a trait with two implementations (Claude, Codex) and a generic pattern adapter.
- **Extensibility** (not now): more origins (Cursor, Windsurf, Gemini CLI), Linux tray app via Tauri (core already compiles on Linux), per-project allow-lists, history/stats page.

## 10. Code-level suggestions (for the executor)

- Core types: `Snapshot { taken_at, procs: Vec<Proc> }`, `Proc { pid, ppid_validated: Option<pid>, pgid, start_time, exe, argv_redacted, uid, cpu_time_ns, footprint_bytes, listeners: Vec<u16>, is_app_bundle, protected: Option<ProtectReason> }`, `Origin`, `Ember { id, pids: Vec<pid>, category, origin, confidence, reason, metrics, suggested_action, recovery_hint, first_seen }`, `Verdict`.
- Rule engine signature: `fn classify(prev: &Snapshot, cur: &Snapshot, cfg: &Config, hosts: &HostStates) -> Vec<Ember>` — pure, fixture-testable.
- CLI: `embers scan [--json]`, `embers watch [--interval 5] --json` (NDJSON events `snapshot`, `ember_found`, `ember_updated`, `ember_resolved`), `embers explain <pid>`, `embers extinguish <ember-id|pid> [--grace 5] [--force] [--tree]`, `embers config path|show`, `embers --self-stats`.
- Config path: `~/Library/Application Support/Embers/config.toml` (mac), `%APPDATA%\Embers\config.toml` (win) via the `directories` crate.
- Swift side: one `CoreClient` actor owning the child process + `AsyncLineSequence` decoding; UI never computes rules.
- Attribute CodexIsland in `THIRD_PARTY_NOTICES.md` for any reused snippets (MIT requires copyright + licence text). Do **not** copy its name, icon, or branding.

## 11. What the Executor should / must NOT implement

**Should** (across phases, per the Executor Goals below): the Rust core and CLI; the macOS overlay; the Windows Tauri app; CI; honest README install sections; fixtures and tests.

**Must NOT**: auto-terminate anything; read `~/.claude/sessions/*.key`, `~/Library/Application Support/Claude/config.json`, `~/.codex/auth.json`, Keychain or any token store; add telemetry, crash reporting, analytics, or auto-update; shell out to `ps`/`lsof`/`tasklist`/`wmic` in the poll loop; add a quarantine-stripping cask postflight; hard-code any user path, machine name, or account; terminate processes of other users or request elevation; store command lines on disk; pull in Electron; implement Phase N+1 features inside Phase N.

## 12. Questions for the owner (each with a recommended answer)

> **Decided 2026-10-05** — all seven answered (recommended answers adopted) plus a new hard security requirement; see `docs/ARCHITECTURE_DECISION.md` §3 and §3.0. The Phase 0 Executor Goal there supersedes the Phase 0 section below.

1. **Core language: Rust (needs `rustup` on both machines) or Go?** → *Recommend Rust*: smallest footprint, `sysinfo`/`windows` crates, in-process linking with the Tauri Windows UI, strongest CI test story. Go only if you want the simpler language and accept stdio IPC on Windows too.
2. **Windows UI: Tauri v2 (recommended) or native WinUI 3?** → *Tauri*: one Rust toolchain, CI-buildable, 40–80 MB; WinUI 3 is nicer but triples the language count for a solo maintainer.
3. **Signing/notarization budget?** Apple Developer ID US$99/yr removes the Gatekeeper dance; Windows signing US$200–400/yr or a free OSS programme (unverified). → *Recommend*: ship unsigned for v0.x with honest README; revisit at v1.0 if there are external users. Keep the cask free of quarantine stripping.
4. **Any network at all (update check)?** → *Recommend none in v1*; "Check for updates" opens the Releases page.
5. **Default idle threshold for AI sessions**: 30 min? → *Recommend 30 min* with a one-tap "Keep" snooze; runaway defaults 8 GB / 100 % CPU for 10 min.
6. **Brand accent colour** (ember amber — e.g. around `#FF9F0A`-family) and whether to ship a menu-bar pill fallback for non-notch displays in Phase 2 or Phase 4. → *Recommend*: pick the amber now; menu-bar fallback in Phase 2 (external monitors are common).
7. **Kill vs "archive in app" for Claude desktop sessions** (assumption A2): acceptable to end the process group directly if Phase 2 testing shows the app tolerates it? → *Recommend yes*, with the resume hint shown; otherwise default to the instruction.

## 13. Unverified items (must be tested before relied upon)

- A1: `~/.claude/sessions/<pid>.json` presence/format for CLI-launched sessions and on Windows; file cleanup on exit (both observed files belonged to live pids, so staleness is untested → always validate pid + start time).
- A2: Claude desktop app behaviour when a session's process group receives SIGTERM.
- WKWebView 120 Hz rendering (irrelevant under Option C, relevant if B2 is chosen).
- Cost of per-pid socket enumeration on macOS (`PROC_PIDLISTFDS`) — restrict to candidates and measure.
- Windows: Claude Desktop Code-tab child-process layout; Codex app/CLI process tree; `GetExtendedTcpTable` via Rust crates; toast registration for unpackaged apps; completeness of the protected-process list.
- `gopsutil` darwin shell-out behaviour (only matters for Option C′).
- Free OSS code-signing eligibility (SignPath) and Azure Trusted Signing for individuals.
- Memory figures for Tauri/Electron/Flutter are third-party comparisons, not measured here.

Web sources consulted (2026-10-02): docs.rs `tauri::window::Window`; v2.tauri.app window/tray/plugins references; Microsoft "The Old New Thing" 2015-04-03 on `CreateToolhelp32Snapshot` parent IDs; `microsoft/winget-pkgs` discussions on unsigned installers; Claude Code Windows install guides (`%USERPROFILE%\.local\bin\claude.exe`); OpenAI community "Codex app is now on Windows"; `code.claude.com/docs/en/desktop`; swift.org Windows install page; Rust forum threads on `x86_64-pc-windows-gnu` cross-compiling.

---

# Executor Goals (phased)

Common to every phase: process tier **Full**; executor = Opus (code + tests only, no commits, no pushes, never runs `extinguish` against a real process without the owner present in Phase 1 QA); after each phase an **independent adversarial Sonnet review** (review only; each finding needs a reproducible failure scenario; list what was verified correct), then tech-lead arbitration, then owner acceptance. Reviews are stored in `docs/reviews/<phase>_<round>.md` (append-only index `docs/reviews/INDEX.md`); the executor never writes to `docs/reviews/`. Report format: institution `10` §4 (conclusion first, evidence as `file:line` and command output excerpts, long logs saved to files).

Phase gates are **sequential**: Phase N+1 does not start until Phase N's review is arbitrated and the owner says go.

## Phase 0 — Repository bootstrap (macOS, ~half a day)

- **Goal**: a building, testing, CI-green skeleton with no product logic.
- **References**: this brief §8.1–8.2; `docs/REQUIREMENTS.md`.
- **Allowed files**: `Cargo.toml`, `crates/embers-core/**`, `crates/embers-cli/**` (hello-world only), `apps/macos/build.sh`, `apps/macos/Sources/App.swift` (empty accessory app), `apps/macos/Resources/Info.plist.template`, `.github/workflows/ci.yml`, `README.md` (structure + honest install placeholders), `THIRD_PARTY_NOTICES.md`, `.gitignore`, `rust-toolchain.toml`.
- **Forbidden**: `docs/**` except adding `docs/reviews/INDEX.md` header; anything under `apps/windows/` beyond a `README.md` stub.
- **Acceptance (action → verification)**:
  - `cargo test --workspace` → 1 trivial test passes locally.
  - `bash apps/macos/build.sh` → `build/Embers.app` exists; `codesign -dv build/Embers.app` prints `Signature=adhoc`; `open build/Embers.app` launches an accessory app (no Dock icon) that quits cleanly via `pkill -f Embers.app` within 1 s (CodexIsland `scripts/verify.sh` pattern).
  - Push → CI matrix green on ubuntu/macos/windows for `cargo test`; macOS job runs `build.sh`.
- **Stop conditions**: `swiftc` cannot link SwiftUI on the runner; `rustup` not installed on the Mac (owner action needed); any need to add a dependency not listed in §10.
- **Non-goals**: any detection logic, any UI, any Windows UI.

## Phase 1 — Core engine + CLI (macOS-validated; Windows compiled in CI)

- **Goal**: `embers scan` lists embers with correct categories and human reasons on the owner's Mac; `embers extinguish` ends a *test* process gracefully; Windows adapter compiles and its pure logic is fixture-tested.
- **References**: §6 (all), §8.4, §10.
- **Module structure**: `embers-core/src/{snapshot/{mod,macos,windows,linux_stub}.rs, origin/{mod,claude,codex,pattern}.rs, rules/{mod,e1_idle_session,e2_orphan,e3_idle_server,e4_loop,e5_runaway,protect}.rs, action/{mod,macos,windows}.rs, config.rs, redact.rs, model.rs}`; `embers-cli/src/main.rs` (clap subcommands from §10). Fixtures in `crates/embers-core/tests/fixtures/*.json` (synthetic snapshots incl. anonymised shapes of the observed Claude session tree, an orphan `http.server`, a 30 GB app, a busy session, a PID-reuse case for Windows).
- **Interfaces**: as §10 (`classify(prev, cur, cfg, hosts) -> Vec<Ember>`; `trait Sampler { fn snapshot(&mut self) -> Result<Snapshot> }`; `trait HostState { fn session_status(&self, pid) -> Option<SessionStatus> }`; `fn extinguish(target, opts) -> Vec<Outcome>`).
- **Allowed files**: `crates/**`, `README.md` (CLI usage section), `.github/workflows/ci.yml` (add `cargo check --target x86_64-pc-windows-msvc` on the windows job).
- **Forbidden**: `apps/**`, `docs/**`.
- **Performance constraints**: `embers watch --interval 5 --self-stats` for 10 min on the owner's Mac → average CPU < 0.5 %, RSS < 15 MB (evidence: `--self-stats` output + `footprint <pid>`); no subprocess spawned per tick (`fs_usage`/`execsnoop` or simply `ps` showing no transient children).
- **Tests**: fixture tests for every category and every protected rule (positive and negative); redaction tests (`--api-key X`, `TOKEN=…`, `Authorization: Bearer …`); PID-reuse test (Windows rule) and stale-session-file test (pid dead / start-time mismatch → ignored); config merge test (defaults + user override); **mutation check**: executor must show at least one rule test failing when the rule is deliberately inverted (institution `20` lesson 2026-08-25).
- **Human QA on the Mac (owner, with executor's checklist)**: (1) start `python3 -m http.server 8765` in a terminal, close the terminal window → within two polls `embers scan` shows it as **E2** with reason mentioning "parent gone" and ":8765"; (2) `embers extinguish <pid>` → process ends gracefully, outcome says "ended gracefully (SIGTERM)"; (3) leave a Claude Code-tab session idle > threshold (temporarily `idle_minutes = 1`) → shows as **E1** with "host says idle" and the MCP helper count; **do not** extinguish it in Phase 1; (4) `embers scan` never lists `kernel_task`, `WindowServer`, the frontmost app, or the busy session as actionable; (5) a heavy app (if present) shows as **E5** informational.
- **Stop conditions**: any need to read a file under §6.4 "Forbidden"; `PROC_PIDLISTFDS` walk exceeds budget (report numbers, don't optimise speculatively); fixture tests pass only after editing fixtures to fit the code; the Windows target fails to compile for reasons outside the adapter.
- **Non-goals**: any UI; Windows real-machine validation; snooze/allow-list persistence beyond config file.

## Phase 2 — macOS notch overlay app

- **Goal**: Embers.app shows the island; embers appear within one poll; End / Keep / Always-ignore work; motion meets the bar; footprint within budget; launch-at-login toggle works.
- **References**: §7.1, §8.2–8.4, CodexIsland files listed in §4 (techniques only; attribute reused snippets).
- **Module structure**: `apps/macos/Sources/{App.swift, Window/{OverlayWindow,OverlayHostingView,OverlayWindowController}.swift, Model/{NotchGeometry,IslandState,EmberStore,CoreClient}.swift, Views/{IslandRoot,Silhouette,PeekHeadline,EmberList,EmberRow,EmberDetail,ActionBar,ProtectedRow,SettingsPanel}.swift, Theme/{Motion,Palette,Type}.swift, Support/{LaunchAtLogin,Redaction}.swift}`; `apps/macos/build.sh`; `apps/macos/Resources/{Embers.icns, Info.plist.template}`.
- **Allowed files**: `apps/macos/**`, `README.md` (macOS install section incl. honest Gatekeeper text), `THIRD_PARTY_NOTICES.md`, `.github/workflows/ci.yml` (artifact upload of the `.app` zip).
- **Forbidden**: `crates/**` except additive NDJSON fields agreed in the goal (`ember_updated` events, `--self-stats`), `docs/**`, `apps/windows/**`.
- **Performance constraints**: compact state 0 timers besides the core poll; occluded → 0 % CPU (verify with `top -pid` while a fullscreen app is frontmost); hover → expand animation with no dropped frames at 120 Hz on the built-in display (verify with Quartz Debug "Frame Meter" or `CADisplayLink`-based stall counter printed in a debug build: p99 main-thread gap < 16 ms); app RSS < 50 MB after 1 h.
- **Tests**: pure-logic harnesses via `swiftc` for `NotchGeometry` (notched / non-notched / scaled modes), `IslandState` transitions (compact↔peek↔expanded incl. hover-leave during open), NDJSON decoding of every event type, redaction passthrough. UI motion is human-verified.
- **Human QA (owner)**: (1) hover → peek within 100 ms, click → expanded with the spring, click outside → close; clicks outside the shape pass through to the app below; (2) with the Phase-1 `http.server` scenario, the island glows amber within 5 s and the row's reason/metrics read correctly; **End** → row animates out, server gone, toast with the relaunch command; (3) **Keep** hides the row for 1 h; **Always ignore** adds a rule visible in `embers config show` and the row never returns; (4) idle Claude session: End → confirm sheet → group ended; verify the desktop app shows the session as ended and `claude --resume <id>` restores it (assumption A2 — record result); (5) `kernel_task`/system items show locked with explanation; (6) Settings: launch-at-login toggle → `SMAppService` status flips (`System Settings → Login Items` shows Embers); Reduce Motion on → no breathing glow; (7) lock screen → island hidden, unlock → fades back; external display without notch → menu-bar pill fallback appears.
- **Stop conditions**: SwiftUI feature needing macOS > the deployment target; any requirement to use `--deep` private APIs or `macOSPrivateApi`; any need to touch `crates/**` beyond additive fields; motion budget unmet after two distinct approaches (escalate, don't keep tuning).
- **Non-goals**: DMG/cask publishing (Phase 4), Windows, stats/history pages, auto-update.

## Phase 3 — Windows adapter validation + tray app (owner's PC for acceptance)

- **Goal**: `embers.exe scan` classifies correctly on a real Windows 11 PC; Tauri tray app shows embers, raises toasts, and ends processes via WM_CLOSE → TerminateProcess with confirmation.
- **References**: §6.5–6.6, §7.2, §8.2–8.5.
- **Module structure**: `crates/embers-core/src/snapshot/windows.rs`, `action/windows.rs` (real implementations); `apps/windows/src-tauri/{Cargo.toml, tauri.conf.json, src/{main,tray,flyout,toast,bridge}.rs}`; `apps/windows/ui/{index.html, styles.css, main.ts, components/*}`; `apps/windows/README.md`.
- **Allowed files**: above + `README.md` (Windows section, honest SmartScreen text), `.github/workflows/ci.yml` (`cargo tauri build` artifacts).
- **Forbidden**: `apps/macos/**`, `docs/**`, rule logic in `rules/**` (platform differences go into adapters; if a rule must change, stop and report).
- **Performance constraints**: tray app < 80 MB private bytes idle (Task Manager "Memory (private working set)"), < 1 % CPU average; no `tasklist`/`wmic`/PowerShell spawns per tick.
- **Tests**: Windows-only unit tests for PPID validation via creation time (real child process spawned by the test, parent exits, child re-queried), TCP listener table parsing, WM_CLOSE path against a spawned `notepad.exe` (ends gracefully), TerminateProcess fallback against a spawned `ping -t`-style console child; run on `windows-latest`.
- **Human QA (owner's PC, step list the executor must ship in `apps/windows/QA.md`)**: (1) install `rustup`, VS Build Tools, Node; `cargo tauri dev` runs; (2) `python -m http.server 8765` from a PowerShell window, close the window → `embers scan` shows E2 with "parent gone" (PID-reuse logic verified by also launching a few new processes first); (3) Claude Code native (`claude.exe`) idle session → E1 appears (record whether `%USERPROFILE%\.claude\sessions` exists — A1 result); (4) tray icon lights amber and a toast appears within 10 s; "End" from the toast and from the flyout both work; Notepad with unsaved text → WM_CLOSE shows Notepad's own save prompt (graceful path proven), Embers reports "still running after 5 s" and offers force; (5) `explorer.exe`, `dwm.exe`, `csrss.exe` never actionable; (6) autostart toggle writes/removes the HKCU `Run` value (`reg query`); (7) memory/CPU of Embers within budget after 30 min.
- **Stop conditions**: any API requiring elevation; `GetExtendedTcpTable` not reachable from chosen crates after one alternative tried; Claude/Codex process trees on Windows look nothing like the Mac shapes (report, do not invent patterns).
- **Non-goals**: winget/scoop publishing (Phase 4), top-pill mode, signing.

## Phase 4 — Distribution, autostart defaults, polish

- **Goal**: tagged release produces `.dmg` + Homebrew cask (in a separate tap repo), Windows `.zip` + NSIS `.exe` + scoop manifest; README privacy and unsigned-build sections final; Keep-snooze durations, allow-list editor, optional local action log (off by default); top-pill mode on Windows if the owner wants it.
- **Allowed files**: `.github/workflows/release.yml`, `scripts/{make-dmg.sh, make-icns.sh}`, `packaging/{homebrew/embers.rb.template, scoop/embers.json}`, `README.md`, settings views in both apps.
- **Acceptance**: `git tag v0.1.0` → release assets appear with SHA-256; `brew install --cask <tap>/embers` installs and launches after the documented Gatekeeper step **without** any quarantine stripping in the cask; `scoop install embers` from the bucket works on the owner's PC; `lsof -p`/Resource Monitor shows **no network sockets** opened by Embers.
- **Stop conditions**: any step that would require a signing secret the owner has not provided; any temptation to add an updater.
- **Non-goals**: Linux UI, multi-user, remote machines.

---

*End of brief. Next action: owner answers §12, tech lead records the Architecture Decision in `docs/ARCHITECTURE_DECISION.md`, Phase 0 Executor Goal is issued.*
