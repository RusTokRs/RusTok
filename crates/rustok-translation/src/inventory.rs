use std::sync::Arc;

use chrono::Utc;
use rustok_api::{Action, PortContext, Resource};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_translation_targets::{
    OpaqueCursor, OwnerSlug, ResourceKind, TranslationResourceLifecycle,
    TranslationTargetCapability, TranslationTargetChangesRequest, TranslationTargetRegistry,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{inventory_resource, provider_checkpoint},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationInventorySyncResult {
    pub observed_resources: u64,
    pub checkpoint: Option<OpaqueCursor>,
    pub checkpoint_revision: i64,
}

pub struct TranslationInventoryService {
    database: DatabaseConnection,
    providers: Arc<TranslationTargetRegistry>,
}

impl TranslationInventoryService {
    pub fn new(database: DatabaseConnection, providers: Arc<TranslationTargetRegistry>) -> Self {
        Self {
            database,
            providers,
        }
    }

    pub async fn sync_provider_changes(
        &self,
        context: PortContext,
        owner_slug: OwnerSlug,
        resource_kind: ResourceKind,
        limit: u16,
    ) -> TranslationResult<TranslationInventorySyncResult> {
        context.require_policy(rustok_api::PortCallPolicy::read())?;
        let security = SecurityContext::try_from_port_context(&context)?;
        if security.get_scope(Resource::Translations, Action::Update) == PermissionScope::None {
            return Err(TranslationError::Forbidden);
        }
        let tenant_id =
            Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
        let provider = self
            .providers
            .get(&owner_slug, &resource_kind)
            .ok_or_else(|| TranslationError::ProviderNotFound {
                owner_slug: owner_slug.as_str().to_string(),
                resource_kind: resource_kind.as_str().to_string(),
            })?;
        if !provider
            .descriptor()
            .capabilities
            .contains(&TranslationTargetCapability::ChangeCursor)
        {
            return Err(TranslationError::ChangeCursorUnavailable);
        }
        let checkpoint = provider_checkpoint::Entity::find()
            .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
            .filter(provider_checkpoint::Column::OwnerSlug.eq(owner_slug.as_str()))
            .filter(provider_checkpoint::Column::ResourceKind.eq(resource_kind.as_str()))
            .one(&self.database)
            .await?;
        let after = checkpoint
            .as_ref()
            .and_then(|row| row.cursor.as_deref())
            .map(OpaqueCursor::new)
            .transpose()
            .map_err(|_| TranslationError::CheckpointConflict)?;
        let page = provider
            .read_changes(context, TranslationTargetChangesRequest { after, limit })
            .await?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;

        for change in &page.changes {
            let identity = &change.identity;
            let subresource_key = identity
                .subresource_id
                .as_ref()
                .map(|value| value.as_str())
                .unwrap_or_default();
            let existing = inventory_resource::Entity::find()
                .filter(inventory_resource::Column::TenantId.eq(tenant_id))
                .filter(inventory_resource::Column::OwnerSlug.eq(identity.owner_slug.as_str()))
                .filter(
                    inventory_resource::Column::ResourceKind.eq(identity.resource_kind.as_str()),
                )
                .filter(inventory_resource::Column::ResourceId.eq(identity.resource_id.as_str()))
                .filter(inventory_resource::Column::SubresourceKey.eq(subresource_key))
                .one(&transaction)
                .await?;
            let lifecycle = lifecycle_name(change.lifecycle).to_string();
            if let Some(existing) = existing {
                let mut active: inventory_resource::ActiveModel = existing.into();
                active.resource_revision = Set(change.resource_revision.as_str().to_string());
                active.lifecycle = Set(lifecycle);
                active.observed_at = Set(now);
                active.update(&transaction).await?;
            } else {
                inventory_resource::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    owner_slug: Set(identity.owner_slug.as_str().to_string()),
                    resource_kind: Set(identity.resource_kind.as_str().to_string()),
                    resource_id: Set(identity.resource_id.as_str().to_string()),
                    subresource_key: Set(subresource_key.to_string()),
                    resource_revision: Set(change.resource_revision.as_str().to_string()),
                    lifecycle: Set(lifecycle),
                    observed_at: Set(now),
                }
                .insert(&transaction)
                .await?;
            }
        }

        let cursor = page.next_cursor.clone().or_else(|| {
            checkpoint
                .as_ref()?
                .cursor
                .as_deref()
                .and_then(|value| OpaqueCursor::new(value).ok())
        });
        let checkpoint_revision = if let Some(existing) = checkpoint {
            let next_revision = existing
                .revision
                .checked_add(1)
                .ok_or(TranslationError::CheckpointConflict)?;
            let update = provider_checkpoint::Entity::update_many()
                .col_expr(
                    provider_checkpoint::Column::Cursor,
                    sea_orm::sea_query::Expr::value(
                        cursor.as_ref().map(|value| value.as_str().to_string()),
                    ),
                )
                .col_expr(
                    provider_checkpoint::Column::Revision,
                    sea_orm::sea_query::Expr::value(next_revision),
                )
                .col_expr(
                    provider_checkpoint::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(provider_checkpoint::Column::Id.eq(existing.id))
                .filter(provider_checkpoint::Column::Revision.eq(existing.revision))
                .exec(&transaction)
                .await?;
            if update.rows_affected != 1 {
                return Err(TranslationError::CheckpointConflict);
            }
            next_revision
        } else {
            provider_checkpoint::ActiveModel {
                id: Set(generate_id()),
                tenant_id: Set(tenant_id),
                owner_slug: Set(owner_slug.as_str().to_string()),
                resource_kind: Set(resource_kind.as_str().to_string()),
                cursor: Set(cursor.as_ref().map(|value| value.as_str().to_string())),
                revision: Set(1),
                updated_at: Set(now),
            }
            .insert(&transaction)
            .await?;
            1
        };
        transaction.commit().await?;

        Ok(TranslationInventorySyncResult {
            observed_resources: page.changes.len() as u64,
            checkpoint: cursor,
            checkpoint_revision,
        })
    }
}

fn lifecycle_name(lifecycle: TranslationResourceLifecycle) -> &'static str {
    match lifecycle {
        TranslationResourceLifecycle::Active => "active",
        TranslationResourceLifecycle::Archived => "archived",
        TranslationResourceLifecycle::Deleted => "deleted",
        TranslationResourceLifecycle::Unavailable => "unavailable",
    }
}
