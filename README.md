# Embers

**Notices what's still smoldering after your AI coding sessions end — and asks before putting it out.**

Claude Code, Codex and friends spin up dev servers, headless browsers, MCP helpers and polling loops.
When the session ends, some of them keep running, quietly eating CPU and memory.
Embers lives in your MacBook's notch, spots those leftovers (plus any app that's running away with your RAM),
explains why each one looks abandoned, and lets you **End**, **Keep**, or **Always ignore** it — nothing is ever killed without asking.

> **Status:** early development. Not ready to install yet.

## Principles

- **Asks first.** Embers never terminates anything on its own.
- **Never touches the system.** kernel_task, WindowServer and other macOS processes are off-limits.
- **Local only.** No telemetry, no accounts, no network calls. It never reads credentials.
- **Light.** A tool that hunts resource hogs must not become one.

## Requirements

- macOS on Apple Silicon (notch MacBooks get the notch UI; others get a menu-bar pill)

## License

[MIT](LICENSE)
