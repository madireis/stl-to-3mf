//! Omni3MF CLI entry point.

mod args;
mod convert;
mod inspect;
mod sanitize;
mod table;

use clap::Parser;
use std::process;

use args::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Convert(args) => convert::execute_convert(args),
        Commands::Inspect(args) => inspect::execute_inspect(args),
        Commands::Sanitize(args) => sanitize::execute_sanitize(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        process::exit(1);
    }
}
