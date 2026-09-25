use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::AppResult;
use crate::storage::SqliteStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRecord {
    pub id: String,
    pub remote_id: String,
    pub filename: String,
    pub path: String,
    pub sha256: String,
    pub size: i64,
    pub downloaded_at: String,
}

#[derive(Clone)]
pub struct DownloadManager {
    store: SqliteStore,
    base_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(store: SqliteStore, base_dir: PathBuf) -> Self {
        Self { store, base_dir }
    }

    pub async fn save_invoice_file(
        &self,
        remote_id: &str,
        filename: &str,
        bytes: &[u8],
    ) -> AppResult<DownloadRecord> {
        let now = Utc::now();
        let subdir = self
            .base_dir
            .join(format!("{:04}", now.year()))
            .join(format!("{:02}", now.month()));
        std::fs::create_dir_all(&subdir)?;

        let safe_name = filename
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let full_path = subdir.join(&safe_name);
        std::fs::write(&full_path, bytes)?;

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sha256 = hex::encode(hasher.finalize());
        let size = bytes.len() as i64;
        let id = format!("dl_{}", Uuid::new_v4().simple());
        let downloaded_at = now.to_rfc3339();
        let path_str = full_path.to_string_lossy().to_string();

        sqlx::query(
            "INSERT INTO downloads (id, remote_id, filename, path, sha256, size, downloaded_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(remote_id)
        .bind(&safe_name)
        .bind(&path_str)
        .bind(&sha256)
        .bind(size)
        .bind(&downloaded_at)
        .execute(self.store.pool())
        .await?;

        Ok(DownloadRecord {
            id,
            remote_id: remote_id.to_string(),
            filename: safe_name,
            path: path_str,
            sha256,
            size,
            downloaded_at,
        })
    }

    pub async fn get_by_remote_id(&self, remote_id: &str) -> AppResult<Option<DownloadRecord>> {
        let row = sqlx::query(
            "SELECT * FROM downloads WHERE remote_id = ? ORDER BY downloaded_at DESC LIMIT 1",
        )
        .bind(remote_id)
        .fetch_optional(self.store.pool())
        .await?;

        Ok(row.map(|r| DownloadRecord {
            id: r.get("id"),
            remote_id: r.get("remote_id"),
            filename: r.get("filename"),
            path: r.get("path"),
            sha256: r.get("sha256"),
            size: r.get("size"),
            downloaded_at: r.get("downloaded_at"),
        }))
    }
}
