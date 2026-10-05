# Dependencies

Rule S7 (docs/ARCHITECTURE_DECISION.md): every **direct** dependency is listed here with a reason.
Adding one needs a one-line justification in the PR and tech-lead approval.
Network stacks (`reqwest`, `hyper`, `openssl`, `native-tls`) are banned in `deny.toml`.

Check: `cargo tree -e no-dev --depth 1` must match this table.

| Crate | Used by | Why |
|---|---|---|
| `clap` (feature `derive`) | `embers-cli` | Command-line argument parsing for the `embers` CLI. |
| `embers-core` (path) | `embers-cli` | Our own engine crate. |

`embers-core` has no external dependencies yet.

Planned for Phase 1 (pre-approved in the allow-list, added only when used):
`sysinfo`, `windows` (Windows only), `libc`, `serde`/`serde_json`, `toml`, `directories`,
`regex`, `thiserror`/`anyhow`, `tracing`.
