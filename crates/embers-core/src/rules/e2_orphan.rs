//! E2 — a developer tool whose parent is gone.

use crate::model::{Action, Category, Confidence, Proc};
use crate::rules::{Context, Hit};

pub fn hit(ctx: &Context, p: &Proc) -> Option<Hit> {
    if !ctx.parent_gone(p) {
        return None;
    }
    let cmd = p.command_line();
    let dev_tool = ctx.cfg.dev_tool_patterns.iter().any(|re| re.is_match(&cmd));
    if !dev_tool && p.listeners.is_empty() {
        return None;
    }
    let mut facts = vec!["parent gone".to_string()];
    facts.extend(super::listening(p));
    facts.push(format!(
        "running {}",
        crate::reason::duration(ctx.age_ms(p))
    ));
    Some(Hit {
        root: p.pid,
        pids: ctx.tree(p.pid),
        category: Category::Orphan,
        confidence: Confidence::High,
        action: Action::Terminate,
        idle_secs: None,
        hint: Some(cmd),
        facts,
    })
}
