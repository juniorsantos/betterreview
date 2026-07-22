use betterreview::cli::Cli;
use clap::Parser as _;

fn main() {
    let _request = Cli::parse().launch_request();
}
