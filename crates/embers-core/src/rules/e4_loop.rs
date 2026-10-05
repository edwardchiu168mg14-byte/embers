//! E4 — a shell polling loop (`until …; do …; sleep N; done`) left behind.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use crate::model::{Action, Category, Confidence, Pid, Proc};
use crate::reason;
use crate::rules::{Context, Hit};

fn loop_script() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)\b(while|until|for)\b.*\bsleep\b").expect("valid regex"))
}

pub fn hit(ctx: &Context, p: &Proc, e1_roots: &HashSet<Pid>) -> Option<Hit> {
    let is_shell = ctx
        .cfg
        .protected
        .shells
        .iter()
        .any(|s| s.eq_ignore_ascii_case(p.name()));
    let has_script_flag = p
        .argv
        .iter()
        .any(|a| a == "-c" || a.eq_ignore_ascii_case("/c"));
    if !is_shell || !has_script_flag || !loop_script().is_match(&p.command_line()) {
        return None;
    }
    // Never judge on a single snapshot: the loop must have been seen before.
    if !ctx.seen_in_prev(p.pid) || ctx.age_ms(p) < ctx.idle_ms {
        return None;
    }
    let why = ctx.session_idle_or_gone(p.pid, e1_roots)?;
    Some(Hit {
        root: p.pid,
        pids: ctx.tree(p.pid),
        category: Category::Loop,
        confidence: Confidence::Medium,
        action: Action::Terminate,
        idle_secs: None,
        hint: Some(super::command_hint(p)),
        facts: vec![
            "polling loop".to_string(),
            format!("running {}", reason::duration(ctx.age_ms(p))),
            why.to_string(),
        ],
    })
}
