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
    after_help = "CLI observation is disabled by default. Set [locus] enabled = true in config.toml under the platform data home for ironbed (~/.ironbed on Unix, LOCALAPPDATA/ironbed on Windows). The default HTTP receiver at http://127.0.0.1:43308 uses state/locus under that home. For HTTPS, set endpoint and an absolute writer key_file in [locus]. The recovery queue is state/locus.https/ironbed under the same home. On Unix, sender can name an absolute Locus executable. With all three unset, LOCUS_API, LOCUS_API_KEY_FILE and LOCUS_SENDER supply the settings. Observation failures preserve execution frames, cleanup and exit status."
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
    let observer = observation::start(name);
    let code = execute(command);
    if let Some(observer) = observer {
        observer.finish(if code == ExitCode::SUCCESS { 0 } else { 2 });
    }
    code
}

#[locus::trace(with = plumb::observation::view())]
fn execute(command: Option<Command>) -> ExitCode {
    match command {
        Some(Command::Run { seat, cancel }) => run::start(&seat, cancel.as_deref()),
        None => ExitCode::SUCCESS,
    }
}
