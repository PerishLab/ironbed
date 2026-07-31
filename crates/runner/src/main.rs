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
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Some(Command::Run { seat }) => run::start(&seat),
        None => ExitCode::SUCCESS,
    }
}
