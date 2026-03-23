use clap::Parser;

use stellatune_icon::cli::Cli;

fn main() -> anyhow::Result<()> {
    Cli::parse().run()
}
