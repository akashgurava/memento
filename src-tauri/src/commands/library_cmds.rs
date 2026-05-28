use tauri::State;
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct LibraryStats {
    pub total_files: i64,
    pub total_size_bytes: i64,
    pub image_count: i64,
    pub image_size_bytes: i64,
    pub video_count: i64,
    pub video_size_bytes: i64,
    pub other_count: i64,
    pub other_size_bytes: i64,
    pub last_scan_at: Option<String>,
}

#[tauri::command]
pub fn get_library_stats(state: State<'_, AppState>) -> Result<LibraryStats, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let total_files: i64 = conn
        .prepare("SELECT COUNT(*) FROM v_files WHERE is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let total_size: i64 = conn
        .prepare("SELECT COALESCE(SUM(size_bytes), 0) FROM v_files WHERE is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let image_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM v_files WHERE file_type = 'image' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let image_size: i64 = conn
        .prepare("SELECT COALESCE(SUM(size_bytes), 0) FROM v_files WHERE file_type = 'image' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let video_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM v_files WHERE file_type = 'video' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let video_size: i64 = conn
        .prepare("SELECT COALESCE(SUM(size_bytes), 0) FROM v_files WHERE file_type = 'video' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let other_count = total_files - image_count - video_count;
    let other_size = total_size - image_size - video_size;

    let last_scan_at: Option<String> = conn
        .prepare("SELECT MAX(completed_at)::VARCHAR FROM scan_runs WHERE status = 'completed'")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .ok()
        .flatten();

    Ok(LibraryStats {
        total_files,
        total_size_bytes: total_size,
        image_count,
        image_size_bytes: image_size,
        video_count,
        video_size_bytes: video_size,
        other_count,
        other_size_bytes: other_size,
        last_scan_at,
    })
}
