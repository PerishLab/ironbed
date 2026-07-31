mod run;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "ironbed",
    version = plumb::version!("IRONBED"),
    about = "Open execution testbed"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long)]
        seat: PathBuf,
        #[arg(long)]
        cancel: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Some(Command::Run { seat, cancel }) => run::start(&seat, cancel.as_deref()),
        None => ExitCode::SUCCESS,
    }
}
