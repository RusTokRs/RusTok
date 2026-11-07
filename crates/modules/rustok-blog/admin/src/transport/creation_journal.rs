use crate::core::creation::{self, CreationScope, JournalError};
use crate::model::BlogPostDraft;

fn storage_key(scope: CreationScope) -> String {
    format!(
        "rustok:blog:create:leptos:{}:{}",
        scope.tenant_id, scope.actor_id
    )
}

pub(crate) fn load(scope: CreationScope) -> Result<Option<BlogPostDraft>, JournalError> {
    read(&storage_key(scope))?
        .map(|raw| creation::decode(scope, &raw))
        .transpose()
}

/// A stale editor cannot replace the record of another creation in this tab.
pub(crate) fn retain(
    scope: CreationScope,
    draft: &BlogPostDraft,
    retry: bool,
) -> Result<(), JournalError> {
    let saved = load(scope)?;
    match (retry, saved) {
        (true, Some(saved)) if saved == *draft => Ok(()),
        (false, None) => {
            let encoded = creation::encode(scope, draft)?;
            write(&storage_key(scope), Some(&encoded))?;
            if load(scope)?.as_ref() == Some(draft) {
                Ok(())
            } else {
                Err(JournalError::Changed)
            }
        }
        _ => Err(JournalError::Changed),
    }
}

pub(crate) fn clear(scope: CreationScope, draft: &BlogPostDraft) -> Result<(), JournalError> {
    if load(scope)?.as_ref() != Some(draft) {
        return Err(JournalError::Changed);
    }
    write(&storage_key(scope), None)
}

fn read(_key: &str) -> Result<Option<String>, JournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        leptos_use::storage::StorageType::Session
            .into_storage()
            .map_err(|_| JournalError::Unavailable)?
            .ok_or(JournalError::Unavailable)?
            .get_item(_key)
            .map_err(|_| JournalError::Unavailable)
    }
    #[cfg(not(target_arch = "wasm32"))]
    Err(JournalError::Unavailable)
}

fn write(_key: &str, _value: Option<&str>) -> Result<(), JournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        let storage = leptos_use::storage::StorageType::Session
            .into_storage()
            .map_err(|_| JournalError::Unavailable)?
            .ok_or(JournalError::Unavailable)?;
        match _value {
            Some(value) => storage.set_item(_key, value),
            None => storage.remove_item(_key),
        }
        .map_err(|_| JournalError::Unavailable)
    }
    #[cfg(not(target_arch = "wasm32"))]
    Err(JournalError::Unavailable)
}
