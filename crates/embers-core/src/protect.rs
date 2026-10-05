//! Processes Embers must never offer to end (rule S11). Checked before any rule runs.

use crate::config::Config;
use crate::model::{Proc, ProtectReason, SessionStatus};
use crate::rules::Context;

/// Why `p` is off-limits, or `None` if rules may consider it.
pub fn is_protected(p: &Proc, cfg: &Config, ctx: &Context) -> Option<ProtectReason> {
    let snap = ctx.cur;
    let name = p.name();
    if p.pid == 0 {
        return Some(ProtectReason::Kernel);
    }
    if p.pid == 1
        || cfg
            .protected
            .names(snap.os)
            .iter()
            .any(|n| n.eq_ignore_ascii_case(name))
    {
        return Some(ProtectReason::SystemDaemon);
    }
    let exe_lower = p.exe.to_ascii_lowercase();
    if cfg
        .protected
        .path_prefixes(snap.os)
        .iter()
        .any(|prefix| exe_lower.starts_with(&prefix.to_ascii_lowercase()))
    {
        return Some(ProtectReason::SystemDaemon);
    }
    if p.uid != snap.current_uid {
        return Some(ProtectReason::NotOwned);
    }
    if snap.self_pids.contains(&p.pid) {
        return Some(ProtectReason::SelfProcess);
    }
    if snap.frontmost_pid == Some(p.pid) {
        return Some(ProtectReason::Frontmost);
    }
    if let Some(s) = ctx.sessions.get(&p.pid) {
        let recently_idle = s.status == Some(SessionStatus::Idle)
            && s.status_since_ms
                .is_none_or(|since| ctx.now_ms.saturating_sub(since) < ctx.idle_ms);
        if s.status == Some(SessionStatus::Busy) || recently_idle {
            return Some(ProtectReason::BusySession);
        }
    }
    if p.has_tty
        && cfg
            .protected
            .shells
            .iter()
            .any(|s| s.eq_ignore_ascii_case(name))
    {
        return Some(ProtectReason::LiveTty);
    }
    let cmd = p.command_line();
    cfg.allowlist
        .iter()
        .find(|rule| rule.pattern.is_match(&cmd) || rule.pattern.is_match(&p.exe))
        .map(|rule| ProtectReason::Allowlisted(rule.label.clone()))
}
