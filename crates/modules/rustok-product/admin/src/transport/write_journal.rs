/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Per-tab browser projection of the owner receipt intent. Exact inputs/key survive reload;
//! corruption, missing storage, a changed generation, or a removed record never authorize a new
//! write. The URL carries a document marker (not credentials), so cleared session storage is not
//! mistaken for a first visit on reload. Operation generations rotate only after terminal proof.

#[cfg(target_arch = "wasm32")]
use crate::core::write_journal::JournalScope;

use crate::core::write_journal::{JournalState, WriteJournal, WriteJournalError};

#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, WriteJournalError> {
    web_sys::window()
        .ok_or(WriteJournalError::Unavailable)?
        .session_storage()
        .map_err(|_| WriteJournalError::Unavailable)?
        .ok_or(WriteJournalError::Unavailable)
}

#[cfg(target_arch = "wasm32")]
fn location() -> Result<web_sys::Url, WriteJournalError> {
    let href = web_sys::window()
        .ok_or(WriteJournalError::Unavailable)?
        .location()
        .href()
        .map_err(|_| WriteJournalError::Unavailable)?;
    web_sys::Url::new(&href).map_err(|_| WriteJournalError::Unavailable)
}

#[cfg(target_arch = "wasm32")]
fn validate_scope_document<S: JournalState>(scope: &S::Scope) -> Result<(), WriteJournalError> {
    if let Some(attribute_id) = scope.attribute_document_id() {
        let option_id = location()?
            .search_params()
            .get(rustok_ui_core::AdminQueryKey::OptionId.as_str());
        validate_definition_document_selection(Some(attribute_id), option_id.as_deref())?;
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn document_marker<S: JournalState>() -> Result<Option<uuid::Uuid>, WriteJournalError> {
    let params = location()?.search_params();
    if params.get_all(S::DOCUMENT_QUERY.as_str()).length() > 1 {
        return Err(WriteJournalError::Invalid);
    }
    params
        .get(S::DOCUMENT_QUERY.as_str())
        .map(|raw| {
            uuid::Uuid::parse_str(&raw)
                .map_err(|_| WriteJournalError::Invalid)
                .and_then(|id| {
                    if id.is_nil() {
                        Err(WriteJournalError::Invalid)
                    } else {
                        Ok(id)
                    }
                })
        })
        .transpose()
}

#[cfg(target_arch = "wasm32")]
fn attach_marker<S: JournalState>(record: &WriteJournal<S>) -> Result<(), WriteJournalError> {
    let location = location()?;
    location.search_params().set(
        S::DOCUMENT_QUERY.as_str(),
        &record.document_intent.to_string(),
    );
    let history = web_sys::window()
        .ok_or(WriteJournalError::Unavailable)?
        .history()
        .map_err(|_| WriteJournalError::Unavailable)?;
    // Preserve the host router's state; this is a recovery annotation, not a navigation.
    let state = history
        .state()
        .map_err(|_| WriteJournalError::Unavailable)?;
    history
        .replace_state_with_url(&state, "", Some(&location.href()))
        .map_err(|_| WriteJournalError::Unavailable)?;
    if document_marker::<S>()? != Some(record.document_intent) {
        return Err(WriteJournalError::Changed);
    }
    Ok(())
}

pub(crate) fn load<S: JournalState>(
    _scope: &S::Scope,
) -> Result<WriteJournal<S>, WriteJournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        validate_scope_document::<S>(_scope)?;
        let storage = storage()?;
        let marker = document_marker::<S>()?;
        let key = _scope.storage_key::<S>();
        let record = match storage
            .get_item(&key)
            .map_err(|_| WriteJournalError::Unavailable)?
        {
            Some(raw) => WriteJournal::<S>::decode(&raw, _scope)?,
            None if marker.is_some() => return Err(WriteJournalError::Changed),
            None => {
                let record = WriteJournal::<S>::new(_scope.clone())?;
                let raw = record.encode()?;
                storage
                    .set_item(&key, &raw)
                    .map_err(|_| WriteJournalError::Unavailable)?;
                if storage
                    .get_item(&key)
                    .map_err(|_| WriteJournalError::Unavailable)?
                    .as_deref()
                    != Some(raw.as_str())
                {
                    return Err(WriteJournalError::Unavailable);
                }
                record
            }
        };
        match marker {
            Some(marker) if marker != record.document_intent => {
                return Err(WriteJournalError::Changed);
            }
            None => attach_marker(&record)?,
            _ => {}
        }
        Ok(record)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}

/// Read only: never initialize a missing predecessor or silently replace a document marker.
pub(crate) fn read<S: JournalState>(
    _scope: &S::Scope,
) -> Result<WriteJournal<S>, WriteJournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        validate_scope_document::<S>(_scope)?;
        let raw = storage()?
            .get_item(&_scope.storage_key::<S>())
            .map_err(|_| WriteJournalError::Unavailable)?
            .ok_or(WriteJournalError::Changed)?;
        let record = WriteJournal::<S>::decode(&raw, _scope)?;
        if document_marker::<S>()? != Some(record.document_intent) {
            return Err(WriteJournalError::Changed);
        }
        Ok(record)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}

pub(crate) fn store<S: JournalState>(
    expected: &WriteJournal<S>,
    next: &WriteJournal<S>,
) -> Result<(), WriteJournalError> {
    if !expected.can_transition_to(next) {
        return Err(WriteJournalError::Changed);
    }
    if read::<S>(&expected.scope)? != *expected {
        return Err(WriteJournalError::Changed);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let encoded = next.encode()?;
        storage()?
            .set_item(&expected.scope.storage_key::<S>(), &encoded)
            .map_err(|_| WriteJournalError::Unavailable)?;
        if read::<S>(&expected.scope)? != *next {
            return Err(WriteJournalError::Changed);
        }
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}

/// Check the persisted predecessor immediately before a retry, not merely at component mount.
pub(crate) fn verify<S: JournalState>(expected: &WriteJournal<S>) -> Result<(), WriteJournalError> {
    if read::<S>(&expected.scope)? == *expected {
        Ok(())
    } else {
        Err(WriteJournalError::Changed)
    }
}

pub(crate) fn reload_owner_document() -> Result<(), WriteJournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .ok_or(WriteJournalError::Unavailable)?
            .location()
            .reload()
            .map_err(|_| WriteJournalError::Unavailable)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}

/// Full-document navigation keeps an explicit fresh lifecycle intent from rebasing a dirty form.
pub(crate) fn navigate_owner_document(_path: &str) -> Result<(), WriteJournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .ok_or(WriteJournalError::Unavailable)?
            .location()
            .assign(_path)
            .map_err(|_| WriteJournalError::Unavailable)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}

/// Fresh has no command body in its Ready envelope. Publish the frozen target before
/// committing that envelope so even failed full navigation reloads the original target.
pub(crate) fn replace_owner_document_selection(_path: &str) -> Result<(), WriteJournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        let current = location()?;
        let target = web_sys::Url::new_with_base(_path, &current.href())
            .map_err(|_| WriteJournalError::Invalid)?;
        if target.origin() != current.origin() {
            return Err(WriteJournalError::Invalid);
        }
        let history = web_sys::window()
            .ok_or(WriteJournalError::Unavailable)?
            .history()
            .map_err(|_| WriteJournalError::Unavailable)?;
        let state = history
            .state()
            .map_err(|_| WriteJournalError::Unavailable)?;
        history
            .replace_state_with_url(&state, "", Some(&target.href()))
            .map_err(|_| WriteJournalError::Unavailable)?;
        if location()?.href() != target.href() {
            return Err(WriteJournalError::Changed);
        }
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}

pub(crate) fn validate_recovery_selection<S: JournalState>(
    _has_selection: bool,
) -> Result<(), WriteJournalError> {
    #[cfg(target_arch = "wasm32")]
    {
        if !_has_selection && document_marker::<S>()?.is_some() {
            Err(WriteJournalError::Changed)
        } else {
            Ok(())
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}

/// Recovery admission checks the typed URL snapshot before creating a dictionary baseline.
/// This is not a query reader: routing owns selection; duplicate/rewritten evidence must not
/// turn an existing document into a new attempt after storage was cleared.
pub(crate) fn validate_definition_document_selection(
    _attribute_id: Option<&str>,
    _option_id: Option<&str>,
) -> Result<(), WriteJournalError> {
    if !crate::core::attribute_lifecycle::valid_document_selection(_attribute_id, _option_id) {
        return Err(WriteJournalError::Invalid);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let params = location()?.search_params();
        for (key, expected) in [
            (rustok_ui_core::AdminQueryKey::AttributeId, _attribute_id),
            (rustok_ui_core::AdminQueryKey::OptionId, _option_id),
        ] {
            if params.get_all(key.as_str()).length() > 1
                || params.get(key.as_str()).as_deref() != expected
            {
                return Err(WriteJournalError::Changed);
            }
        }
        if _option_id.is_some() && _attribute_id.is_none() {
            return Err(WriteJournalError::Invalid);
        }
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(WriteJournalError::Unavailable)
    }
}
