//! Data types shared by samplers, rules and front-ends.

use serde::{Deserialize, Serialize};

pub type Pid = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Os {
    Macos,
    Windows,
    Linux,
}

/// One point-in-time view of every process the sampler could see.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub taken_at_ms: u64,
    pub os: Os,
    pub procs: Vec<Proc>,
    #[serde(default)]
    pub frontmost_pid: Option<Pid>,
    #[serde(default)]
    pub self_pids: Vec<Pid>,
    pub current_uid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proc {
    pub pid: Pid,
    /// `None` means the parent is gone or invalid (the sampler already checked start times).
    pub ppid: Option<Pid>,
    #[serde(default)]
    pub pgid: Option<Pid>,
    pub start_time_ms: u64,
    pub uid: u32,
    pub exe: String,
    /// Already redacted by the sampler; rules never un-redact.
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub cpu_time_ms: u64,
    #[serde(default)]
    pub footprint_bytes: u64,
    #[serde(default)]
    pub listeners: Vec<u16>,
    #[serde(default)]
    pub established: u32,
    #[serde(default)]
    pub is_app_bundle: bool,
    #[serde(default)]
    pub has_tty: bool,
    #[serde(default)]
    pub is_system_path: bool,
}

impl Proc {
    /// Executable file name without its directory (`/usr/bin/python3` → `python3`).
    pub fn name(&self) -> &str {
        self.exe.rsplit(['/', '\\']).next().unwrap_or(&self.exe)
    }

    /// Executable plus arguments, space-joined, for pattern matching and hints.
    pub fn command_line(&self) -> String {
        if self.argv.is_empty() {
            self.exe.clone()
        } else {
            self.argv.join(" ")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Origin {
    ClaudeDesktop,
    ClaudeCli,
    CodexApp,
    CodexCli,
    Terminal,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Category {
    IdleSession,
    Orphan,
    IdleServer,
    Loop,
    Runaway,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    High,
    Medium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    TerminateGroup,
    Terminate,
    QuitApp,
    Inform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtectReason {
    Kernel,
    SystemDaemon,
    NotOwned,
    SelfProcess,
    Frontmost,
    BusySession,
    LiveTty,
    Allowlisted(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub idle_secs: Option<u64>,
    /// CPU over the observed interval; `None` when only one snapshot was available.
    pub cpu_pct: Option<f32>,
    pub footprint_bytes: u64,
    pub age_secs: u64,
    pub helper_count: u32,
    pub listeners: Vec<u16>,
}

/// A process (or process group) Embers proposes to end, or reports for information.
///
/// Safety contract: when `protected` is `Some`, `action` is always [`Action::Inform`] and
/// `pids` must never be passed to any terminate call. The action layer re-checks this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ember {
    /// `"<root_pid>-<start_time_ms>"`, stable across polls and safe against PID reuse.
    pub id: String,
    /// Executable name of the root process, for display.
    pub name: String,
    pub root_pid: Pid,
    pub pids: Vec<Pid>,
    pub categories: Vec<Category>,
    pub origin: Origin,
    pub confidence: Confidence,
    pub reason: String,
    pub metrics: Metrics,
    pub action: Action,
    pub recovery_hint: Option<String>,
    pub protected: Option<ProtectReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Idle,
    Busy,
}

/// What an AI tool's host reports about one of its sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub pid: Pid,
    /// `None` when the host wrote no status file and only transcript activity is known.
    #[serde(default)]
    pub status: Option<SessionStatus>,
    #[serde(default)]
    pub status_since_ms: Option<u64>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub last_transcript_write_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStates {
    #[serde(default)]
    pub sessions: Vec<SessionState>,
}
