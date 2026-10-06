//! E1 — an AI session that has been idle for a while (with its helpers).

use crate::model::{Action, Category, Confidence, Origin, SessionStatus};
use crate::reason;
use crate::redact::{cap_field, redact_text, MAX_FIELD_BYTES};
use crate::rules::{Context, Hit};

pub fn hits(ctx: &Context) -> Vec<Hit> {
    let mut roots: Vec<_> = ctx.session_roots.iter().copied().collect();
    roots.sort_unstable();
    roots.into_iter().filter_map(|pid| hit(ctx, pid)).collect()
}

fn hit(ctx: &Context, pid: u32) -> Option<Hit> {
    if ctx.protection(pid).is_some() {
        return None;
    }
    let p = ctx.procs.get(&pid)?;
    let state = ctx.sessions.get(&pid)?;
    // A status older than the process belongs to an earlier process with the same pid.
    let (status, since) = match state.status_since_ms {
        Some(since) if since < p.start_time_ms => (None, None),
        since => (state.status, since),
    };
    let (idle_ms, confidence, signal) = match (status, since, state.last_transcript_write_ms) {
        (Some(SessionStatus::Idle), Some(since), _) => (
            ctx.now_ms.saturating_sub(since),
            Confidence::High,
            "host says idle".to_string(),
        ),
        (None, _, Some(written)) => {
            let ms = ctx.now_ms.saturating_sub(written);
            (
                ms,
                Confidence::Medium,
                format!("no transcript activity for {}", reason::duration(ms)),
            )
        }
        _ => return None,
    };
    if idle_ms < ctx.idle_ms {
        return None;
    }

    // The whole process group goes together only when that is provably safe
    // (see `Context::session_group`); otherwise just the session's own subtree.
    let (pids, action) = match ctx.session_group(pid) {
        Some(group) => (group, Action::TerminateGroup),
        None => (ctx.tree(pid), Action::Terminate),
    };

    let cpu = ctx.cpu_pct_sum(&pids)?;
    if cpu >= 1.0 {
        return None;
    }
    let window_start = ctx.now_ms.saturating_sub(ctx.window_ms);
    let fresh_child = pids
        .iter()
        .filter(|q| **q != pid)
        .filter_map(|q| ctx.procs.get(q))
        .any(|q| q.start_time_ms >= window_start);
    if fresh_child {
        return None;
    }

    let is_claude = matches!(
        ctx.origins.get(&pid),
        Some(Origin::ClaudeDesktop | Origin::ClaudeCli)
    );
    let hint = match (&state.session_id, &state.cwd) {
        (Some(id), Some(cwd)) if is_claude => Some(format!(
            "Resume: claude --resume {} (in {})",
            shell_quote(&safe(id)),
            shell_quote(&safe(cwd))
        )),
        _ => None,
    };
    let helpers = pids.len().saturating_sub(1);
    Some(Hit {
        root: pid,
        category: Category::IdleSession,
        confidence,
        action,
        idle_secs: Some(idle_ms / 1000),
        hint,
        facts: vec![
            format!("idle {}", reason::duration(idle_ms)),
            signal,
            reason::cpu(cpu),
            match helpers {
                0 => String::new(),
                1 => "1 helper".to_string(),
                n => format!("{n} helpers"),
            },
        ],
        pids,
    })
}

/// Host-supplied text is untrusted (rule S10): redact, then cap.
fn safe(text: &str) -> String {
    cap_field(&redact_text(text), MAX_FIELD_BYTES)
}

/// Single-quotes `text` for a POSIX shell when it contains anything but safe characters.
fn shell_quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:@%+=,".contains(c));
    if plain {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}
