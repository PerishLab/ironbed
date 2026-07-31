use clap::Parser;

#[derive(Parser)]
#[command(
    name = "ironbed",
    version = plumb::version!("IRONBED"),
    about = "Open execution testbed"
)]
struct Cli {}

fn main() {
    Cli::parse();
}
