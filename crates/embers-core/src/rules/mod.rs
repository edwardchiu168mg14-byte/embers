//! The rule engine: turns snapshots into embers. Pure — no OS calls, no I/O.

mod e1_idle_session;
mod e2_orphan;
mod e3_idle_server;
mod e4_loop;
mod e5_runaway;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::config::Config;
use crate::model::{
    Action, Category, Confidence, Ember, HostStates, Metrics, Origin, Os, Pid, Proc, ProtectReason,
    SessionState, Snapshot,
};
use crate::origin::{self, MAX_DEPTH};
use crate::protect::is_protected;
use crate::reason;

/// Everything a rule may look at, precomputed once per `classify` call.
pub struct Context<'a> {
    pub cur: &'a Snapshot,
    pub cfg: &'a Config,
    pub now_ms: u64,
    pub idle_ms: u64,
    pub window_ms: u64,
    pub procs: HashMap<Pid, &'a Proc>,
    pub children: HashMap<Pid, Vec<Pid>>,
    pub sessions: HashMap<Pid, &'a SessionState>,
    pub origins: HashMap<Pid, Origin>,
    /// Processes that are themselves an AI session (an origin pattern with `session_root`).
    pub session_roots: HashSet<Pid>,
    /// Same process (pid *and* start time) in the previous snapshot.
    prev: HashMap<Pid, &'a Proc>,
    interval_ms: Option<u64>,
    protected: HashMap<Pid, ProtectReason>,
}

/// Where a process's ancestor chain leads.
pub enum Ancestry {
    /// Reaches a live AI session process.
    Session(Pid),
    /// A parent along the way is gone.
    Broken,
    /// Reaches the OS root, a terminal, or an unknown parent, with no AI session on the way.
    Intact,
}

/// One rule's finding about one root process, before findings are merged.
pub struct Hit {
    pub root: Pid,
    pub pids: Vec<Pid>,
    pub category: Category,
    pub confidence: Confidence,
    pub action: Action,
    pub idle_secs: Option<u64>,
    pub hint: Option<String>,
    pub facts: Vec<String>,
}

impl<'a> Context<'a> {
    pub fn new(
        prev: Option<&'a Snapshot>,
        cur: &'a Snapshot,
        cfg: &'a Config,
        hosts: &'a HostStates,
    ) -> Self {
        let procs: HashMap<Pid, &Proc> = cur.procs.iter().map(|p| (p.pid, p)).collect();
        let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
        for p in &cur.procs {
            if let Some(pp) = p.ppid.filter(|pp| *pp != p.pid) {
                children.entry(pp).or_default().push(p.pid);
            }
        }
        let prev_map = prev
            .map(|s| {
                s.procs
                    .iter()
                    .filter(|old| {
                        procs
                            .get(&old.pid)
                            .is_some_and(|new| new.start_time_ms == old.start_time_ms)
                    })
                    .map(|old| (old.pid, old))
                    .collect()
            })
            .unwrap_or_default();
        let mut ctx = Context {
            cur,
            cfg,
            now_ms: cur.taken_at_ms,
            idle_ms: u64::from(cfg.idle_minutes) * 60_000,
            window_ms: u64::from(cfg.window_minutes) * 60_000,
            children,
            sessions: hosts.sessions.iter().map(|s| (s.pid, s)).collect(),
            origins: origin::attribute(cur, cfg),
            session_roots: cur
                .procs
                .iter()
                .filter(|p| origin::own_match(p, cfg).is_some_and(|o| o.session_root))
                .map(|p| p.pid)
                .collect(),
            prev: prev_map,
            interval_ms: prev
                .and_then(|p| cur.taken_at_ms.checked_sub(p.taken_at_ms))
                .filter(|ms| *ms > 0),
            procs,
            protected: HashMap::new(),
        };
        ctx.protected = cur
            .procs
            .iter()
            .filter_map(|p| is_protected(p, cfg, &ctx).map(|r| (p.pid, r)))
            .collect();
        ctx
    }

    pub fn protection(&self, pid: Pid) -> Option<&ProtectReason> {
        self.protected.get(&pid)
    }

    pub fn age_ms(&self, p: &Proc) -> u64 {
        self.now_ms.saturating_sub(p.start_time_ms)
    }

    pub fn interval_ms(&self) -> Option<u64> {
        self.interval_ms
    }

    pub fn seen_in_prev(&self, pid: Pid) -> bool {
        self.prev.contains_key(&pid)
    }

    /// CPU % over the interval between snapshots; `None` when it cannot be known.
    pub fn cpu_pct(&self, pid: Pid) -> Option<f32> {
        let (old, new, interval) = (
            self.prev.get(&pid)?,
            self.procs.get(&pid)?,
            self.interval_ms?,
        );
        let used = new.cpu_time_ms.checked_sub(old.cpu_time_ms)?;
        Some(used as f32 / interval as f32 * 100.0)
    }

    /// Summed CPU of `pids`; `None` if any of them cannot be measured.
    pub fn cpu_pct_sum(&self, pids: &[Pid]) -> Option<f32> {
        pids.iter().map(|p| self.cpu_pct(*p)).sum()
    }

    /// All descendants of `pid` (not including `pid`), breadth-first.
    pub fn descendants(&self, pid: Pid) -> Vec<Pid> {
        let mut out = Vec::new();
        let mut seen = HashSet::from([pid]);
        let mut queue = VecDeque::from([pid]);
        while let Some(next) = queue.pop_front() {
            for child in self.children.get(&next).into_iter().flatten() {
                if seen.insert(*child) {
                    out.push(*child);
                    queue.push_back(*child);
                }
            }
        }
        out
    }

    /// `pid` plus its descendants, minus anything protected.
    pub fn tree(&self, pid: Pid) -> Vec<Pid> {
        std::iter::once(pid)
            .chain(self.descendants(pid))
            .filter(|p| *p == pid || !self.protected.contains_key(p))
            .collect()
    }

    /// The parent is gone: no valid parent, or (macOS) adopted by launchd while not
    /// being an app or a system binary.
    pub fn parent_gone(&self, p: &Proc) -> bool {
        match p.ppid {
            None => true,
            Some(1) => self.cur.os == Os::Macos && !p.is_app_bundle && !p.is_system_path,
            Some(_) => false,
        }
    }

    pub fn ancestry(&self, pid: Pid) -> Ancestry {
        let Some(mut cur) = self.procs.get(&pid).copied() else {
            return Ancestry::Intact;
        };
        for _ in 0..MAX_DEPTH {
            if cur.pid != pid && self.session_roots.contains(&cur.pid) {
                return Ancestry::Session(cur.pid);
            }
            if cur.pid <= 1 {
                return Ancestry::Intact;
            }
            if self.parent_gone(cur) {
                return Ancestry::Broken;
            }
            match cur.ppid.and_then(|pp| self.procs.get(&pp)) {
                Some(parent) if parent.pid != cur.pid => cur = parent,
                _ => return Ancestry::Intact,
            }
        }
        Ancestry::Intact
    }

    /// The process's AI session is idle (an E1 root) or gone.
    pub fn session_idle_or_gone(&self, pid: Pid, e1_roots: &HashSet<Pid>) -> Option<&'static str> {
        match self.ancestry(pid) {
            Ancestry::Session(root) if e1_roots.contains(&root) => Some("its session is idle"),
            Ancestry::Broken => Some("its session ended"),
            _ => None,
        }
    }
}

/// Classifies `cur` (optionally against `prev`, for CPU deltas) into embers.
///
/// Protected processes are decided first and can only ever appear as [`Action::Inform`];
/// allow-listed processes never appear at all.
pub fn classify(
    prev: Option<&Snapshot>,
    cur: &Snapshot,
    cfg: &Config,
    hosts: &HostStates,
) -> Vec<Ember> {
    let ctx = Context::new(prev, cur, cfg, hosts);

    let e1 = e1_idle_session::hits(&ctx);
    let e1_roots: HashSet<Pid> = e1.iter().map(|h| h.root).collect();
    let mut hits = e1;
    for p in &cur.procs {
        if ctx.protection(p.pid).is_none() {
            hits.extend(e2_orphan::hit(&ctx, p));
            hits.extend(e3_idle_server::hit(&ctx, p, &e1_roots));
            hits.extend(e4_loop::hit(&ctx, p, &e1_roots));
        }
        hits.extend(e5_runaway::hit(&ctx, p));
    }
    merge(&ctx, hits)
}

fn merge(ctx: &Context, hits: Vec<Hit>) -> Vec<Ember> {
    let mut by_root: BTreeMap<Pid, Vec<Hit>> = BTreeMap::new();
    for hit in hits {
        by_root.entry(hit.root).or_default().push(hit);
    }

    let mut embers: Vec<Ember> = by_root
        .into_iter()
        .filter_map(|(root, mut group)| {
            let p = ctx.procs.get(&root)?;
            let protected = ctx.protection(root).cloned();
            if matches!(protected, Some(ProtectReason::Allowlisted(_))) {
                return None;
            }
            group.sort_by_key(|h| h.category);
            let first = &group[0];
            let mut pids: Vec<Pid> = group.iter().flat_map(|h| h.pids.iter().copied()).collect();
            pids.sort_unstable();
            pids.dedup();
            let facts: Vec<String> = group.iter().flat_map(|h| h.facts.iter().cloned()).collect();
            let mut listeners: Vec<u16> = pids
                .iter()
                .filter_map(|pid| ctx.procs.get(pid))
                .flat_map(|p| p.listeners.iter().copied())
                .collect();
            listeners.sort_unstable();
            listeners.dedup();
            Some(Ember {
                id: format!("{root}-{}", p.start_time_ms),
                name: p.name().to_string(),
                root_pid: root,
                categories: group.iter().map(|h| h.category).collect(),
                origin: ctx.origins.get(&root).copied().unwrap_or(Origin::Unknown),
                confidence: if group.iter().any(|h| h.confidence == Confidence::High) {
                    Confidence::High
                } else {
                    Confidence::Medium
                },
                reason: reason::sentence(&facts),
                metrics: Metrics {
                    idle_secs: group.iter().find_map(|h| h.idle_secs),
                    cpu_pct: ctx.cpu_pct_sum(&pids),
                    footprint_bytes: pids
                        .iter()
                        .filter_map(|pid| ctx.procs.get(pid))
                        .map(|p| p.footprint_bytes)
                        .sum(),
                    age_secs: ctx.age_ms(p) / 1000,
                    helper_count: u32::try_from(pids.len().saturating_sub(1)).unwrap_or(u32::MAX),
                    listeners,
                },
                action: if protected.is_some() {
                    Action::Inform
                } else {
                    first.action
                },
                recovery_hint: group.iter().find_map(|h| h.hint.clone()),
                pids,
                protected,
            })
        })
        .collect();

    // A process already inside an actionable group is not offered a second time on its own.
    let member_of: HashMap<Pid, Pid> = embers
        .iter()
        .filter(|e| e.protected.is_none())
        .flat_map(|e| {
            e.pids
                .iter()
                .filter(move |p| **p != e.root_pid)
                .map(move |p| (*p, e.root_pid))
        })
        .collect();
    embers.retain(|e| !member_of.contains_key(&e.root_pid));

    embers.sort_by_key(|e| (e.categories[0], e.root_pid));
    embers
}

/// `"listening :8765"` style facts for a process's open server ports.
fn listening(p: &Proc) -> Vec<String> {
    if p.listeners.is_empty() {
        return Vec::new();
    }
    let ports: Vec<String> = p.listeners.iter().map(|l| format!(":{l}")).collect();
    vec![format!("listening {}", ports.join(" "))]
}
