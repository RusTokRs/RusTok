//! Media Cleanup Task
//!
//! Scans the `media` table and deletes records whose backing file is
//! confirmed missing from storage.  Safe to run in production: unknown
//! errors are treated conservatively (record is kept).
//!
//! Run manually:
//! ```text
//! cargo loco task --name media_cleanup
//! ```
//! Or schedule via `scheduler.yaml`.

use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Result,
};

pub struct MediaCleanupTask;

#[async_trait]
impl Task for MediaCleanupTask {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "media_cleanup".to_string(),
            detail: "Remove media DB records whose storage objects are missing".to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, _vars: &Vars) -> Result<()> {
        #[cfg(feature = "mod-media")]
        run_media_cleanup(_app_context).await?;

        #[cfg(not(feature = "mod-media"))]
        tracing::info!("mod-media not enabled — media cleanup is a no-op");

        Ok(())
    }
}

#[cfg(feature = "mod-media")]
async fn run_media_cleanup(ctx: &AppContext) -> Result<()> {
    use rustok_media::media::Entity as MediaEntity;
    use rustok_storage::StorageService;
    use sea_orm::EntityTrait;

    let Some(storage) = ctx.shared_store.get::<StorageService>() else {
        tracing::warn!("StorageService not available — skipping media cleanup");
        return Ok(());
    };

    // Verify storage is reachable before scanning.
    let probe = ".media-cleanup-probe";
    if let Err(e) = storage
        .store(probe, bytes::Bytes::from_static(b"probe"), "text/plain")
        .await
    {
        tracing::warn!(error = %e, "Storage backend unreachable — aborting media cleanup");
        return Ok(());
    }
    let _ = storage.delete(probe).await;

    // Fetch full models and use the fields we need. This avoids brittle
    // column-only type plumbing for a maintenance task.
    let records = MediaEntity::find()
        .all(&ctx.db)
        .await
        .map_err(|e| loco_rs::Error::Message(e.to_string()))?;

    let total = records.len();
    let mut removed = 0usize;

    for record in records {
        let id = record.id;
        let path = record.storage_path;

        // Probe the exact object without mutating storage. Missing or invalid
        // paths are safe orphan signals; transient backend/read errors are
        // conservative keep decisions.
        let decision = classify_storage_probe(storage.read(&path).await);

        if decision.remove_record {
            match MediaEntity::delete_by_id(id).exec(&ctx.db).await {
                Ok(_) => {
                    removed += 1;
                    tracing::info!(media_id = %id, path, "Removed orphaned media record");
                }
                Err(e) => {
                    tracing::warn!(media_id = %id, error = %e, "Failed to purge orphaned record");
                }
            }
        } else if let Some(error) = decision.keep_reason {
            tracing::debug!(media_id = %id, path, error, "Keeping media record after conservative storage probe");
        }
    }

    tracing::info!(scanned = total, removed, "Media cleanup complete");
    Ok(())
}

#[cfg(feature = "mod-media")]
struct CleanupProbeDecision {
    remove_record: bool,
    keep_reason: Option<String>,
}

#[cfg(feature = "mod-media")]
fn classify_storage_probe(
    result: std::result::Result<bytes::Bytes, rustok_storage::StorageError>,
) -> CleanupProbeDecision {
    use rustok_storage::StorageError;

    match result {
        Ok(_) => CleanupProbeDecision {
            remove_record: false,
            keep_reason: None,
        },
        Err(StorageError::NotFound(_) | StorageError::InvalidPath(_)) => CleanupProbeDecision {
            remove_record: true,
            keep_reason: None,
        },
        Err(error) => CleanupProbeDecision {
            remove_record: false,
            keep_reason: Some(error.to_string()),
        },
    }
}

#[cfg(all(test, feature = "mod-media"))]
mod tests {
    use super::classify_storage_probe;
    use rustok_storage::StorageError;

    #[test]
    fn cleanup_probe_removes_records_for_missing_or_invalid_paths() {
        let not_found = classify_storage_probe(Err(StorageError::NotFound("missing".to_string())));
        let invalid = classify_storage_probe(Err(StorageError::InvalidPath("../bad".to_string())));

        assert!(not_found.remove_record);
        assert!(invalid.remove_record);
        assert_eq!(not_found.keep_reason, None);
        assert_eq!(invalid.keep_reason, None);
    }

    #[test]
    fn cleanup_probe_keeps_records_when_object_is_readable_or_backend_errors() {
        let readable = classify_storage_probe(Ok(bytes::Bytes::from_static(b"asset")));
        let backend_error =
            classify_storage_probe(Err(StorageError::Backend("timeout".to_string())));

        assert!(!readable.remove_record);
        assert_eq!(readable.keep_reason, None);
        assert!(!backend_error.remove_record);
        assert_eq!(
            backend_error.keep_reason.as_deref(),
            Some("Backend error: timeout")
        );
    }
}
