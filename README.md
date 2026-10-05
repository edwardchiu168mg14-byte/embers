# Embers

**Notices what's still smoldering after your AI coding sessions end — and asks before putting it out.**

Claude Code, Codex and friends spin up dev servers, headless browsers, MCP helpers and polling loops.
When the session ends, some of them keep running, quietly eating CPU and memory.
Embers lives in your MacBook's notch (or your Windows system tray), spots those leftovers (plus any app that's running away with your RAM),
explains why each one looks abandoned, and lets you **End**, **Keep**, or **Always ignore** it — nothing is ever killed without asking.

> **Status:** pre-alpha. Nothing to install yet — the project skeleton is in place and the detection engine is next.

## Principles

- **Asks first.** Embers never terminates anything on its own.
- **Never touches the system.** OS processes (kernel_task, WindowServer, csrss, explorer…) are off-limits.
- **Local only.** No telemetry, no accounts, no network calls. It never reads credentials.
- **Light.** A tool that hunts resource hogs must not become one.

## Privacy

Embers runs entirely on your computer. It opens no network connections, sends no telemetry,
and never reads passwords, keys or tokens. See [SECURITY.md](SECURITY.md).

## Installing (once releases exist)

Builds are not signed by Apple or Microsoft (no paid certificate), so your OS will ask once:

- **macOS:** the first launch is blocked. Open **System Settings → Privacy & Security** and click
  **Open Anyway** next to Embers. Embers' installer will never switch this check off for you.
- **Windows:** SmartScreen may say "Windows protected your PC" — click **More info → Run anyway**.

Every release is built by GitHub Actions from a tagged commit and ships with `SHA256SUMS.txt`
so you can verify the download.

## Repository layout

| Path | What |
|---|---|
| `crates/embers-core` | Rust engine: scan, classify, explain, end (shared by both apps) |
| `crates/embers-cli` | `embers` command-line tool |
| `apps/macos` | Native notch app (Swift); `bash apps/macos/build.sh` builds `build/Embers.app` |
| `apps/windows` | Tray app (Tauri) — coming later |
| `docs/` | Requirements, design brief, architecture decision |

## Building from source

Requires Rust (via [rustup](https://rustup.rs)) and, for the macOS app, Xcode Command Line Tools.

```bash
cargo test --workspace
cargo run -p embers-cli -- --help
bash apps/macos/build.sh
```

## Requirements

- **macOS** — notch MacBooks get the notch UI; other Macs get a menu-bar pill
- **Windows** — lives in the system tray

## License

[MIT](LICENSE)
