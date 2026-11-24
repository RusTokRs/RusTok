use std::sync::Arc;

use chrono::{DateTime, Utc};
use object_store::ObjectStoreExt;
use rustok_storage::{ObjectKey, ObjectScope, ObjectZone, StorageRuntime};
use uuid::Uuid;

use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Clone)]
struct StorageHealthProbeState {
    object_id: Uuid,
    created_at: DateTime<Utc>,
    operation_lock: Arc<tokio::sync::Mutex<()>>,
}

impl StorageHealthProbeState {
    fn new() -> Self {
        Self {
            object_id: Uuid::new_v4(),
            created_at: Utc::now(),
            operation_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

fn storage_health_probe_state(ctx: &ServerRuntimeContext) -> StorageHealthProbeState {
    ctx.shared_get_or_insert_with(StorageHealthProbeState::new)
}

fn storage_health_probe_key(
    state: &StorageHealthProbeState,
) -> std::result::Result<object_store::path::Path, String> {
    ObjectKey::chronological(
        "platform-health",
        ObjectZone::Staging,
        ObjectScope::Platform,
        state.created_at.clone(),
        state.object_id,
        "probe",
    )
    .map(ObjectKey::into_path)
    .map_err(|_| "storage health probe could not be constructed".to_string())
}

/// Probe the configured object store with one stable, runtime-owned staging key.
///
/// The lock is shared by cloned runtime contexts and by HTTP/GraphQL health
/// surfaces. If a write reports an ambiguous failure after creating the object,
/// cleanup is attempted here and the next probe reuses the same key.
pub(crate) async fn probe_storage_backend(
    ctx: &ServerRuntimeContext,
    storage: &StorageRuntime,
) -> std::result::Result<(), String> {
    let state = storage_health_probe_state(ctx);
    let _operation_guard = state.operation_lock.lock().await;
    let probe = storage_health_probe_key(&state)?;
    let data = bytes::Bytes::from_static(b"ok");

    if storage
        .objects
        .put_opts(&probe, data.into(), storage.put_options("text/plain"))
        .await
        .is_err()
    {
        if storage.objects.delete(&probe).await.is_err() {
            tracing::warn!(
                error_code = "storage_health_probe_ambiguous_write_cleanup_failed",
                "Storage health probe cleanup after an ambiguous write failure did not complete"
            );
        }
        return Err("storage health probe write failed".to_string());
    }

    storage
        .objects
        .delete(&probe)
        .await
        .map_err(|_| "storage health probe cleanup failed".to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::RustokSettings;
    use sea_orm::DatabaseConnection;

    #[test]
    fn probe_identity_is_unique_per_runtime_and_shared_by_clones() {
        let first_context = ServerRuntimeContext::new(
            DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let first = storage_health_probe_state(&first_context);
        let first_clone = storage_health_probe_state(&first_context.clone());

        assert!(!first.object_id.is_nil());
        assert_eq!(first.object_id, first_clone.object_id);
        assert!(Arc::ptr_eq(
            &first.operation_lock,
            &first_clone.operation_lock
        ));

        let second_context = ServerRuntimeContext::new(
            DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let second = storage_health_probe_state(&second_context);
        assert_ne!(first.object_id, second.object_id);
        assert_ne!(
            storage_health_probe_key(&first).expect("first probe key is valid"),
            storage_health_probe_key(&second).expect("second probe key is valid")
        );
    }
}
