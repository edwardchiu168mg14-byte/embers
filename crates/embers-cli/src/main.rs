use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use embers_core::fixture;
use embers_core::{classify, config, reason, Ember};

/// Spots processes still smoldering after your AI coding sessions end.
#[derive(Parser)]
#[command(name = "embers", version = embers_core::version(), about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List leftover and runaway processes
    Scan {
        /// Classify a recorded snapshot (file or directory of fixtures) instead of this machine
        #[arg(long)]
        fixture: Option<PathBuf>,
        /// Print one JSON object per ember
        #[arg(long)]
        json: bool,
        /// Extra settings file (TOML) applied on top of the defaults
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Keep scanning and print changes
    Watch,
    /// Explain why a process was flagged
    Explain,
    /// End a flagged process (asks first)
    Extinguish,
    /// Show or edit settings
    Config,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let name = match cli.command {
        Command::Scan {
            fixture: Some(path),
            json,
            config,
        } => {
            return match scan_fixtures(&path, json, config.as_deref()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("embers: {err}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Scan { fixture: None, .. } => "scan",
        Command::Watch => "watch",
        Command::Explain => "explain",
        Command::Extinguish => "extinguish",
        Command::Config => "config",
    };
    eprintln!("embers {name}: not implemented yet");
    ExitCode::from(2)
}

fn scan_fixtures(
    path: &Path,
    json: bool,
    user_config: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let user_toml = user_config.map(std::fs::read_to_string).transpose()?;
    let files = fixture::list(path)?;
    for file in &files {
        let fx = fixture::load(file)?;
        let mut cfg = fixture::config_for(&fx)?;
        if let Some(toml) = &user_toml {
            cfg = config::merge(cfg, toml)?;
        }
        let embers = classify(fx.prev.as_ref(), &fx.cur, &cfg, &fx.hosts);
        if json {
            for ember in &embers {
                println!("{}", serde_json::to_string(ember)?);
            }
            continue;
        }
        if files.len() > 1 {
            println!("# {}", plain(&file.display().to_string()));
        }
        if embers.is_empty() {
            println!("nothing smoldering");
        }
        for ember in &embers {
            print_row(ember);
        }
    }
    Ok(())
}

fn print_row(e: &Ember) {
    let metrics = format!(
        "{} · {} pid{}",
        reason::bytes(e.metrics.footprint_bytes),
        e.pids.len(),
        if e.pids.len() == 1 { "" } else { "s" }
    );
    println!(
        "{:<18} {:<14} {:<15} {}  [{}]",
        plain(&e.name),
        format!("{:?}", e.origin),
        format!("{:?}", e.action),
        plain(&e.reason),
        metrics
    );
    if let Some(hint) = &e.recovery_hint {
        println!("{:18} ↳ {}", "", plain(hint));
    }
}

/// Text from processes is untrusted: show control characters (e.g. terminal escape
/// sequences) as visible escapes instead of letting the terminal interpret them.
fn plain(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::plain;

    #[test]
    fn control_characters_are_escaped() {
        assert_eq!(plain("a\u{1b}[31mb\nc"), "a\\u{1b}[31mb\\nc");
        assert_eq!(plain("<script>"), "<script>");
    }
}
