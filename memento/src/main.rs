use clap::Parser;

use memento::{AppConfig, Cli, MementoError};

fn main() -> Result<(), MementoError> {
    let config = AppConfig::from_cli(Cli::parse())?;
    println!("{config:#?}");
    Ok(())
}
