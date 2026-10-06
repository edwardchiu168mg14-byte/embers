//! E5 — any process using an extreme amount of memory or CPU.

use crate::model::{Action, Category, Confidence, Proc};
use crate::reason;
use crate::rules::{Context, Hit};

pub fn hit(ctx: &Context, p: &Proc) -> Option<Hit> {
    let cfg = ctx.cfg;
    let memory_hog = p.footprint_bytes > cfg.hog_memory_bytes;
    // The CPU clause needs an observation window at least `hog_minutes` long.
    let long_enough = ctx
        .interval_ms()
        .is_some_and(|ms| ms >= u64::from(cfg.hog_minutes) * 60_000);
    let cpu = ctx.cpu_pct(p.pid).filter(|_| long_enough);
    let cpu_hog = cpu.is_some_and(|c| c > cfg.hog_cpu_pct);
    if !memory_hog && !cpu_hog {
        return None;
    }
    let action = if ctx.protection(p.pid).is_some() || p.is_system_path {
        Action::Inform
    } else if p.is_app_bundle {
        Action::QuitApp
    } else {
        Action::Terminate
    };
    let mut facts = Vec::new();
    if memory_hog {
        facts.push(format!("{} memory", reason::bytes(p.footprint_bytes)));
    }
    if let Some(c) = cpu.filter(|_| cpu_hog) {
        facts.push(reason::cpu(c));
    }
    facts.push(format!("running {}", reason::duration(ctx.age_ms(p))));
    Some(Hit {
        root: p.pid,
        pids: vec![p.pid],
        category: Category::Runaway,
        confidence: Confidence::High,
        action,
        idle_secs: None,
        hint: None,
        facts,
    })
}
