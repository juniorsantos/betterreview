use betterreview::{cli::Cli, entrypoint};
use clap::Parser as _;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match entrypoint::run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
