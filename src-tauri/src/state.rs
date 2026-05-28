use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use memento::duckdb::Connection;
use tokio::task::JoinHandle;
use memento::tokio_util::sync::CancellationToken;

use memento::config::AppConfig;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub config: Arc<RwLock<AppConfig>>,
    pub scan_manager: Arc<Mutex<ScanManager>>,
}

pub struct ScanManager {
    pub active_scans: HashMap<i64, ScanHandle>,
}

pub struct ScanHandle {
    pub join_handle: JoinHandle<()>,
    pub cancel_token: CancellationToken,
}

impl ScanManager {
    pub fn new() -> Self {
        Self {
            active_scans: HashMap::new(),
        }
    }

    pub fn register(&mut self, scan_run_id: i64, handle: ScanHandle) {
        self.active_scans.insert(scan_run_id, handle);
    }

    pub fn cancel(&self, scan_run_id: i64) -> bool {
        if let Some(handle) = self.active_scans.get(&scan_run_id) {
            handle.cancel_token.cancel();
            true
        } else {
            false
        }
    }
}
