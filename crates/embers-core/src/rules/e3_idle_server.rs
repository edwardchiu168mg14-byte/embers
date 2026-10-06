//! E3 — a server nobody is talking to, left behind by an idle or ended AI session.

use std::collections::HashSet;

use crate::model::{Action, Category, Confidence, Pid, Proc};
use crate::reason;
use crate::rules::{Context, Hit};

pub fn hit(ctx: &Context, p: &Proc, e1_roots: &HashSet<Pid>) -> Option<Hit> {
    if p.listeners.is_empty() || p.established > 0 || ctx.age_ms(p) < ctx.idle_ms {
        return None;
    }
    let cpu = ctx.cpu_pct(p.pid)?;
    if cpu >= 1.0 {
        return None;
    }
    let why = ctx.session_idle_or_gone(p.pid, e1_roots)?;
    let mut facts = super::listening(p);
    facts.extend([
        "no connections".to_string(),
        reason::cpu(cpu),
        format!("up {}", reason::duration(ctx.age_ms(p))),
        why.to_string(),
    ]);
    Some(Hit {
        root: p.pid,
        pids: ctx.tree(p.pid),
        category: Category::IdleServer,
        confidence: Confidence::Medium,
        action: Action::Terminate,
        idle_secs: None,
        hint: Some(super::command_hint(p)),
        facts,
    })
}
