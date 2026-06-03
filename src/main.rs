use std::{
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{sleep, spawn},
};

use clap::Parser;

use ctrlc::{set_handler, Error as CtrlcError};
use memento::{AppConfig, Bus, Cli};

/// Handle Ctrl+C signal to exit the application gracefully
fn setup_ctrlc() -> Result<Arc<AtomicBool>, CtrlcError> {
    let cancel = Arc::new(AtomicBool::new(false));

    let cancel_clone = cancel.clone();
    set_handler(move || {
        cancel_clone.store(true, Ordering::Relaxed);
        tracing::warn!("SIGTERM_RECEIVED. Shutting down.");

        spawn(|| {
            sleep(std::time::Duration::from_secs(5));
            exit(1);
        });
    })?;
    Ok(cancel)
}

fn tracing_init() {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .init();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_init();
    tracing::info!("START_APPLICATION.");

    let config = AppConfig::from_cli(Cli::parse())?;
    tracing::debug!("Config: {:#?}", config);

    let cancel = setup_ctrlc()?;

    tracing::info!("ROOT_SCAN. START.");
    let bus = Bus::scan(&config, cancel)?;
    let result = bus
        .run()
        .join()
        .unwrap_or_else(|e| Err(memento::MementoError::thread_panic("bus_consumer".into(), e)));
    result?;
    tracing::info!("ROOT_SCAN. SUCCESS.");

    Ok(())
}
