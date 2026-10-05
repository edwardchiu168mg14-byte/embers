# Dependencies

Rule S7 (docs/ARCHITECTURE_DECISION.md): every **direct** dependency is listed here with a reason.
Adding one needs a one-line justification in the PR and tech-lead approval.
Network stacks (`reqwest`, `hyper`, `openssl`, `native-tls`) are banned in `deny.toml`.

Check: `cargo tree -e no-dev --depth 1` must match this table.

| Crate | Used by | Why |
|---|---|---|
| `clap` (feature `derive`) | `embers-cli` | Command-line argument parsing for the `embers` CLI. |
| `embers-core` (path) | `embers-cli` | Our own engine crate. |
| `serde_json` | `embers-cli`, `embers-core` | NDJSON output (`--json`) and loading fixture snapshots. |
| `serde` (feature `derive`) | `embers-core` | (De)serialising snapshots, embers and config. |
| `toml` (features `std`, `parse`, `serde` only) | `embers-core` | Reading the defaults and the user's `config.toml`. |
| `regex` | `embers-core` | Origin, dev-tool, allow-list and secret-redaction patterns. |
| `thiserror` | `embers-core` | Typed config / fixture errors without a runtime dependency. |

Still pre-approved for later phases (added only when used):
`sysinfo`, `windows` (Windows only), `libc`, `directories`, `anyhow`, `tracing`.

## Manual pins

Versions Dependabot cannot see. Review and bump them at every release (rule S12).

| Tool | Version | Where |
|---|---|---|
| `cargo-audit` | 0.22.2 | `.github/workflows/audit.yml` |
| `gitleaks` | 8.24.3 | `.github/workflows/ci.yml` (`GITLEAKS_VERSION`) |
