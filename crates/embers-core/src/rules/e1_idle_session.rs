//! E1 — an AI session that has been idle for a while (with its helpers).

use crate::model::{Action, Category, Confidence, Origin, SessionStatus};
use crate::reason;
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
    let (idle_ms, confidence, signal) = match (
        state.status,
        state.status_since_ms,
        state.last_transcript_write_ms,
    ) {
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

    // The whole process group goes together when every member may be ended; otherwise
    // fall back to the session's own (unprotected) tree.
    let group: Vec<u32> = match p.pgid {
        Some(g) => ctx
            .cur
            .procs
            .iter()
            .filter(|q| q.pgid == Some(g))
            .map(|q| q.pid)
            .collect(),
        None => Vec::new(),
    };
    let group_ok = !group.is_empty() && group.iter().all(|q| ctx.protection(*q).is_none());
    let (mut pids, action) = if group_ok {
        (group, Action::TerminateGroup)
    } else {
        (ctx.tree(pid), Action::Terminate)
    };
    for q in ctx.tree(pid) {
        if !pids.contains(&q) {
            pids.push(q);
        }
    }

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
        (Some(id), Some(cwd)) if is_claude => {
            Some(format!("Resume: claude --resume {id} (in {cwd})"))
        }
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
