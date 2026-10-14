use std::process::ExitCode;

use clap::Parser;
use leake::cli::Cli;
use leake::converter;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match converter::run(&cli.into_options()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
