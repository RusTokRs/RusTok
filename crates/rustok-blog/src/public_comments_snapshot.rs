use std::{fmt::Write as _, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_api::sha256_digest;
use rustok_core::error::ErrorKind;

use crate::{BlogError, BlogResult, CommentListItem, CommentService, ListCommentsFilter};

const SNAPSHOT_SCHEMA_VERSION: u16 = 1;
pub const MAX_PUBLIC_COMMENTS_SNAPSHOT_BYTES: usize = 256 * 1024;

#[async_trait]
pub trait PublicCommentsSnapshotStore: Send + Sync {
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    async fn store(&self, key: String, value: Vec<u8>) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicCommentsAvailability {
    Available,
    Unavailable,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct PublicCommentsRead {
    pub availability: PublicCommentsAvailability,
    pub cached_snapshot: bool,
    pub items: Vec<CommentListItem>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PublicCommentsSnapshotIdentity {
    tenant_id: Uuid,
    post_id: Uuid,
    requested_locale: String,
    fallback_locale: Option<String>,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicCommentsSnapshotEnvelope {
    schema_version: u16,
    identity: PublicCommentsSnapshotIdentity,
    items: Vec<CommentListItem>,
    total: u64,
}

pub async fn list_public_comments_with_snapshot(
    service: &CommentService,
    snapshot_store: Option<&Arc<dyn PublicCommentsSnapshotStore>>,
    tenant_id: Uuid,
    post_id: Uuid,
    requested_locale: &str,
    fallback_locale: Option<&str>,
    page: u64,
    per_page: u64,
) -> BlogResult<PublicCommentsRead> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let identity = PublicCommentsSnapshotIdentity {
        tenant_id,
        post_id,
        requested_locale: requested_locale.to_string(),
        fallback_locale: fallback_locale.map(str::to_string),
        page,
        per_page,
    };

    match service
        .list_for_post_with_locale_fallback(
            tenant_id,
            rustok_core::SecurityContext::public_read(),
            post_id,
            ListCommentsFilter {
                locale: Some(requested_locale.to_string()),
                page,
                per_page,
            },
            fallback_locale,
        )
        .await
    {
        Ok((items, total)) => {
            if let Some(store) = snapshot_store {
                store_snapshot_best_effort(store.as_ref(), &identity, &items, total).await;
            }
            Ok(PublicCommentsRead {
                availability: PublicCommentsAvailability::Available,
                cached_snapshot: false,
                items,
                total,
            })
        }
        Err(error) => {
            let Some(availability) = degraded_availability(&error) else {
                return Err(error);
            };

            if let Some(store) = snapshot_store {
                let snapshot = load_snapshot_best_effort(store.as_ref(), &identity).await;
                if let Some(snapshot) = snapshot {
                    return Ok(PublicCommentsRead {
                        availability,
                        cached_snapshot: true,
                        items: snapshot.items,
                        total: snapshot.total,
                    });
                }
            }

            Ok(PublicCommentsRead {
                availability,
                cached_snapshot: false,
                items: Vec::new(),
                total: 0,
            })
        }
    }
}

async fn store_snapshot_best_effort(
    store: &dyn PublicCommentsSnapshotStore,
    identity: &PublicCommentsSnapshotIdentity,
    items: &[CommentListItem],
    total: u64,
) {
    let envelope = PublicCommentsSnapshotEnvelope {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        identity: identity.clone(),
        items: items.to_vec(),
        total,
    };
    if !snapshot_matches(&envelope, identity) {
        tracing::warn!("Blog public comments live response failed snapshot visibility checks");
        return;
    }
    let bytes = match serde_json::to_vec(&envelope) {
        Ok(bytes) if bytes.len() <= MAX_PUBLIC_COMMENTS_SNAPSHOT_BYTES => bytes,
        Ok(bytes) => {
            tracing::warn!(
                encoded_bytes = bytes.len(),
                maximum_bytes = MAX_PUBLIC_COMMENTS_SNAPSHOT_BYTES,
                "Blog public comments snapshot exceeded the bounded payload size"
            );
            return;
        }
        Err(error) => {
            tracing::warn!(%error, "Blog public comments snapshot serialization failed");
            return;
        }
    };

    if let Err(error) = store.store(snapshot_key(identity), bytes).await {
        tracing::warn!(%error, "Blog public comments snapshot cache write failed");
    }
}

async fn load_snapshot_best_effort(
    store: &dyn PublicCommentsSnapshotStore,
    identity: &PublicCommentsSnapshotIdentity,
) -> Option<PublicCommentsSnapshotEnvelope> {
    let bytes = match store.load(snapshot_key(identity).as_str()).await {
        Ok(Some(bytes)) if bytes.len() <= MAX_PUBLIC_COMMENTS_SNAPSHOT_BYTES => bytes,
        Ok(Some(bytes)) => {
            tracing::warn!(
                encoded_bytes = bytes.len(),
                maximum_bytes = MAX_PUBLIC_COMMENTS_SNAPSHOT_BYTES,
                "Blog public comments snapshot cache payload exceeded the bounded size"
            );
            return None;
        }
        Ok(None) => return None,
        Err(error) => {
            tracing::warn!(%error, "Blog public comments snapshot cache read failed");
            return None;
        }
    };

    let snapshot: PublicCommentsSnapshotEnvelope = match serde_json::from_slice(&bytes) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(%error, "Blog public comments snapshot cache payload was invalid");
            return None;
        }
    };
    if !snapshot_matches(&snapshot, identity) {
        tracing::warn!("Blog public comments snapshot cache identity or visibility check failed");
        return None;
    }
    Some(snapshot)
}

fn snapshot_matches(
    snapshot: &PublicCommentsSnapshotEnvelope,
    identity: &PublicCommentsSnapshotIdentity,
) -> bool {
    snapshot.schema_version == SNAPSHOT_SCHEMA_VERSION
        && snapshot.identity == *identity
        && snapshot.items.len() <= identity.per_page as usize
        && snapshot.total >= snapshot.items.len() as u64
        && snapshot
            .items
            .iter()
            .all(|item| item.post_id == identity.post_id && item.status == "approved")
}

fn snapshot_key(identity: &PublicCommentsSnapshotIdentity) -> String {
    let encoded = serde_json::to_vec(identity)
        .expect("public comments snapshot identity must remain serializable");
    let digest = sha256_digest(&[b"blog-public-comments-snapshot-v1\0", encoded.as_slice()]);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("hex encoding into String cannot fail");
    }
    format!("snapshot:{hex}")
}

fn degraded_availability(error: &BlogError) -> Option<PublicCommentsAvailability> {
    let BlogError::Rich(error) = error else {
        return None;
    };
    match error.kind {
        ErrorKind::ExternalService => Some(PublicCommentsAvailability::Unavailable),
        ErrorKind::Timeout => Some(PublicCommentsAvailability::Timeout),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::error::RichError;

    fn identity(tenant_id: Uuid, post_id: Uuid, page: u64) -> PublicCommentsSnapshotIdentity {
        PublicCommentsSnapshotIdentity {
            tenant_id,
            post_id,
            requested_locale: "fr".to_string(),
            fallback_locale: Some("en".to_string()),
            page,
            per_page: 20,
        }
    }

    fn approved_item(post_id: Uuid) -> CommentListItem {
        CommentListItem {
            id: Uuid::new_v4(),
            locale: "fr".to_string(),
            effective_locale: "fr".to_string(),
            post_id,
            author_id: Some(Uuid::new_v4()),
            content_preview: "cached".to_string(),
            status: "approved".to_string(),
            parent_comment_id: None,
            created_at: "2026-08-09T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn snapshot_key_is_tenant_post_and_page_scoped() {
        let tenant_id = Uuid::new_v4();
        let post_id = Uuid::new_v4();
        assert_ne!(
            snapshot_key(&identity(tenant_id, post_id, 1)),
            snapshot_key(&identity(tenant_id, post_id, 2))
        );
        assert_ne!(
            snapshot_key(&identity(tenant_id, post_id, 1)),
            snapshot_key(&identity(Uuid::new_v4(), post_id, 1))
        );
    }

    #[test]
    fn snapshot_validation_rejects_cross_post_and_non_approved_rows() {
        let tenant_id = Uuid::new_v4();
        let post_id = Uuid::new_v4();
        let identity = identity(tenant_id, post_id, 1);
        let valid = PublicCommentsSnapshotEnvelope {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            identity: identity.clone(),
            items: vec![approved_item(post_id)],
            total: 1,
        };
        assert!(snapshot_matches(&valid, &identity));

        let mut cross_post = valid.clone();
        cross_post.items[0].post_id = Uuid::new_v4();
        assert!(!snapshot_matches(&cross_post, &identity));

        let mut pending = valid;
        pending.items[0].status = "pending".to_string();
        assert!(!snapshot_matches(&pending, &identity));
    }

    #[test]
    fn only_external_service_and_timeout_errors_degrade() {
        let unavailable = BlogError::Rich(Box::new(RichError::new(
            ErrorKind::ExternalService,
            "unavailable",
        )));
        let timeout = BlogError::Rich(Box::new(RichError::new(ErrorKind::Timeout, "timeout")));
        let validation = BlogError::Rich(Box::new(RichError::new(
            ErrorKind::Validation,
            "validation",
        )));

        assert_eq!(
            degraded_availability(&unavailable),
            Some(PublicCommentsAvailability::Unavailable)
        );
        assert_eq!(
            degraded_availability(&timeout),
            Some(PublicCommentsAvailability::Timeout)
        );
        assert_eq!(degraded_availability(&validation), None);
    }
}
