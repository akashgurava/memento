use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use memento::db::queries;
use memento::error::{MementoError, ScanError};
use memento::scanner::progress::{ProgressReporter, ScanProgress};
use memento::scanner::{level1, level2, level3};
use crate::state::{AppState, ScanHandle};

use serde::Serialize;

/// Bridges memento's ProgressReporter trait to Tauri events
struct TauriProgressReporter {
    app: AppHandle,
}

impl ProgressReporter for TauriProgressReporter {
    fn report(&self, progress: &ScanProgress) {
        let _ = self.app.emit("scan:progress", progress);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanRunStatus {
    pub id: i64,
    pub scan_level: i32,
    pub hash_type: Option<String>,
    pub status: String,
    pub files_processed: i64,
    pub files_total: Option<i64>,
    pub error_message: Option<String>,
}

#[tauri::command]
pub async fn start_scan(
    level: u8,
    hash_type: Option<String>,
    _root_dir: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let db = Arc::clone(&state.db);
    let config = {
        let c = state.config.read().map_err(|e| e.to_string())?;
        c.clone()
    };

    if level == 3 && hash_type.is_none() {
        return Err("hash_type is required for level 3 scans".into());
    }

    let scan_run_id = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        queries::create_scan_run(
            &conn,
            level as i32,
            hash_type.as_deref(),
            _root_dir.as_deref(),
        )
        .map_err(|e| e.to_string())?
    };

    let cancel_token = CancellationToken::new();
    let token_clone = cancel_token.clone();
    let hash_type_clone = hash_type.clone();
    let db_clone = Arc::clone(&db);

    let join_handle = tokio::task::spawn_blocking(move || {
        let reporter = TauriProgressReporter { app };

        let result = match level {
            1 => level1::run_stats_scan(&config, scan_run_id, &reporter, &token_clone).map(|_| ()),
            2 => level2::run_metadata_scan(&config, &db_clone, scan_run_id, &reporter, &token_clone),
            3 => {
                let ht = hash_type_clone.as_deref().unwrap_or("blake3");
                level3::run_hash_scan(&config, &db_clone, scan_run_id, ht, &reporter, &token_clone)
            }
            _ => Err(ScanError::invalid_level(level)),
        };

        let status = match &result {
            Ok(_) => "completed",
            Err(MementoError::Scan(memento::error::ScanError::Cancelled)) => "cancelled",
            Err(_) => "error",
        };
        let error_msg = match &result {
            Err(e) if !matches!(e, MementoError::Scan(memento::error::ScanError::Cancelled)) => {
                Some(e.to_string())
            }
            _ => None,
        };

        if let Ok(conn) = db_clone.lock() {
            let _ = queries::complete_scan_run(&conn, scan_run_id, status, error_msg.as_deref());
        }
    });

    {
        let mut mgr = state.scan_manager.lock().map_err(|e| e.to_string())?;
        mgr.register(scan_run_id, ScanHandle {
            join_handle,
            cancel_token,
        });
    }

    Ok(scan_run_id)
}

#[tauri::command]
pub fn cancel_scan(scan_run_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let mgr = state.scan_manager.lock().map_err(|e| e.to_string())?;
    if !mgr.cancel(scan_run_id) {
        return Err(format!("No active scan with id {}", scan_run_id));
    }
    Ok(())
}

#[tauri::command]
pub fn get_scan_status(scan_run_id: i64, state: State<'_, AppState>) -> Result<ScanRunStatus, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, scan_level, hash_type, status, files_processed, files_total, error_message FROM scan_runs WHERE id = ?")
        .map_err(|e| e.to_string())?;

    stmt.query_row([scan_run_id], |row| {
        Ok(ScanRunStatus {
            id: row.get(0)?,
            scan_level: row.get(1)?,
            hash_type: row.get(2)?,
            status: row.get(3)?,
            files_processed: row.get(4)?,
            files_total: row.get(5)?,
            error_message: row.get(6)?,
        })
    })
    .map_err(|e| e.to_string())
}
