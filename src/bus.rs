use std::thread::JoinHandle;

use crate::error::MementoError;

pub trait Message: Send + 'static {
    fn is_terminal(&self) -> bool;
}

pub trait Producer {
    type Message: Message;

    fn produce(&mut self) -> Self::Message;
}

pub trait Consumer {
    type Message: Message;

    fn consume(&mut self, message: &Self::Message) -> Result<(), MementoError>;

    /// Called once after all messages have been consumed.
    fn finish(&mut self) -> Result<(), MementoError> {
        Ok(())
    }
}

/// Connects a [`Producer`] on the calling thread to two [`Consumer`]s
/// running on a background thread, linked by a bounded channel.
pub struct Bus<P, C1, C2>
where
    P: Producer,
    C1: Consumer<Message = P::Message> + Send + 'static,
    C2: Consumer<Message = P::Message> + Send + 'static,
{
    producer: P,
    c1: C1,
    c2: C2,
}

impl<P, C1, C2> Bus<P, C1, C2>
where
    P: Producer,
    C1: Consumer<Message = P::Message> + Send + 'static,
    C2: Consumer<Message = P::Message> + Send + 'static,
{
    pub fn new(producer: P, c1: C1, c2: C2) -> Self {
        Self { producer, c1, c2 }
    }

    pub fn run(self) -> JoinHandle<Result<(), MementoError>> {
        let (tx, rx) = crossbeam::channel::bounded(64);
        let Self {
            mut producer,
            mut c1,
            mut c2,
        } = self;

        let handle = std::thread::spawn(move || {
            for msg in rx {
                c1.consume(&msg)?;
                c2.consume(&msg)?;
            }
            c1.finish()?;
            c2.finish()?;
            Ok(())
        });

        loop {
            let msg = producer.produce();
            let terminal = msg.is_terminal();
            if tx.send(msg).is_err() {
                break;
            }
            if terminal {
                break;
            }
        }
        drop(tx);

        handle
    }
}

#[cfg(feature = "cli")]
mod cli {
    use super::*;

    use std::sync::{atomic::AtomicBool, Arc};

    use crate::{config::AppConfig, scan::cli::ScanCliConsumer, DbScanConsumer, Walker};

    impl<'a> Bus<Walker<'a>, ScanCliConsumer, DbScanConsumer> {
        pub fn scan(config: &'a AppConfig, cancel: Arc<AtomicBool>) -> Result<Self, MementoError> {
            let producer = Walker::new(config, cancel)?;
            let c1 = ScanCliConsumer::new(&config.roots()?)?;
            let c2 = DbScanConsumer::new(config.db_path())?;
            Ok(Self::new(producer, c1, c2))
        }
    }
}
