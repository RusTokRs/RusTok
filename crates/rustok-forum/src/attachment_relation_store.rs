use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait, sea_query::OnConflict,
};
use sha2::{Digest, Sha256};

use crate::{
    ForumError, ForumResult,
    attachment_relation::{
        ForumAttachmentSourceRevision, ForumAttachmentUsage,
        ForumMediaAdmittedAttachmentRelationBatch, ForumPreparedAttachmentRelation,
        MAX_FORUM_ATTACHMENT_CAPTION_BYTES, MAX_FORUM_ATTACHMENTS_PER_REVISION,
    },
    entities::{forum_attachment_relation, forum_attachment_relation_revision},
    mentions::ForumContentTargetKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumAttachmentRelationPersistenceOutcome {
    Persisted,
    Replayed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumPersistedAttachmentRelationProjection {
    source: ForumAttachmentSourceRevision,
    attachments: Vec<ForumPreparedAttachmentRelation>,
    projection_fingerprint: String,
}

impl ForumPersistedAttachmentRelationProjection {
    pub fn source(&self) -> &ForumAttachmentSourceRevision {
        &self.source
    }

    pub fn attachments(&self) -> &[ForumPreparedAttachmentRelation] {
        &self.attachments
    }

    pub fn projection_fingerprint(&self) -> &str {
        &self.projection_fingerprint
    }
}

/// Durable Forum-owned store for revision-scoped attachment projections.
///
/// The write entrypoint deliberately accepts only a Media-admitted batch. The
/// store never asks Media for lifecycle state and never persists Media-owned
/// fields; its only cross-module datum is the typed Media UUID admitted before
/// this boundary.
pub struct ForumAttachmentRelationStore {
    db: DatabaseConnection,
}

impl ForumAttachmentRelationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn persist(
        &self,
        batch: &ForumMediaAdmittedAttachmentRelationBatch,
    ) -> ForumResult<ForumAttachmentRelationPersistenceOutcome> {
        let identity = DbAttachmentRevisionIdentity::from_source(batch.source())?;
        let projection_fingerprint =
            projection_fingerprint(batch.source(), batch.attachments());
        let txn = self.db.begin().await?;

        let inserted_headers = forum_attachment_relation_revision::Entity::insert(
            forum_attachment_relation_revision::ActiveModel {
                tenant_id: Set(identity.tenant_id),
                target_kind: Set(identity.target_kind.clone()),
                target_id: Set(identity.target_id),
                source_revision: Set(identity.source_revision),
                locale: Set(identity.locale.clone()),
                projection_fingerprint: Set(projection_fingerprint.clone()),
                created_at: Set(Utc::now().into()),
            },
        )
        .on_conflict(
            OnConflict::columns([
                forum_attachment_relation_revision::Column::TenantId,
                forum_attachment_relation_revision::Column::TargetKind,
                forum_attachment_relation_revision::Column::TargetId,
                forum_attachment_relation_revision::Column::SourceRevision,
                forum_attachment_relation_revision::Column::Locale,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&txn)
        .await?;

        match inserted_headers {
            1 => {
                if !batch.attachments().is_empty() {
                    let rows = batch
                        .attachments()
                        .iter()
                        .map(|relation| forum_attachment_relation::ActiveModel {
                            tenant_id: Set(identity.tenant_id),
                            target_kind: Set(identity.target_kind.clone()),
                            target_id: Set(identity.target_id),
                            source_revision: Set(identity.source_revision),
                            locale: Set(identity.locale.clone()),
                            position: Set(i32::from(relation.position)),
                            media_id: Set(relation.media_id),
                            usage: Set(usage_to_storage(relation.usage).to_string()),
                            caption: Set(relation.caption.clone()),
                            created_at: Set(Utc::now().into()),
                        })
                        .collect::<Vec<_>>();
                    let inserted_rows = forum_attachment_relation::Entity::insert_many(rows)
                        .exec_without_returning(&txn)
                        .await?;
                    let expected_rows = u64::try_from(batch.attachments().len()).map_err(|_| {
                        attachment_projection_integrity_error(
                            "attachment projection row count exceeds database range",
                        )
                    })?;
                    if inserted_rows != expected_rows {
                        return Err(attachment_projection_integrity_error(
                            "attachment projection insert cardinality mismatch",
                        ));
                    }
                }
                txn.commit().await?;
                Ok(ForumAttachmentRelationPersistenceOutcome::Persisted)
            }
            0 => {
                let existing = load_revision_header(&txn, &identity)
                    .await?
                    .ok_or_else(|| {
                        attachment_projection_integrity_error(
                            "attachment projection conflict did not expose a revision header",
                        )
                    })?;
                if existing.projection_fingerprint != projection_fingerprint {
                    return Err(attachment_projection_conflict());
                }

                let rows = load_relation_rows(&txn, &identity).await?;
                let stored_attachments = decode_relation_rows(rows)?;
                if stored_attachments != batch.attachments() {
                    return Err(attachment_projection_integrity_error(
                        "attachment projection fingerprint matched but stored rows differed",
                    ));
                }

                txn.commit().await?;
                Ok(ForumAttachmentRelationPersistenceOutcome::Replayed)
            }
            _ => Err(attachment_projection_integrity_error(
                "attachment projection header insert affected more than one row",
            )),
        }
    }

    pub async fn load(
        &self,
        source: &ForumAttachmentSourceRevision,
    ) -> ForumResult<Option<ForumPersistedAttachmentRelationProjection>> {
        let identity = DbAttachmentRevisionIdentity::from_source(source)?;
        let Some(header) = load_revision_header(&self.db, &identity).await? else {
            return Ok(None);
        };
        let rows = load_relation_rows(&self.db, &identity).await?;
        let attachments = decode_relation_rows(rows)?;
        let actual_fingerprint = projection_fingerprint(source, &attachments);
        if actual_fingerprint != header.projection_fingerprint {
            return Err(attachment_projection_integrity_error(
                "attachment projection fingerprint does not match stored rows",
            ));
        }

        Ok(Some(ForumPersistedAttachmentRelationProjection {
            source: source.clone(),
            attachments,
            projection_fingerprint: header.projection_fingerprint,
        }))
    }
}

#[derive(Debug, Clone)]
struct DbAttachmentRevisionIdentity {
    tenant_id: uuid::Uuid,
    target_kind: String,
    target_id: uuid::Uuid,
    source_revision: i64,
    locale: String,
}

impl DbAttachmentRevisionIdentity {
    fn from_source(source: &ForumAttachmentSourceRevision) -> ForumResult<Self> {
        let source_revision = i64::try_from(source.source_revision()).map_err(|_| {
            ForumError::Validation(
                "Forum attachment source revision exceeds durable storage range".to_string(),
            )
        })?;
        if source.locale().len() > 32 {
            return Err(ForumError::Validation(
                "Forum attachment locale exceeds durable storage width".to_string(),
            ));
        }
        Ok(Self {
            tenant_id: source.tenant_id(),
            target_kind: target_kind_to_storage(source.target().kind()).to_string(),
            target_id: source.target().id(),
            source_revision,
            locale: source.locale().to_string(),
        })
    }
}

async fn load_revision_header<C>(
    db: &C,
    identity: &DbAttachmentRevisionIdentity,
) -> Result<Option<forum_attachment_relation_revision::Model>, sea_orm::DbErr>
where
    C: sea_orm::ConnectionTrait,
{
    forum_attachment_relation_revision::Entity::find()
        .filter(
            forum_attachment_relation_revision::Column::TenantId.eq(identity.tenant_id),
        )
        .filter(
            forum_attachment_relation_revision::Column::TargetKind
                .eq(identity.target_kind.clone()),
        )
        .filter(
            forum_attachment_relation_revision::Column::TargetId.eq(identity.target_id),
        )
        .filter(
            forum_attachment_relation_revision::Column::SourceRevision
                .eq(identity.source_revision),
        )
        .filter(
            forum_attachment_relation_revision::Column::Locale.eq(identity.locale.clone()),
        )
        .one(db)
        .await
}

async fn load_relation_rows<C>(
    db: &C,
    identity: &DbAttachmentRevisionIdentity,
) -> Result<Vec<forum_attachment_relation::Model>, sea_orm::DbErr>
where
    C: sea_orm::ConnectionTrait,
{
    forum_attachment_relation::Entity::find()
        .filter(forum_attachment_relation::Column::TenantId.eq(identity.tenant_id))
        .filter(
            forum_attachment_relation::Column::TargetKind.eq(identity.target_kind.clone()),
        )
        .filter(forum_attachment_relation::Column::TargetId.eq(identity.target_id))
        .filter(
            forum_attachment_relation::Column::SourceRevision.eq(identity.source_revision),
        )
        .filter(forum_attachment_relation::Column::Locale.eq(identity.locale.clone()))
        .order_by_asc(forum_attachment_relation::Column::Position)
        .all(db)
        .await
}

fn decode_relation_rows(
    rows: Vec<forum_attachment_relation::Model>,
) -> ForumResult<Vec<ForumPreparedAttachmentRelation>> {
    if rows.len() > MAX_FORUM_ATTACHMENTS_PER_REVISION {
        return Err(attachment_projection_integrity_error(
            "attachment projection exceeds the bounded relation count",
        ));
    }

    rows.into_iter()
        .enumerate()
        .map(|(expected_position, row)| {
            let position = u16::try_from(row.position).map_err(|_| {
                attachment_projection_integrity_error(
                    "attachment projection contains an invalid persisted position",
                )
            })?;
            if usize::from(position) != expected_position {
                return Err(attachment_projection_integrity_error(
                    "attachment projection persisted positions are not contiguous",
                ));
            }
            if row.media_id.is_nil() {
                return Err(attachment_projection_integrity_error(
                    "attachment projection contains a nil Media identity",
                ));
            }
            let usage = usage_from_storage(&row.usage).ok_or_else(|| {
                attachment_projection_integrity_error(
                    "attachment projection contains an unknown usage value",
                )
            })?;
            if let Some(caption) = row.caption.as_deref() {
                if caption.is_empty()
                    || caption.trim() != caption
                    || caption.len() > MAX_FORUM_ATTACHMENT_CAPTION_BYTES
                    || caption.chars().any(char::is_control)
                {
                    return Err(attachment_projection_integrity_error(
                        "attachment projection contains a non-normalized caption",
                    ));
                }
            }
            Ok(ForumPreparedAttachmentRelation {
                media_id: row.media_id,
                usage,
                position,
                caption: row.caption,
            })
        })
        .collect()
}

fn projection_fingerprint(
    source: &ForumAttachmentSourceRevision,
    attachments: &[ForumPreparedAttachmentRelation],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok-forum/attachment-projection/v1\0");
    hasher.update(source.tenant_id().as_bytes());
    hasher.update([target_kind_fingerprint_byte(source.target().kind())]);
    hasher.update(source.target().id().as_bytes());
    hasher.update(source.source_revision().to_be_bytes());
    hash_len_prefixed(&mut hasher, source.locale().as_bytes());
    hasher.update(
        u64::try_from(attachments.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );

    for relation in attachments {
        hasher.update(relation.position.to_be_bytes());
        hasher.update(relation.media_id.as_bytes());
        hasher.update([usage_fingerprint_byte(relation.usage)]);
        match relation.caption.as_deref() {
            Some(caption) => {
                hasher.update([1_u8]);
                hash_len_prefixed(&mut hasher, caption.as_bytes());
            }
            None => hasher.update([0_u8]),
        }
    }

    hex::encode(hasher.finalize())
}

fn hash_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        u64::try_from(value.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(value);
}

fn target_kind_to_storage(kind: ForumContentTargetKind) -> &'static str {
    match kind {
        ForumContentTargetKind::Topic => "topic",
        ForumContentTargetKind::Reply => "reply",
    }
}

fn target_kind_fingerprint_byte(kind: ForumContentTargetKind) -> u8 {
    match kind {
        ForumContentTargetKind::Topic => 1,
        ForumContentTargetKind::Reply => 2,
    }
}

fn usage_to_storage(usage: ForumAttachmentUsage) -> &'static str {
    match usage {
        ForumAttachmentUsage::Inline => "inline",
        ForumAttachmentUsage::Attachment => "attachment",
    }
}

fn usage_from_storage(value: &str) -> Option<ForumAttachmentUsage> {
    match value {
        "inline" => Some(ForumAttachmentUsage::Inline),
        "attachment" => Some(ForumAttachmentUsage::Attachment),
        _ => None,
    }
}

fn usage_fingerprint_byte(usage: ForumAttachmentUsage) -> u8 {
    match usage {
        ForumAttachmentUsage::Inline => 1,
        ForumAttachmentUsage::Attachment => 2,
    }
}

fn attachment_projection_conflict() -> ForumError {
    ForumError::Validation(
        "Forum attachment replay changed an existing immutable revision projection".to_string(),
    )
}

fn attachment_projection_integrity_error(message: &str) -> ForumError {
    ForumError::Database(sea_orm::DbErr::Custom(format!(
        "Forum attachment projection integrity failure: {message}"
    )))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;
    use rustok_api::{PortActor, PortContext, PortError};
    use rustok_media::{MediaReferenceAdmission, MediaReferenceAdmissionPort};
    use sea_orm::Database;
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    use super::*;
    use crate::{
        ForumAttachmentRelationAdmissionRequest, ForumAttachmentRelationInput,
        ForumAttachmentRelationPreparer, ForumContentTarget,
        admit_attachment_relations_for_persistence,
    };

    struct AllowMedia {
        tenant_id: Uuid,
    }

    #[async_trait]
    impl MediaReferenceAdmissionPort for AllowMedia {
        async fn admit_references(
            &self,
            _context: PortContext,
            media_ids: Vec<Uuid>,
        ) -> Result<Vec<MediaReferenceAdmission>, PortError> {
            Ok(media_ids
                .into_iter()
                .map(|media_id| MediaReferenceAdmission {
                    media_id,
                    tenant_id: self.tenant_id,
                    referenceable: true,
                })
                .collect())
        }
    }

    async fn setup() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("attachment relation store test database should connect");
        let manager = SchemaManager::new(&db);
        let migration = crate::migrations::migrations()
            .into_iter()
            .find(|migration| migration.name() == "m20260812_000028_add_forum_attachment_relations")
            .expect("FORUM-14 attachment migration should be registered");
        migration
            .up(&manager)
            .await
            .expect("FORUM-14 attachment migration should apply");
        db
    }

    async fn admitted_batch(
        tenant_id: Uuid,
        target: ForumContentTarget,
        source_revision: u64,
        locale: &str,
        media_ids: &[Uuid],
    ) -> ForumMediaAdmittedAttachmentRelationBatch {
        let prepared = ForumAttachmentRelationPreparer
            .prepare(ForumAttachmentRelationAdmissionRequest {
                tenant_id,
                target,
                source_revision,
                locale: locale.to_string(),
                attachments: media_ids
                    .iter()
                    .enumerate()
                    .map(|(position, media_id)| ForumAttachmentRelationInput {
                        media_id: *media_id,
                        usage: if position == 0 {
                            ForumAttachmentUsage::Inline
                        } else {
                            ForumAttachmentUsage::Attachment
                        },
                        position: u16::try_from(position)
                            .expect("test attachment position should fit u16"),
                        caption: (position == 1).then(|| "secondary asset".to_string()),
                    })
                    .collect(),
            })
            .expect("attachment relation batch should prepare");
        let media = AllowMedia { tenant_id };
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::system(),
            locale,
            "forum-attachment-store-test",
        )
        .with_deadline(Duration::from_secs(1));
        admit_attachment_relations_for_persistence(Some(&media), context, prepared)
            .await
            .expect("Media owner admission should succeed")
    }

    #[tokio::test]
    async fn persists_reads_and_replays_the_exact_admitted_projection() {
        let db = setup().await;
        let store = ForumAttachmentRelationStore::new(db);
        let tenant_id = Uuid::new_v4();
        let target = ForumContentTarget::topic(Uuid::new_v4());
        let media_ids = [Uuid::new_v4(), Uuid::new_v4()];
        let batch = admitted_batch(tenant_id, target, 7, "en-US", &media_ids).await;

        assert_eq!(
            store.persist(&batch).await.expect("first persist"),
            ForumAttachmentRelationPersistenceOutcome::Persisted
        );
        let loaded = store
            .load(batch.source())
            .await
            .expect("readback should succeed")
            .expect("persisted projection should exist");
        assert_eq!(loaded.source(), batch.source());
        assert_eq!(loaded.attachments(), batch.attachments());
        assert_eq!(loaded.projection_fingerprint().len(), 64);

        assert_eq!(
            store.persist(&batch).await.expect("exact replay"),
            ForumAttachmentRelationPersistenceOutcome::Replayed
        );
    }

    #[tokio::test]
    async fn conflicting_replay_of_the_same_revision_fails_closed() {
        let db = setup().await;
        let store = ForumAttachmentRelationStore::new(db);
        let tenant_id = Uuid::new_v4();
        let target = ForumContentTarget::reply(Uuid::new_v4());
        let first = admitted_batch(tenant_id, target, 3, "fr", &[Uuid::new_v4()]).await;
        let changed = admitted_batch(tenant_id, target, 3, "fr", &[Uuid::new_v4()]).await;

        store.persist(&first).await.expect("first persist");
        let error = store
            .persist(&changed)
            .await
            .expect_err("changed replay must conflict");
        assert!(matches!(error, ForumError::Validation(_)));
    }

    #[tokio::test]
    async fn empty_attachment_snapshot_is_durable_without_media() {
        let db = setup().await;
        let store = ForumAttachmentRelationStore::new(db);
        let tenant_id = Uuid::new_v4();
        let prepared = ForumAttachmentRelationPreparer
            .prepare(ForumAttachmentRelationAdmissionRequest {
                tenant_id,
                target: ForumContentTarget::topic(Uuid::new_v4()),
                source_revision: 11,
                locale: "de".to_string(),
                attachments: Vec::new(),
            })
            .expect("empty relation batch should prepare");
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::system(),
            "de",
            "forum-empty-attachment-store-test",
        );
        let batch = admit_attachment_relations_for_persistence(None, context, prepared)
            .await
            .expect("empty batch should not require Media");

        assert_eq!(
            store.persist(&batch).await.expect("empty persist"),
            ForumAttachmentRelationPersistenceOutcome::Persisted
        );
        let loaded = store
            .load(batch.source())
            .await
            .expect("empty readback")
            .expect("empty snapshot header should exist");
        assert!(loaded.attachments().is_empty());
        assert_eq!(
            store.persist(&batch).await.expect("empty replay"),
            ForumAttachmentRelationPersistenceOutcome::Replayed
        );
    }
}
