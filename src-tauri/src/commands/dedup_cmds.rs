use tauri::State;
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub hash_value: String,
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub files: Vec<FileSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSummary {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub size_bytes: i64,
    pub file_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DedupSummary {
    pub blake3_groups: i64,
    pub content_blake3_groups: i64,
    pub phash_groups: i64,
}

#[tauri::command]
pub fn find_exact_duplicates(hash_type: String, state: State<'_, AppState>) -> Result<Vec<DuplicateGroup>, String> {
    let ht = match hash_type.as_str() {
        "blake3" | "content_blake3" => hash_type.as_str(),
        _ => return Err(format!("Invalid hash type for exact duplicates: {}", hash_type)),
    };

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let sql = format!(
        "SELECT h.hash_value, COUNT(*) as cnt, SUM(v.size_bytes) as total_size
         FROM v_file_hashes h
         JOIN v_files v ON v.id = h.file_id
         WHERE h.hash_value IS NOT NULL AND h.hash_name = '{ht}' AND v.is_missing = false
         GROUP BY h.hash_value HAVING COUNT(*) > 1
         ORDER BY total_size DESC",
        ht = ht
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let groups: Vec<(String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut result = Vec::new();
    for (hash_value, file_count, total_size) in groups {
        let file_sql = format!(
            "SELECT v.id, v.path, v.filename, v.size_bytes, v.file_type
             FROM v_file_hashes h
             JOIN v_files v ON v.id = h.file_id
             WHERE h.hash_name = '{ht}' AND h.hash_value = ? AND v.is_missing = false",
            ht = ht
        );
        let mut file_stmt = conn.prepare(&file_sql).map_err(|e| e.to_string())?;
        let files: Vec<FileSummary> = file_stmt
            .query_map([&hash_value], |row| {
                Ok(FileSummary {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    filename: row.get(2)?,
                    size_bytes: row.get(3)?,
                    file_type: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        result.push(DuplicateGroup {
            hash_value,
            file_count,
            total_size_bytes: total_size,
            files,
        });
    }

    Ok(result)
}

#[tauri::command]
pub fn find_near_duplicates(
    hash_type: String,
    max_distance: u8,
    state: State<'_, AppState>,
) -> Result<Vec<DuplicateGroup>, String> {
    let ht = match hash_type.as_str() {
        "phash" | "dhash" | "whash" => hash_type.as_str(),
        _ => return Err(format!("Invalid hash type for near duplicates: {}", hash_type)),
    };

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let sql = format!(
        "SELECT v.id, v.path, v.filename, v.size_bytes, v.file_type, h.hash_value
         FROM v_file_hashes h
         JOIN v_files v ON v.id = h.file_id
         WHERE h.hash_name = '{ht}' AND h.hash_value IS NOT NULL AND v.is_missing = false",
        ht = ht
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let files: Vec<(i64, String, String, i64, String, i64)> = stmt
        .query_map([], |row| {
            let hash_str: String = row.get(5)?;
            let hash_val: i64 = hash_str.parse().unwrap_or(0);
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, hash_val))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut assigned: Vec<bool> = vec![false; files.len()];

    for i in 0..files.len() {
        if assigned[i] {
            continue;
        }

        let mut group_files = vec![FileSummary {
            id: files[i].0,
            path: files[i].1.clone(),
            filename: files[i].2.clone(),
            size_bytes: files[i].3,
            file_type: files[i].4.clone(),
        }];

        let hash_i = files[i].5;

        for j in (i + 1)..files.len() {
            if assigned[j] {
                continue;
            }

            let hash_j = files[j].5;
            let distance = (hash_i ^ hash_j).count_ones() as u8;

            if distance <= max_distance {
                assigned[j] = true;
                group_files.push(FileSummary {
                    id: files[j].0,
                    path: files[j].1.clone(),
                    filename: files[j].2.clone(),
                    size_bytes: files[j].3,
                    file_type: files[j].4.clone(),
                });
            }
        }

        if group_files.len() > 1 {
            assigned[i] = true;
            let total_size: i64 = group_files.iter().map(|f| f.size_bytes).sum();
            groups.push(DuplicateGroup {
                hash_value: format!("{:016x}", hash_i),
                file_count: group_files.len() as i64,
                total_size_bytes: total_size,
                files: group_files,
            });
        }
    }

    groups.sort_by(|a, b| b.total_size_bytes.cmp(&a.total_size_bytes));
    Ok(groups)
}

#[tauri::command]
pub fn get_duplicate_summary(state: State<'_, AppState>) -> Result<DedupSummary, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let blake3_groups: i64 = conn
        .prepare(
            "SELECT COUNT(*) FROM (
                SELECT hash_value FROM v_file_hashes h
                JOIN v_files v ON v.id = h.file_id
                WHERE h.hash_name = 'blake3' AND h.hash_value IS NOT NULL AND v.is_missing = false
                GROUP BY hash_value HAVING COUNT(*) > 1
            )"
        )
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let content_groups: i64 = conn
        .prepare(
            "SELECT COUNT(*) FROM (
                SELECT hash_value FROM v_file_hashes h
                JOIN v_files v ON v.id = h.file_id
                WHERE h.hash_name = 'content_blake3' AND h.hash_value IS NOT NULL AND v.is_missing = false
                GROUP BY hash_value HAVING COUNT(*) > 1
            )"
        )
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    Ok(DedupSummary {
        blake3_groups,
        content_blake3_groups: content_groups,
        phash_groups: 0,
    })
}
