use std::{collections::BTreeMap, sync::Arc};

use chrono::Utc;
use rustok_api::{Action, PortContext, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, OwnerSlug, ResourceKind,
    TranslationResourceLifecycle, TranslationResourceSummary, TranslationTargetCapability,
    TranslationTargetChangesRequest, TranslationTargetRegistry,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QuerySelect, Set, TransactionTrait, sea_query::OnConflict,
};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{inventory_resource, provider_checkpoint},
    memory::record_owner_deletion,
    observability::{self, ProviderOperation},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationInventorySyncResult {
    pub observed_resources: u64,
    pub checkpoint: Option<OpaqueCursor>,
    pub checkpoint_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationInventoryRebuildResult {
    pub observed_resources: u64,
    pub checkpoint: Option<OpaqueCursor>,
    pub checkpoint_revision: i64,
}

const MAX_FULL_RESCAN_CHANGE_PAGES: usize = 10_000;
const MAX_FULL_RESCAN_RESOURCES: usize = 100_000;

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
        observability::observe_provider_operation(
            ProviderOperation::ChangeSync,
            self.sync_provider_changes_inner(context, owner_slug, resource_kind, limit),
        )
        .await
    }

    async fn sync_provider_changes_inner(
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
        let request = TranslationTargetChangesRequest {
            after: after.clone(),
            limit,
        };
        request
            .validate()
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        let page = provider.read_changes(context, request).await?;
        if !page.changes.is_empty() && page.next_cursor.is_none() {
            return Err(TranslationError::MissingCheckpointCursor);
        }
        if !page.changes.is_empty()
            && page.next_cursor.as_ref().map(OpaqueCursor::as_str)
                == after.as_ref().map(OpaqueCursor::as_str)
        {
            return Err(TranslationError::CursorDidNotAdvance);
        }
        let mut latest_changes = BTreeMap::new();
        for change in page.changes {
            if change.identity.owner_slug != owner_slug
                || change.identity.resource_kind != resource_kind
            {
                return Err(TranslationError::ProviderIdentityMismatch);
            }
            let key = (
                change.identity.resource_id.as_str().to_string(),
                change
                    .identity
                    .subresource_id
                    .as_ref()
                    .map(|value| value.as_str().to_string())
                    .unwrap_or_default(),
            );
            latest_changes.insert(key, change);
        }
        let now = Utc::now().fixed_offset();
        provider_checkpoint::Entity::insert(provider_checkpoint::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            owner_slug: Set(owner_slug.as_str().to_string()),
            resource_kind: Set(resource_kind.as_str().to_string()),
            cursor: Set(None),
            revision: Set(0),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                provider_checkpoint::Column::TenantId,
                provider_checkpoint::Column::OwnerSlug,
                provider_checkpoint::Column::ResourceKind,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&self.database)
        .await?;

        let transaction = self.database.begin().await?;
        let current_checkpoint_query = provider_checkpoint::Entity::find()
            .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
            .filter(provider_checkpoint::Column::OwnerSlug.eq(owner_slug.as_str()))
            .filter(provider_checkpoint::Column::ResourceKind.eq(resource_kind.as_str()));
        let current_checkpoint = match self.database.get_database_backend() {
            DbBackend::Postgres | DbBackend::MySql => {
                current_checkpoint_query
                    .lock_exclusive()
                    .one(&transaction)
                    .await?
            }
            DbBackend::Sqlite => current_checkpoint_query.one(&transaction).await?,
        }
        .ok_or(TranslationError::CheckpointConflict)?;
        let expected_revision = checkpoint.as_ref().map_or(0, |row| row.revision);
        let expected_cursor = checkpoint.as_ref().and_then(|row| row.cursor.as_deref());
        if current_checkpoint.revision != expected_revision
            || current_checkpoint.cursor.as_deref() != expected_cursor
        {
            return Err(TranslationError::CheckpointConflict);
        }

        let resource_ids = latest_changes
            .keys()
            .map(|(resource_id, _)| resource_id.clone())
            .collect::<Vec<_>>();
        let existing_rows = if resource_ids.is_empty() {
            Vec::new()
        } else {
            inventory_resource::Entity::find()
                .filter(inventory_resource::Column::TenantId.eq(tenant_id))
                .filter(inventory_resource::Column::OwnerSlug.eq(owner_slug.as_str()))
                .filter(inventory_resource::Column::ResourceKind.eq(resource_kind.as_str()))
                .filter(inventory_resource::Column::ResourceId.is_in(resource_ids))
                .all(&transaction)
                .await?
        };
        let mut existing_by_identity = existing_rows
            .into_iter()
            .map(|row| ((row.resource_id.clone(), row.subresource_key.clone()), row))
            .collect::<BTreeMap<_, _>>();

        for (identity_key, change) in &latest_changes {
            let identity = &change.identity;
            let lifecycle = lifecycle_name(change.lifecycle).to_string();
            if let Some(existing) = existing_by_identity.remove(identity_key) {
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
                    subresource_key: Set(identity_key.1.clone()),
                    resource_revision: Set(change.resource_revision.as_str().to_string()),
                    lifecycle: Set(lifecycle),
                    observed_at: Set(now),
                }
                .insert(&transaction)
                .await?;
            }
            if change.lifecycle == TranslationResourceLifecycle::Deleted {
                record_owner_deletion(
                    &transaction,
                    tenant_id,
                    identity,
                    change.resource_revision.as_str(),
                    now,
                )
                .await?;
            }
        }

        let cursor = page.next_cursor.clone().or_else(|| {
            current_checkpoint
                .cursor
                .as_deref()
                .and_then(|value| OpaqueCursor::new(value).ok())
        });
        let checkpoint_revision = current_checkpoint
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
                sea_orm::sea_query::Expr::value(checkpoint_revision),
            )
            .col_expr(
                provider_checkpoint::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(provider_checkpoint::Column::Id.eq(current_checkpoint.id))
            .filter(provider_checkpoint::Column::Revision.eq(current_checkpoint.revision))
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::CheckpointConflict);
        }
        transaction.commit().await?;

        Ok(TranslationInventorySyncResult {
            observed_resources: latest_changes.len() as u64,
            checkpoint: cursor,
            checkpoint_revision,
        })
    }

    pub async fn rebuild_provider_inventory(
        &self,
        context: PortContext,
        owner_slug: OwnerSlug,
        resource_kind: ResourceKind,
        source_locale: TenantLocale,
        target_locale: TenantLocale,
        page_size: u16,
    ) -> TranslationResult<TranslationInventoryRebuildResult> {
        observability::observe_provider_operation(
            ProviderOperation::InventoryRebuild,
            self.rebuild_provider_inventory_inner(
                context,
                owner_slug,
                resource_kind,
                source_locale,
                target_locale,
                page_size,
            ),
        )
        .await
    }

    async fn rebuild_provider_inventory_inner(
        &self,
        context: PortContext,
        owner_slug: OwnerSlug,
        resource_kind: ResourceKind,
        source_locale: TenantLocale,
        target_locale: TenantLocale,
        page_size: u16,
    ) -> TranslationResult<TranslationInventoryRebuildResult> {
        let list_request = ListTranslationResourcesRequest {
            source_locale: source_locale.clone(),
            target_locale: target_locale.clone(),
            cursor: None,
            limit: page_size,
        };
        list_request
            .validate()
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
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
            .contains(&TranslationTargetCapability::ListResources)
        {
            return Err(TranslationError::FullRescanUnavailable);
        }

        let mut drained = false;
        for _ in 0..MAX_FULL_RESCAN_CHANGE_PAGES {
            let result = self
                .sync_provider_changes(
                    context.clone(),
                    owner_slug.clone(),
                    resource_kind.clone(),
                    page_size,
                )
                .await?;
            if result.observed_resources == 0 {
                drained = true;
                break;
            }
        }
        if !drained {
            return Err(TranslationError::FullRescanChangeDrainLimit);
        }

        let tenant_id =
            Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
        let expected_checkpoint = provider_checkpoint::Entity::find()
            .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
            .filter(provider_checkpoint::Column::OwnerSlug.eq(owner_slug.as_str()))
            .filter(provider_checkpoint::Column::ResourceKind.eq(resource_kind.as_str()))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::CheckpointConflict)?;

        let mut resources = BTreeMap::<(String, String), TranslationResourceSummary>::new();
        let mut cursor = None;
        loop {
            let request = ListTranslationResourcesRequest {
                source_locale: source_locale.clone(),
                target_locale: target_locale.clone(),
                cursor: cursor.clone(),
                limit: page_size,
            };
            let page = provider.list_resources(context.clone(), request).await?;
            if page.resources.len() > usize::from(page_size) {
                return Err(TranslationError::FullRescanPageOverflow);
            }
            if page.next_cursor.as_ref().map(OpaqueCursor::as_str)
                == cursor.as_ref().map(OpaqueCursor::as_str)
                && page.next_cursor.is_some()
            {
                return Err(TranslationError::FullRescanCursorDidNotAdvance);
            }
            for resource in page.resources {
                if resource.identity.owner_slug != owner_slug
                    || resource.identity.resource_kind != resource_kind
                {
                    return Err(TranslationError::ProviderIdentityMismatch);
                }
                let key = (
                    resource.identity.resource_id.as_str().to_string(),
                    resource
                        .identity
                        .subresource_id
                        .as_ref()
                        .map(|value| value.as_str().to_string())
                        .unwrap_or_default(),
                );
                resources.insert(key, resource);
                if resources.len() > MAX_FULL_RESCAN_RESOURCES {
                    return Err(TranslationError::FullRescanResourceLimit);
                }
            }
            match page.next_cursor {
                Some(next_cursor) => cursor = Some(next_cursor),
                None => break,
            }
        }

        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        let current_checkpoint_query = provider_checkpoint::Entity::find()
            .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
            .filter(provider_checkpoint::Column::OwnerSlug.eq(owner_slug.as_str()))
            .filter(provider_checkpoint::Column::ResourceKind.eq(resource_kind.as_str()));
        let current_checkpoint = match self.database.get_database_backend() {
            DbBackend::Postgres | DbBackend::MySql => {
                current_checkpoint_query
                    .lock_exclusive()
                    .one(&transaction)
                    .await?
            }
            DbBackend::Sqlite => current_checkpoint_query.one(&transaction).await?,
        }
        .ok_or(TranslationError::CheckpointConflict)?;
        if current_checkpoint.revision != expected_checkpoint.revision
            || current_checkpoint.cursor != expected_checkpoint.cursor
        {
            return Err(TranslationError::CheckpointConflict);
        }

        inventory_resource::Entity::delete_many()
            .filter(inventory_resource::Column::TenantId.eq(tenant_id))
            .filter(inventory_resource::Column::OwnerSlug.eq(owner_slug.as_str()))
            .filter(inventory_resource::Column::ResourceKind.eq(resource_kind.as_str()))
            .exec(&transaction)
            .await?;
        for ((_, subresource_key), resource) in &resources {
            inventory_resource::ActiveModel {
                id: Set(generate_id()),
                tenant_id: Set(tenant_id),
                owner_slug: Set(owner_slug.as_str().to_string()),
                resource_kind: Set(resource_kind.as_str().to_string()),
                resource_id: Set(resource.identity.resource_id.as_str().to_string()),
                subresource_key: Set(subresource_key.clone()),
                resource_revision: Set(resource.resource_revision.as_str().to_string()),
                lifecycle: Set(lifecycle_name(resource.lifecycle).to_string()),
                observed_at: Set(now),
            }
            .insert(&transaction)
            .await?;
            if resource.lifecycle == TranslationResourceLifecycle::Deleted {
                record_owner_deletion(
                    &transaction,
                    tenant_id,
                    &resource.identity,
                    resource.resource_revision.as_str(),
                    now,
                )
                .await?;
            }
        }

        let checkpoint_revision = current_checkpoint
            .revision
            .checked_add(1)
            .ok_or(TranslationError::CheckpointConflict)?;
        let update = provider_checkpoint::Entity::update_many()
            .col_expr(
                provider_checkpoint::Column::Revision,
                sea_orm::sea_query::Expr::value(checkpoint_revision),
            )
            .col_expr(
                provider_checkpoint::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(provider_checkpoint::Column::Id.eq(current_checkpoint.id))
            .filter(provider_checkpoint::Column::Revision.eq(current_checkpoint.revision))
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::CheckpointConflict);
        }
        transaction.commit().await?;

        let checkpoint = current_checkpoint
            .cursor
            .as_deref()
            .map(OpaqueCursor::new)
            .transpose()
            .map_err(|_| TranslationError::CheckpointConflict)?;
        Ok(TranslationInventoryRebuildResult {
            observed_resources: resources.len() as u64,
            checkpoint,
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
