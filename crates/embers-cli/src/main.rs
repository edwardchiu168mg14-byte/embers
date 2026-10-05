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
                println!("{}", json_safe(&serde_json::to_string(ember)?));
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

/// Characters never written raw, because a terminal or a line-based reader could act on
/// them instead of showing them (rule S10): C0/C1 controls and DEL, line and paragraph
/// separators (U+2028/2029), bidi controls (U+061C, U+200E/200F, U+202A–202E, U+2066–2069),
/// invisible formatting characters (U+00AD, U+180E, U+200B–200F, U+2060–2064, U+FEFF,
/// U+FFF9–FFFB) and tag characters (U+E0000–E007F).
fn needs_escape(c: char) -> bool {
    c.is_control()
        || embers_core::redact::is_invisible(c)
        || matches!(
            c,
            '\u{061C}'
                | '\u{180E}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

/// Text from processes is untrusted: such characters become visible `\u{XXXX}` escapes.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if needs_escape(c) {
            out.push_str(&format!("\\u{{{:04X}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// A serialized JSON line with every such character written as a `\uXXXX` escape
/// (serde_json only escapes U+0000–U+001F), so NDJSON stays one clean line per ember.
fn json_safe(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        if needs_escape(c) {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{json_safe, plain};

    const NASTY: [char; 9] = [
        '\u{1b}',
        '\u{7f}',
        '\u{85}',
        '\u{9b}',
        '\u{200b}',
        '\u{202e}',
        '\u{2028}',
        '\u{feff}',
        '\u{e0041}',
    ];

    #[test]
    fn text_output_escapes_risky_characters() {
        for c in NASTY {
            let shown = plain(&format!("a{c}b"));
            assert!(!shown.contains(c), "{c:?} printed raw: {shown}");
            assert!(
                shown.starts_with("a\\u{") && shown.ends_with("}b"),
                "{shown}"
            );
        }
        assert_eq!(plain("<script> é ✓"), "<script> é ✓");
    }

    #[test]
    fn json_output_escapes_risky_characters() {
        for c in NASTY {
            let line = json_safe(&serde_json::to_string(&format!("a{c}b")).unwrap());
            assert!(!line.contains(c), "{c:?} written raw: {line}");
            let back: String = serde_json::from_str(&line).unwrap();
            assert_eq!(back, format!("a{c}b"), "escape must round-trip");
        }
    }
}
