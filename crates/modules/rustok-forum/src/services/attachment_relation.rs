use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::Utc;
use rustok_api::{PortContext, PortError};
use rustok_media::{MediaAssetReferenceInput, MediaAssetWritePort};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use sha2::{Digest, Sha256};
use tracing::{error, instrument};
use uuid::Uuid;

use crate::attachment_relation::{
    ForumAttachmentRelationAdmissionError, ForumAttachmentRelationAdmissionRequest,
    ForumAttachmentRelationPreparer, ForumAttachmentRelationRecord,
    ForumAttachmentRelationRevision, ForumAttachmentRelationSet, ForumAttachmentRelationUsage,
    ForumContentTarget, ForumContentTargetKind,
};
use crate::entities::{
    forum_attachment_relation, forum_attachment_relation_head, forum_reply, forum_topic,
};
use crate::error::{ForumError, ForumResult};

const FORUM_MEDIA_OWNER_MODULE: &str = "forum";
const FORUM_ATTACHMENT_REFERENCE_PREFIX: &str = "rustok:forum:attachment-reference";

pub struct ForumAttachmentRelationService {
    db: DatabaseConnection,
    media: Arc<dyn MediaAssetWritePort>,
    preparer: ForumAttachmentRelationPreparer,
}

impl ForumAttachmentRelationService {
    pub fn new(db: DatabaseConnection, media: Arc<dyn MediaAssetWritePort>) -> Self {
        Self {
            db,
            media,
            preparer: ForumAttachmentRelationPreparer,
        }
    }

    #[instrument(skip(self))]
    pub async fn get_attachment_relations(
        &self,
        tenant_id: Uuid,
        target: ForumContentTarget,
        locale: &str,
    ) -> ForumResult<ForumAttachmentRelationSet> {
        validate_context_target(tenant_id, target)?;
        let locale = normalize_locale(locale)?;
        let txn = self.db.begin().await?;
        lock_forum_target(&txn, tenant_id, target).await?;
        let head = self.load_head(&txn, tenant_id, target, &locale).await?;
        let result = match head {
            Some(head) => self.load_relation_set_from_head(&txn, head).await,
            None => Ok(empty_relation_set(tenant_id, target, locale)),
        };
        match txn.commit().await {
            Ok(()) => result,
            Err(error) => Err(ForumError::Database(error)),
        }
    }

    #[instrument(skip(self, context, request), fields(target_id = %request.target.id()))]
    pub async fn replace_attachment_relations(
        &self,
        context: PortContext,
        request: ForumAttachmentRelationAdmissionRequest,
    ) -> ForumResult<ForumAttachmentRelationSet> {
        let tenant_id = parse_context_tenant(&context)?;
        if tenant_id != request.tenant_id {
            return Err(ForumError::Validation(
                "Forum attachment relation tenant does not match the trusted port context"
                    .to_string(),
            ));
        }

        let batch = self
            .preparer
            .prepare(request)
            .map_err(attachment_admission_to_forum_error)?;
        let desired = batch.attachments();
        let target = batch.source().target();
        let locale = batch.source().locale().to_string();

        let observed = self.load_head(&self.db, tenant_id, target, &locale).await?;
        if let Some(head) = observed {
            let current = self
                .load_relation_set_from_head(&self.db, head)
                .await?;
            if batch.expected_relation_revision() != current.relation_revision
                && same_requested_state(&batch, &current)
            {
                self.ensure_media_holds(&context, &current.attachments).await?;
                return Ok(current);
            }
            batch
                .expected_relation_revision()
                .check_expected(Some(current.relation_revision))
                .map_err(map_relation_revision_error)?;
        } else {
            batch
                .expected_relation_revision()
                .check_expected(None)
                .map_err(map_relation_revision_error)?;
        }

        let new_references = desired
            .iter()
            .map(|relation| {
                (
                    relation.media_id,
                    forum_attachment_reference_id(
                        tenant_id,
                        target,
                        &locale,
                        relation.position,
                        relation.media_id,
                    ),
                )
            })
            .collect::<Vec<_>>();

        for (media_id, reference_id) in &new_references {
            self.retain_media_reference(&context, *media_id, *reference_id)
                .await?;
        }

        let txn = self.db.begin().await?;
        lock_forum_target(&txn, tenant_id, target).await?;

        let (head, created_head) =
            match self.load_head(&txn, tenant_id, target, &locale).await? {
                Some(head) => (head, false),
                None if batch.expected_relation_revision().is_empty() => {
                    insert_head(
                        &txn,
                        tenant_id,
                        target,
                        &locale,
                        ForumAttachmentRelationRevision::FIRST,
                        batch.source().source_revision(),
                    )
                    .await?;
                    (
                        self.load_head(&txn, tenant_id, target, &locale)
                            .await?
                            .ok_or(ForumError::AttachmentRelationInvariant)?,
                        true,
                    )
                }
                None => {
                    txn.rollback().await?;
                    return Err(ForumError::RelationRevisionConflict);
                }
            };

        let current = self
            .load_relation_set_from_head(&txn, head.clone())
            .await?;

        if batch.expected_relation_revision() != current.relation_revision {
            if same_requested_state(&batch, &current) {
                txn.rollback().await?;
                self.ensure_media_holds(&context, &current.attachments).await?;
                return Ok(current);
            }
            txn.rollback().await?;
            return Err(ForumError::RelationRevisionConflict);
        }

        let next_revision = if created_head {
            ForumAttachmentRelationRevision::FIRST
        } else {
            current
                .relation_revision
                .next()
                .map_err(|_| ForumError::AttachmentRelationRevisionExhausted)?
        };

        let old_references = current
            .attachments
            .iter()
            .map(|relation| (relation.reference_id, relation.media_id))
            .collect::<Vec<_>>();

        update_head(
            &txn,
            &head,
            next_revision,
            batch.source().source_revision(),
        )
        .await?;
        delete_relations_for_head(&txn, &head).await?;
        insert_relations_for_head(
            &txn,
            tenant_id,
            target,
            &locale,
            desired,
        )
        .await?;

        let committed_set = ForumAttachmentRelationSet {
            tenant_id,
            target,
            locale: locale.clone(),
            relation_revision: next_revision,
            source_revision: Some(batch.source().source_revision()),
            attachments: desired
                .iter()
                .zip(new_references.iter())
                .map(|(relation, (_, reference_id))| ForumAttachmentRelationRecord {
                    reference_id: *reference_id,
                    media_id: relation.media_id,
                    usage: relation.usage,
                    position: relation.position,
                    caption: relation.caption.clone(),
                })
                .collect(),
        };

        if let Err(db_error) = txn.commit().await {
            error!(
                error = %db_error,
                tenant_id = %tenant_id,
                target_id = %target.id(),
                "Forum attachment relation commit returned an ambiguous outcome; retained Media holds are intentionally preserved"
            );
            return Err(ForumError::Database(db_error));
        }

        let desired_refs = committed_set
            .attachments
            .iter()
            .map(|relation| relation.reference_id)
            .collect::<BTreeSet<_>>();

        for (reference_id, media_id) in old_references {
            if desired_refs.contains(&reference_id) {
                continue;
            }
            if let Err(error) = self
                .release_media_reference(&context, media_id, reference_id)
                .await
            {
                error!(
                    error = %error,
                    reference_id = %reference_id,
                    media_id = %media_id,
                    "Forum attachment relation committed but removed Media reference could not be released; conservative hold remains"
                );
            }
        }

        Ok(committed_set)
    }

    async fn retain_media_reference(
        &self,
        context: &PortContext,
        media_id: Uuid,
        reference_id: Uuid,
    ) -> ForumResult<()> {
        let media_context = context.clone().with_idempotency_key(format!(
            "{FORUM_ATTACHMENT_REFERENCE_PREFIX}:retain:{reference_id}"
        ));
        self.media
            .retain_asset_reference(
                media_context,
                media_id,
                MediaAssetReferenceInput {
                    owner_module: FORUM_MEDIA_OWNER_MODULE.to_string(),
                    reference_id,
                },
            )
            .await
            .map(|_| ())
            .map_err(media_port_error)
    }

    async fn release_media_reference(
        &self,
        context: &PortContext,
        media_id: Uuid,
        reference_id: Uuid,
    ) -> ForumResult<()> {
        let media_context = context.clone().with_idempotency_key(format!(
            "{FORUM_ATTACHMENT_REFERENCE_PREFIX}:release:{reference_id}"
        ));
        self.media
            .release_asset_reference(
                media_context,
                media_id,
                MediaAssetReferenceInput {
                    owner_module: FORUM_MEDIA_OWNER_MODULE.to_string(),
                    reference_id,
                },
            )
            .await
            .map_err(media_port_error)
    }

    async fn ensure_media_holds(
        &self,
        context: &PortContext,
        attachments: &[ForumAttachmentRelationRecord],
    ) -> ForumResult<()> {
        for relation in attachments {
            self.retain_media_reference(context, relation.media_id, relation.reference_id)
                .await?;
        }
        Ok(())
    }

    async fn load_head<C: ConnectionTrait>(
        &self,
        connection: &C,
        tenant_id: Uuid,
        target: ForumContentTarget,
        locale: &str,
    ) -> ForumResult<Option<forum_attachment_relation_head::Model>> {
        Ok(forum_attachment_relation_head::Entity::find()
            .filter(forum_attachment_relation_head::Column::TenantId.eq(tenant_id))
            .filter(
                forum_attachment_relation_head::Column::TargetKind
                    .eq(target_kind_value(target.kind())),
            )
            .filter(forum_attachment_relation_head::Column::TargetId.eq(target.id()))
            .filter(forum_attachment_relation_head::Column::Locale.eq(locale))
            .one(connection)
            .await?)
    }

    async fn load_relation_set_from_head<C: ConnectionTrait>(
        &self,
        connection: &C,
        head: forum_attachment_relation_head::Model,
    ) -> ForumResult<ForumAttachmentRelationSet> {
        let relation_revision = ForumAttachmentRelationRevision::new(head.relation_revision)
            .map_err(|_| ForumError::AttachmentRelationInvariant)?;
        let source_revision = u64::try_from(head.source_revision)
            .ok()
            .filter(|value| *value > 0)
            .ok_or(ForumError::AttachmentRelationInvariant)?;
        let target = match parse_target_kind(&head.target_kind)? {
            ForumContentTargetKind::Topic => ForumContentTarget::topic(head.target_id),
            ForumContentTargetKind::Reply => ForumContentTarget::reply(head.target_id),
        };

        let rows = forum_attachment_relation::Entity::find()
            .filter(forum_attachment_relation::Column::TenantId.eq(head.tenant_id))
            .filter(forum_attachment_relation::Column::TargetKind.eq(&head.target_kind))
            .filter(forum_attachment_relation::Column::TargetId.eq(head.target_id))
            .filter(forum_attachment_relation::Column::Locale.eq(&head.locale))
            .order_by_asc(forum_attachment_relation::Column::Position)
            .all(connection)
            .await?;

        if rows.len() > crate::attachment_relation::MAX_FORUM_ATTACHMENTS_PER_SET {
            return Err(ForumError::AttachmentRelationInvariant);
        }

        let mut attachments = Vec::with_capacity(rows.len());
        for row in rows {
            let position = u16::try_from(row.position)
                .ok()
                .filter(|value| {
                    usize::from(*value)
                        < crate::attachment_relation::MAX_FORUM_ATTACHMENTS_PER_SET
                })
                .ok_or(ForumError::AttachmentRelationInvariant)?;
            if row.media_id.is_nil() || row.reference_id.is_nil() {
                return Err(ForumError::AttachmentRelationInvariant);
            }
            let expected_reference_id = forum_attachment_reference_id(
                head.tenant_id,
                target,
                &head.locale,
                position,
                row.media_id,
            );
            if row.reference_id != expected_reference_id {
                return Err(ForumError::AttachmentRelationInvariant);
            }
            let usage = match row.usage.as_str() {
                "inline" => ForumAttachmentRelationUsage::Inline,
                "attachment" => ForumAttachmentRelationUsage::Attachment,
                _ => return Err(ForumError::AttachmentRelationInvariant),
            };
            if row.caption.as_deref().is_some_and(|caption| {
                caption.len() > crate::attachment_relation::MAX_FORUM_ATTACHMENT_CAPTION_BYTES
                    || caption.chars().any(char::is_control)
            }) {
                return Err(ForumError::AttachmentRelationInvariant);
            }
            attachments.push(ForumAttachmentRelationRecord {
                reference_id: row.reference_id,
                media_id: row.media_id,
                usage,
                position,
                caption: row.caption,
            });
        }

        for (expected, row) in attachments.iter().enumerate() {
            if usize::from(row.position) != expected {
                return Err(ForumError::AttachmentRelationInvariant);
            }
        }

        Ok(ForumAttachmentRelationSet {
            tenant_id: head.tenant_id,
            target,
            locale: head.locale,
            relation_revision,
            source_revision: Some(source_revision),
            attachments,
        })
    }
}

async fn lock_forum_target<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    target: ForumContentTarget,
) -> ForumResult<()> {
    let rows = match target.kind() {
        ForumContentTargetKind::Topic => forum_topic::Entity::update_many()
            .col_expr(
                forum_topic::Column::UpdatedAt,
                sea_orm::sea_query::Expr::col(forum_topic::Column::UpdatedAt),
            )
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.eq(target.id()))
            .exec(connection)
            .await?
            .rows_affected(),
        ForumContentTargetKind::Reply => forum_reply::Entity::update_many()
            .col_expr(
                forum_reply::Column::UpdatedAt,
                sea_orm::sea_query::Expr::col(forum_reply::Column::UpdatedAt),
            )
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::Id.eq(target.id()))
            .exec(connection)
            .await?
            .rows_affected(),
    };

    if rows != 1 {
        return Err(match target.kind() {
            ForumContentTargetKind::Topic => ForumError::TopicNotFound(target.id()),
            ForumContentTargetKind::Reply => ForumError::ReplyNotFound(target.id()),
        });
    }
    Ok(())
}

async fn insert_head<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    target: ForumContentTarget,
    locale: &str,
    relation_revision: ForumAttachmentRelationRevision,
    source_revision: u64,
) -> ForumResult<()> {
    let model = forum_attachment_relation_head::ActiveModel {
        tenant_id: Set(tenant_id),
        target_kind: Set(target_kind_value(target.kind()).to_string()),
        target_id: Set(target.id()),
        locale: Set(locale.to_string()),
        relation_revision: Set(relation_revision.value()),
        source_revision: Set(
            i64::try_from(source_revision)
                .map_err(|_| ForumError::AttachmentRelationInvariant)?,
        ),
        updated_at: Set(Utc::now().fixed_offset()),
    };
    model.insert(connection).await?;
    Ok(())
}

async fn update_head<C: ConnectionTrait>(
    connection: &C,
    head: &forum_attachment_relation_head::Model,
    relation_revision: ForumAttachmentRelationRevision,
    source_revision: u64,
) -> ForumResult<()> {
    let mut model: forum_attachment_relation_head::ActiveModel = head.clone().into();
    model.relation_revision = Set(relation_revision.value());
    model.source_revision = Set(
        i64::try_from(source_revision)
            .map_err(|_| ForumError::AttachmentRelationInvariant)?,
    );
    model.updated_at = Set(Utc::now().fixed_offset());
    model.update(connection).await?;
    Ok(())
}

async fn delete_relations_for_head<C: ConnectionTrait>(
    connection: &C,
    head: &forum_attachment_relation_head::Model,
) -> ForumResult<()> {
    forum_attachment_relation::Entity::delete_many()
        .filter(forum_attachment_relation::Column::TenantId.eq(head.tenant_id))
        .filter(forum_attachment_relation::Column::TargetKind.eq(&head.target_kind))
        .filter(forum_attachment_relation::Column::TargetId.eq(head.target_id))
        .filter(forum_attachment_relation::Column::Locale.eq(&head.locale))
        .exec(connection)
        .await?;
    Ok(())
}

async fn insert_relations_for_head<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    target: ForumContentTarget,
    locale: &str,
    attachments: &[crate::attachment_relation::ForumPreparedAttachmentRelation],
) -> ForumResult<()> {
    if attachments.is_empty() {
        return Ok(());
    }

    let rows = attachments
        .iter()
        .map(|relation| forum_attachment_relation::ActiveModel {
            reference_id: Set(forum_attachment_reference_id(
                tenant_id,
                target,
                locale,
                relation.position,
                relation.media_id,
            )),
            tenant_id: Set(tenant_id),
            target_kind: Set(target_kind_value(target.kind()).to_string()),
            target_id: Set(target.id()),
            locale: Set(locale.to_string()),
            position: Set(i32::from(relation.position)),
            media_id: Set(relation.media_id),
            usage: Set(usage_value(relation.usage).to_string()),
            caption: Set(relation.caption.clone()),
            created_at: sea_orm::ActiveValue::NotSet,
        })
        .collect::<Vec<_>>();

    forum_attachment_relation::Entity::insert_many(rows)
        .exec(connection)
        .await?;
    Ok(())
}

fn same_requested_state(
    batch: &crate::attachment_relation::ForumPreparedAttachmentRelationBatch,
    current: &ForumAttachmentRelationSet,
) -> bool {
    if batch.source().source_revision() != current.source_revision.unwrap_or_default()
        || batch.attachments().len() != current.attachments.len()
    {
        return false;
    }

    batch
        .attachments()
        .iter()
        .zip(current.attachments.iter())
        .all(|(requested, current)| {
            requested.media_id == current.media_id
                && requested.usage == current.usage
                && requested.position == current.position
                && requested.caption == current.caption
        })
}

fn empty_relation_set(
    tenant_id: Uuid,
    target: ForumContentTarget,
    locale: String,
) -> ForumAttachmentRelationSet {
    ForumAttachmentRelationSet {
        tenant_id,
        target,
        locale,
        relation_revision: ForumAttachmentRelationRevision::EMPTY,
        source_revision: None,
        attachments: Vec::new(),
    }
}

fn parse_context_tenant(context: &PortContext) -> ForumResult<Uuid> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        ForumError::Validation(
            "Forum attachment relation context must carry a UUID tenant ID".to_string(),
        )
    })
}

fn validate_context_target(tenant_id: Uuid, target: ForumContentTarget) -> ForumResult<()> {
    if tenant_id.is_nil() || target.id().is_nil() {
        return Err(ForumError::Validation(
            "Forum attachment relation requires non-nil tenant and target IDs".to_string(),
        ));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    rustok_api::normalize_locale_tag(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn target_kind_value(kind: ForumContentTargetKind) -> &'static str {
    match kind {
        ForumContentTargetKind::Topic => "topic",
        ForumContentTargetKind::Reply => "reply",
    }
}

fn parse_target_kind(value: &str) -> ForumResult<ForumContentTargetKind> {
    match value {
        "topic" => Ok(ForumContentTargetKind::Topic),
        "reply" => Ok(ForumContentTargetKind::Reply),
        _ => Err(ForumError::AttachmentRelationInvariant),
    }
}

fn usage_value(usage: ForumAttachmentRelationUsage) -> &'static str {
    match usage {
        ForumAttachmentRelationUsage::Inline => "inline",
        ForumAttachmentRelationUsage::Attachment => "attachment",
    }
}

fn forum_attachment_reference_id(
    tenant_id: Uuid,
    target: ForumContentTarget,
    locale: &str,
    position: u16,
    media_id: Uuid,
) -> Uuid {
    let identity = format!(
        "{FORUM_ATTACHMENT_REFERENCE_PREFIX}:{tenant_id}:{}:{}:{locale}:{position}:{media_id}",
        target_kind_value(target.kind()),
        target.id(),
    );
    let digest = Sha256::digest(identity.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn attachment_admission_to_forum_error(
    error: ForumAttachmentRelationAdmissionError,
) -> ForumError {
    ForumError::Validation(error.to_string())
}

fn map_relation_revision_error(
    error: crate::attachment_relation::ForumAttachmentRelationRevisionError,
) -> ForumError {
    match error {
        crate::attachment_relation::ForumAttachmentRelationRevisionError::Negative { .. } => {
            ForumError::AttachmentRelationInvariant
        }
        crate::attachment_relation::ForumAttachmentRelationRevisionError::Exhausted => {
            ForumError::AttachmentRelationRevisionExhausted
        }
        crate::attachment_relation::ForumAttachmentRelationRevisionError::Conflict { .. } => {
            ForumError::RelationRevisionConflict
        }
    }
}

fn media_port_error(error: PortError) -> ForumError {
    ForumError::capability_failure(
        "forum.media.asset_reference_retention",
        error.code,
        error.message,
        error.retryable,
    )
}

#[cfg(test)]
mod tests {
    use super::forum_attachment_reference_id;
    use crate::attachment_relation::ForumContentTarget;
    use uuid::Uuid;

    #[test]
    fn attachment_reference_identity_is_stable_and_changes_with_position_or_media() {
        let tenant = Uuid::new_v4();
        let target = ForumContentTarget::topic(Uuid::new_v4());
        let media_a = Uuid::new_v4();
        let media_b = Uuid::new_v4();

        let original = forum_attachment_reference_id(tenant, target, "en", 0, media_a);
        assert_eq!(
            original,
            forum_attachment_reference_id(tenant, target, "en", 0, media_a)
        );
        assert_ne!(
            original,
            forum_attachment_reference_id(tenant, target, "en", 0, media_b)
        );
        assert_ne!(
            original,
            forum_attachment_reference_id(tenant, target, "en", 1, media_a)
        );
    }
}
