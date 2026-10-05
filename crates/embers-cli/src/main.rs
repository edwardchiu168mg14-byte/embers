use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    Scan,
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
        Command::Scan => "scan",
        Command::Watch => "watch",
        Command::Explain => "explain",
        Command::Extinguish => "extinguish",
        Command::Config => "config",
    };
    eprintln!("embers {name}: not implemented yet");
    ExitCode::from(2)
}
