use clap::Parser;
use rs_peekaboo::cli::{Cli, execute};

fn main() {
    let cli = Cli::parse();
    if let Err(err) = execute(cli) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
