use clap::Parser;
use rs_peekaboo::cli::{Cli, execute};
use rs_peekaboo::CommandResult;

fn main() {
    let cli = Cli::parse();
    let json_output = cli.json;
    if let Err(err) = execute(cli) {
        if json_output {
            let result = CommandResult::err(err.to_string());
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| {
                    format!(
                        "{{\"ok\":false,\"error\":{}}}",
                        serde_json::to_string(&err.to_string()).unwrap_or_default()
                    )
                })
            );
        } else {
            eprintln!("{err}");
        }
        std::process::exit(1);
    }
}
