# Security policy

Embers can end processes on your computer, so we treat security bugs seriously.

## Reporting a vulnerability

Please **do not open a public issue.** Report privately through GitHub:
**Security** tab → **Report a vulnerability** (GitHub Private Vulnerability Reporting).

You'll get an acknowledgement within **7 days**. Please include steps to reproduce and the
Embers version (`embers --version`).

## Supported versions

Embers is pre-alpha. Only the latest commit on `main` (and, once releases exist, the latest
release) receives security fixes.

## What Embers promises

- It never ends anything without asking you first.
- It never requests administrator/root rights and only acts on processes owned by you.
- It opens no network connections and sends no telemetry.
- It never reads credentials, keychains or token files.
- Release binaries are built only by GitHub Actions from a tagged commit and published with SHA-256 checksums.

If you find Embers breaking any of these, that is a security bug — please report it.
