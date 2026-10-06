//! Works out which tool started each process by walking up its ancestors.

use std::collections::HashMap;

use crate::config::{Config, OriginPattern};
use crate::model::{Origin, Pid, Proc, Snapshot};

/// Ancestor walks stop after this many steps, so corrupt parent links cannot loop forever.
pub const MAX_DEPTH: usize = 64;

/// The first configured pattern that matches the process itself, if any.
pub fn own_match<'c>(p: &Proc, cfg: &'c Config) -> Option<&'c OriginPattern> {
    let cmd = p.command_line();
    cfg.origins.iter().find(|o| {
        o.exe.as_ref().is_none_or(|re| re.is_match(&p.exe))
            && o.argv.as_ref().is_none_or(|re| re.is_match(&cmd))
    })
}

/// Origin of every process: the nearest ancestor (or itself) that matches a pattern.
pub fn attribute(snap: &Snapshot, cfg: &Config) -> HashMap<Pid, Origin> {
    let by_pid: HashMap<Pid, &Proc> = snap.procs.iter().map(|p| (p.pid, p)).collect();
    let own: HashMap<Pid, Origin> = snap
        .procs
        .iter()
        .filter_map(|p| own_match(p, cfg).map(|o| (p.pid, o.origin)))
        .collect();

    snap.procs
        .iter()
        .map(|p| {
            let mut cur = Some(p);
            let mut origin = Origin::Unknown;
            for _ in 0..MAX_DEPTH {
                let Some(proc_) = cur else { break };
                if let Some(o) = own.get(&proc_.pid) {
                    origin = *o;
                    break;
                }
                cur = proc_
                    .ppid
                    .filter(|pp| *pp != proc_.pid)
                    .and_then(|pp| by_pid.get(&pp).copied());
            }
            (p.pid, origin)
        })
        .collect()
}
