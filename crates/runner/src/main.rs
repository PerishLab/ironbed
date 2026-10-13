mod observation;
mod run;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "ironbed",
    version = plumb::version!("IRONBED"),
    about = "Open execution testbed",
    after_help = "CLI observation is disabled by default. Set [locus] enabled = true in config.toml under the platform data home for ironbed (~/.ironbed on Unix, LOCALAPPDATA/ironbed on Windows). Buffered observations use state/locus there. endpoint in [locus] overrides LOCUS_API, then http://127.0.0.1:43308. Observation failures preserve execution frames, cleanup and exit status."
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
    if let Err(error) = plumb::identity!("IRONBED") {
        eprintln!("ironbed: {error}");
        return ExitCode::FAILURE;
    }
    let command = Cli::parse().command;
    let name = if command.is_some() { "run" } else { "idle" };
    let observer = observation::Run::start(name);
    let code = execute(command, observer.as_ref());
    if let Some(observer) = observer {
        observer.finish(if code == ExitCode::SUCCESS { 0 } else { 2 });
    }
    code
}

#[locus::trace(with = observer.map(observation::Run::view))]
fn execute(command: Option<Command>, observer: Option<&observation::Run>) -> ExitCode {
    match command {
        Some(Command::Run { seat, cancel }) => run::start(&seat, cancel.as_deref(), observer),
        None => ExitCode::SUCCESS,
    }
}
