//! Advisory cross-tab notifications for confirmed page writes.
//!
//! This channel never carries document content. The server-side revision compare-and-swap remains
//! authoritative; the channel only fences stale browser sessions before they submit another edit.

use serde::{Deserialize, Serialize};

pub const FLY_PEER_WRITE_CHANNEL: &str = "fly:page-authoring-writes:v1";
pub const FLY_PEER_WRITE_CONFLICT_CODE: &str = "FLY_PEER_WRITE_CONFLICT";
const MAX_PEER_WRITE_FIELD_BYTES: usize = 256;
const MAX_PEER_WRITE_MESSAGE_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPeerWriteConflict {
    pub page_id: String,
    pub revision: String,
    pub project_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PeerWriteMessage {
    kind: String,
    page_id: String,
    revision: String,
    hash: String,
    /// Shared by every channel instance in one document, but unique across tabs.
    sender_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PeerWriteBase {
    page_id: String,
    revision: String,
    project_hash: String,
    sender_id: String,
    conflicted: bool,
}

impl PeerWriteBase {
    fn new(
        page_id: String,
        revision: String,
        project_hash: String,
        sender_id: String,
    ) -> Result<Self, String> {
        validate_field("page id", &page_id)?;
        validate_field("revision", &revision)?;
        validate_field("project hash", &project_hash)?;
        validate_field("sender id", &sender_id)?;
        Ok(Self {
            page_id,
            revision,
            project_hash,
            sender_id,
            conflicted: false,
        })
    }

    fn observe(&mut self, message: PeerWriteMessage) -> Option<BrowserPeerWriteConflict> {
        if message.kind != "confirmed-write"
            || message.page_id != self.page_id
            || self.conflicted
            || validate_field("revision", &message.revision).is_err()
            || validate_field("project hash", &message.hash).is_err()
            || validate_field("sender id", &message.sender_id).is_err()
        {
            return None;
        }

        if message.sender_id == self.sender_id {
            // The JS adapter and Rust Leptos runtime may each own a BroadcastChannel
            // in the same document. A local confirmed write advances both baselines.
            self.revision = message.revision;
            self.project_hash = message.hash;
            return None;
        }

        if message.revision == self.revision && message.hash == self.project_hash {
            return None;
        }

        self.conflicted = true;
        Some(BrowserPeerWriteConflict {
            page_id: message.page_id,
            revision: message.revision,
            project_hash: message.hash,
        })
    }

    fn advance(&mut self, revision: String, project_hash: String) -> Result<(), String> {
        if self.conflicted {
            return Err("peer write conflict already fenced this editor session".to_string());
        }
        validate_field("revision", &revision)?;
        validate_field("project hash", &project_hash)?;
        self.revision = revision;
        self.project_hash = project_hash;
        Ok(())
    }
}

fn validate_field(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > MAX_PEER_WRITE_FIELD_BYTES {
        return Err(format!(
            "peer write {name} is empty or exceeds the size limit"
        ));
    }
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser {
    use super::*;
    use crate::BrowserRuntimeError;
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{BroadcastChannel, MessageEvent};

    fn peer_write_document_id() -> Result<String, BrowserRuntimeError> {
        const KEY: &str = "__flyPeerWriteDocumentId";
        let global = js_sys::global();
        let key = JsValue::from_str(KEY);
        if let Ok(existing) = js_sys::Reflect::get(&global, &key) {
            if let Some(existing) = existing.as_string() {
                if !existing.trim().is_empty() {
                    return Ok(existing);
                }
            }
        }

        let generated = format!(
            "fly-document-{}-{}",
            js_sys::Math::random(),
            js_sys::Math::random()
        );
        let stored = js_sys::Reflect::set(&global, &key, &JsValue::from_str(&generated)).map_err(
            |error| {
                BrowserRuntimeError::Browser(format!(
                    "cannot establish shared Fly document identity: {error:?}"
                ))
            },
        )?;
        if !stored {
            return Err(BrowserRuntimeError::Browser(
                "cannot establish shared Fly document identity".to_string(),
            ));
        }
        Ok(generated)
    }

    /// A page-scoped advisory channel that fences this tab after another tab confirms a write.
    pub struct BrowserPeerWriteChannel {
        channel: BroadcastChannel,
        // Retained for the lifetime of the channel so the JS callback is not collected.
        _callback: Closure<dyn FnMut(MessageEvent)>,
        base: Rc<RefCell<PeerWriteBase>>,
    }

    impl BrowserPeerWriteChannel {
        /// Construct a channel with a unique editor-root identity in this browser document.
        /// Use `new_scoped` when another adapter (such as the JS host) must share this root.
        pub fn new(
            page_id: impl Into<String>,
            revision: impl Into<String>,
            project_hash: impl Into<String>,
            on_conflict: impl FnMut(BrowserPeerWriteConflict) + 'static,
        ) -> Result<Self, BrowserRuntimeError> {
            let scope_id = format!(
                "fly-root-{}-{}",
                js_sys::Math::random(),
                js_sys::Math::random()
            );
            Self::new_scoped(page_id, revision, project_hash, scope_id, on_conflict)
        }

        /// Construct a channel whose identity is scoped to one editor root within this document.
        /// The document nonce keeps identical SSR-generated root IDs distinct across tabs.
        pub fn new_scoped(
            page_id: impl Into<String>,
            revision: impl Into<String>,
            project_hash: impl Into<String>,
            scope_id: impl AsRef<str>,
            on_conflict: impl FnMut(BrowserPeerWriteConflict) + 'static,
        ) -> Result<Self, BrowserRuntimeError> {
            validate_field("editor scope id", scope_id.as_ref())
                .map_err(BrowserRuntimeError::Serialization)?;
            let document_id = peer_write_document_id()?;
            let sender_id = format!("{document_id}:{}", scope_id.as_ref());
            Self::new_with_sender_id(page_id, revision, project_hash, sender_id, on_conflict)
        }

        /// Construct a channel that shares a logical editor identity with another adapter.
        pub fn new_with_sender_id(
            page_id: impl Into<String>,
            revision: impl Into<String>,
            project_hash: impl Into<String>,
            sender_id: impl Into<String>,
            mut on_conflict: impl FnMut(BrowserPeerWriteConflict) + 'static,
        ) -> Result<Self, BrowserRuntimeError> {
            let base = Rc::new(RefCell::new(
                PeerWriteBase::new(
                    page_id.into(),
                    revision.into(),
                    project_hash.into(),
                    sender_id.into(),
                )
                .map_err(BrowserRuntimeError::Serialization)?,
            ));
            let channel = BroadcastChannel::new(FLY_PEER_WRITE_CHANNEL).map_err(|error| {
                BrowserRuntimeError::Browser(format!(
                    "cannot open Fly peer-write channel: {error:?}"
                ))
            })?;

            let callback_base = Rc::clone(&base);
            let callback = Closure::<dyn FnMut(MessageEvent)>::new(move |event| {
                let data = event.data();
                // Both producers send structured objects, not serialized JSON strings.
                if !data.is_object() || js_sys::Array::is_array(&data) {
                    return;
                }
                let serialized = js_sys::JSON::stringify(&data)
                    .ok()
                    .and_then(|value| value.as_string());
                let Some(serialized) = serialized else {
                    return;
                };
                if serialized.len() > MAX_PEER_WRITE_MESSAGE_BYTES {
                    return;
                }
                let Ok(message) = serde_json::from_str::<PeerWriteMessage>(&serialized) else {
                    return;
                };
                let conflict = callback_base.borrow_mut().observe(message);
                if let Some(conflict) = conflict {
                    on_conflict(conflict);
                }
            });
            channel.set_onmessage(Some(callback.as_ref().unchecked_ref()));

            Ok(Self {
                channel,
                _callback: callback,
                base,
            })
        }

        /// Broadcast a confirmed server write and advance this tab's comparison baseline.
        ///
        /// The baseline is advanced only after postMessage succeeds. A fenced session cannot
        /// announce another write until it reloads the authoritative document.
        pub fn post_confirmed_write(
            &self,
            revision: &str,
            project_hash: &str,
        ) -> Result<(), BrowserRuntimeError> {
            validate_field("revision", revision)
                .and_then(|_| validate_field("project hash", project_hash))
                .map_err(BrowserRuntimeError::Serialization)?;

            let message = {
                let base = self.base.borrow();
                if base.conflicted {
                    return Err(BrowserRuntimeError::Browser(
                        "peer write conflict already fenced this editor session".to_string(),
                    ));
                }
                PeerWriteMessage {
                    kind: "confirmed-write".to_string(),
                    page_id: base.page_id.clone(),
                    revision: revision.to_string(),
                    hash: project_hash.to_string(),
                    sender_id: base.sender_id.clone(),
                }
            };

            let serialized = serde_json::to_string(&message)
                .map_err(|error| BrowserRuntimeError::Serialization(error.to_string()))?;
            if serialized.len() > MAX_PEER_WRITE_MESSAGE_BYTES {
                return Err(BrowserRuntimeError::Serialization(
                    "peer write message exceeds the size limit".to_string(),
                ));
            }
            let value = js_sys::JSON::parse(&serialized).map_err(|error| {
                BrowserRuntimeError::Browser(format!(
                    "cannot encode Fly peer-write message: {error:?}"
                ))
            })?;
            self.channel.post_message(&value).map_err(|error| {
                BrowserRuntimeError::Browser(format!(
                    "cannot broadcast Fly peer-write message: {error:?}"
                ))
            })?;

            self.base
                .borrow_mut()
                .advance(revision.to_string(), project_hash.to_string())
                .map_err(BrowserRuntimeError::Serialization)
        }
    }

    impl Drop for BrowserPeerWriteChannel {
        fn drop(&mut self) {
            self.channel.set_onmessage(None);
            self.channel.close();
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser::BrowserPeerWriteChannel;

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PeerWriteBase {
        PeerWriteBase::new(
            "page-1".to_string(),
            "revision-1".to_string(),
            "hash-1".to_string(),
            "document-a".to_string(),
        )
        .expect("valid base")
    }

    #[test]
    fn message_matches_existing_browser_adapter_wire_contract() {
        let message = PeerWriteMessage {
            kind: "confirmed-write".to_string(),
            page_id: "page-1".to_string(),
            revision: "revision-2".to_string(),
            hash: "hash-2".to_string(),
            sender_id: "document-a".to_string(),
        };
        assert_eq!(
            serde_json::to_value(message).expect("serialize message"),
            serde_json::json!({
                "kind": "confirmed-write",
                "pageId": "page-1",
                "revision": "revision-2",
                "hash": "hash-2",
                "senderId": "document-a"
            })
        );
    }

    #[test]
    fn ignores_other_pages_and_identical_confirmed_snapshots() {
        let mut base = base();
        assert!(
            base.observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-2".to_string(),
                revision: "revision-2".to_string(),
                hash: "hash-2".to_string(),
                sender_id: "document-b".to_string(),
            })
            .is_none()
        );
        assert!(
            base.observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-1".to_string(),
                revision: "revision-1".to_string(),
                hash: "hash-1".to_string(),
                sender_id: "document-b".to_string(),
            })
            .is_none()
        );
        assert!(!base.conflicted);
    }

    #[test]
    fn same_document_channel_advances_baseline_without_fencing() {
        let mut base = base();
        assert!(
            base.observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-1".to_string(),
                revision: "revision-2".to_string(),
                hash: "hash-2".to_string(),
                sender_id: "document-a".to_string(),
            })
            .is_none()
        );
        assert_eq!(base.revision, "revision-2");
        assert_eq!(base.project_hash, "hash-2");
        assert!(!base.conflicted);
    }

    #[test]
    fn another_editor_root_in_the_same_document_is_still_a_peer() {
        let mut base = PeerWriteBase::new(
            "page-1".to_string(),
            "revision-1".to_string(),
            "hash-1".to_string(),
            "document-a:root-1".to_string(),
        )
        .expect("valid base");
        let conflict = base
            .observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-1".to_string(),
                revision: "revision-2".to_string(),
                hash: "hash-2".to_string(),
                sender_id: "document-a:root-2".to_string(),
            })
            .expect("different editor root must be fenced");
        assert_eq!(conflict.revision, "revision-2");
    }

    #[test]
    fn fences_once_when_another_tab_confirms_a_new_revision_or_hash() {
        let mut base = base();
        let conflict = base
            .observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-1".to_string(),
                revision: "revision-2".to_string(),
                hash: "hash-2".to_string(),
                sender_id: "document-b".to_string(),
            })
            .expect("new revision must fence the stale editor");
        assert_eq!(conflict.revision, "revision-2");
        assert!(base.conflicted);
        assert!(
            base.observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-1".to_string(),
                revision: "revision-3".to_string(),
                hash: "hash-3".to_string(),
                sender_id: "document-b".to_string(),
            })
            .is_none()
        );
    }

    #[test]
    fn malformed_sender_cannot_fence_or_advance_baseline() {
        let mut base = base();
        for sender_id in [
            String::new(),
            " \u{85}".to_string(),
            "é".repeat(MAX_PEER_WRITE_FIELD_BYTES / 2 + 1),
        ] {
            assert!(
                base.observe(PeerWriteMessage {
                    kind: "confirmed-write".to_string(),
                    page_id: "page-1".to_string(),
                    revision: "revision-2".to_string(),
                    hash: "hash-2".to_string(),
                    sender_id,
                })
                .is_none()
            );
            assert!(!base.conflicted);
            assert_eq!(base.revision, "revision-1");
            assert_eq!(base.project_hash, "hash-1");
        }
        assert!(
            serde_json::from_value::<PeerWriteMessage>(serde_json::json!({
                "kind": "confirmed-write",
                "pageId": "page-1",
                "revision": "revision-2",
                "hash": "hash-2"
            }))
            .is_err()
        );
        assert!(
            base.observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-1".to_string(),
                revision: "revision-2".to_string(),
                hash: "hash-2".to_string(),
                sender_id: "document-b".to_string(),
            })
            .is_some()
        );
    }

    #[test]
    fn rejects_invalid_baselines_and_messages() {
        assert!(
            PeerWriteBase::new(
                "".to_string(),
                "r".to_string(),
                "h".to_string(),
                "document-a".to_string(),
            )
            .is_err()
        );
        let mut base = base();
        assert!(
            base.observe(PeerWriteMessage {
                kind: "confirmed-write".to_string(),
                page_id: "page-1".to_string(),
                revision: String::new(),
                hash: "hash-2".to_string(),
                sender_id: "document-b".to_string(),
            })
            .is_none()
        );
        assert!(base.advance("".to_string(), "hash-2".to_string()).is_err());
    }
}
