use std::collections::HashMap;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::dto::{
    BindChannelModuleInput, BindChannelOauthAppInput, ChannelDetailResponse,
    ChannelModuleBindingResponse, ChannelOauthAppResponse,
    ChannelResolutionPolicySetDetailResponse, ChannelResolutionPolicySetResponse,
    ChannelResolutionRuleResponse, ChannelResponse, ChannelTargetResponse, CreateChannelInput,
    CreateChannelResolutionPolicySetInput, CreateChannelResolutionRuleInput,
    CreateChannelTargetInput, ReorderChannelResolutionRulesInput, UpdateChannelInput,
    UpdateChannelResolutionRuleInput, UpdateChannelTargetInput,
};
use crate::entities::channel::{self, ActiveModel as ChannelActiveModel};
use crate::entities::channel_module_binding::{
    self, ActiveModel as ChannelModuleBindingActiveModel,
};
use crate::entities::channel_oauth_app::{self, ActiveModel as ChannelOauthAppActiveModel};
use crate::entities::channel_resolution_policy_rule::{
    self, ActiveModel as ChannelResolutionPolicyRuleActiveModel,
};
use crate::entities::channel_resolution_policy_set::{
    self, ActiveModel as ChannelResolutionPolicySetActiveModel,
};
use crate::entities::channel_target::{self, ActiveModel as ChannelTargetActiveModel};
use crate::error::{ChannelError, ChannelResult};
use crate::policy::{
    CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION, ChannelResolutionRuleDefinition, ResolutionAction,
    ResolutionPredicate, StoredChannelResolutionRule,
};
use crate::resolution::TargetSurface;
use crate::target_type::ChannelTargetType;

mod public_module;
pub use public_module::in_process_public_module_read_port;

/// Column widths the channel schema enforces.
///
/// PostgreSQL `varchar(n)` counts characters while Rust `str::len()` counts
/// bytes, so these limits are checked with `chars().count()`: using byte length
/// rejected valid multi-byte names and, worse, let ASCII values longer than the
/// column through to fail later as a database error.
const CHANNEL_SLUG_MAX_CHARS: usize = 100;
const CHANNEL_NAME_MAX_CHARS: usize = 255;
const CHANNEL_MODULE_SLUG_MAX_CHARS: usize = 100;
const CHANNEL_POLICY_SET_SLUG_MAX_CHARS: usize = 100;
const CHANNEL_TARGET_VALUE_MAX_CHARS: usize = 500;
const CHANNEL_SETTINGS_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct ChannelService {
    db: DatabaseConnection,
}

impl ChannelService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %input.tenant_id, slug = %input.slug))]
    pub async fn create_channel(
        &self,
        input: CreateChannelInput,
    ) -> ChannelResult<ChannelResponse> {
        if input.tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "channel tenant_id must not be nil".to_string(),
            ));
        }
        let slug = normalize_channel_slug(&input.slug)?;
        let name = normalize_channel_name(&input.name)?;

        // The default flag and the slug are both unique per tenant, so the insert
        // is the conflict test: a pre-flight `SELECT` would still race, and the
        // database is the only authority that can decide the winner.
        let now = chrono::Utc::now().into();
        let settings = normalize_channel_settings(
            input.settings.unwrap_or_else(|| serde_json::json!({})),
        )?;

        for attempt in 0..2 {
            let is_default = attempt == 0
                && !self
                    .default_channel_exists_for_tenant(input.tenant_id)
                    .await?;
            let model = ChannelActiveModel {
                id: Set(generate_id()),
                tenant_id: Set(input.tenant_id),
                slug: Set(slug.clone()),
                name: Set(name.clone()),
                is_active: Set(true),
                is_default: Set(is_default),
                settings: Set(settings.clone()),
                created_at: Set(now),
                updated_at: Set(now),
            };

            match model.insert(&self.db).await {
                Ok(model) => return Ok(to_channel_response(model)),
                Err(error) if is_unique_violation(&error) => {
                    if is_channel_slug_conflict(&error) {
                        return Err(ChannelError::SlugAlreadyExists(slug));
                    }
                    // A concurrent create claimed the tenant default between the
                    // check and the insert: retry once as a non-default channel.
                    if is_default {
                        continue;
                    }
                    return Err(ChannelError::Database(error));
                }
                Err(error) => return Err(ChannelError::Database(error)),
            }
        }

        Err(ChannelError::Conflict(
            "a concurrent channel create claimed the tenant default channel; retry the operation"
                .to_string(),
        ))
    }

    /// Applies a name/settings patch to an existing channel of the caller's tenant.
    ///
    /// The slug is immutable: it is the stable channel identity used by consumers
    /// and by persisted visibility relations, so renaming it would silently break
    /// stored references.
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, channel_id = %channel_id))]
    pub async fn update_channel(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
        input: UpdateChannelInput,
    ) -> ChannelResult<ChannelResponse> {
        let channel = self.ensure_channel_for_tenant(tenant_id, channel_id).await?;
        let name = match input.name {
            Some(name) => Some(normalize_channel_name(&name)?),
            None => None,
        };
        if name.is_none() && input.settings.is_none() {
            return Err(ChannelError::Validation(
                "channel update requires at least one of name or settings".to_string(),
            ));
        }

        let now = chrono::Utc::now().into();
        let mut active: channel::ActiveModel = channel.into();
        if let Some(name) = name {
            active.name = Set(name);
        }
        if let Some(settings) = input.settings {
            active.settings = Set(normalize_channel_settings(settings)?);
        }
        active.updated_at = Set(now);
        Ok(to_channel_response(active.update(&self.db).await?))
    }

    /// Activates or deactivates a channel.
    ///
    /// Deactivating the tenant default promotes the oldest remaining active
    /// channel instead of leaving a default that can never be resolved, and
    /// clears the default flag when the tenant has no active channel left. Both
    /// writes happen in one transaction so `is_default` never points at an
    /// inactive channel.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, channel_id = %channel_id))]
    pub async fn set_channel_active(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
        is_active: bool,
    ) -> ChannelResult<ChannelResponse> {
        let channel = self.ensure_channel_for_tenant(tenant_id, channel_id).await?;
        if channel.is_active == is_active {
            return Ok(to_channel_response(channel));
        }

        let now = chrono::Utc::now().into();
        let txn = self.db.begin().await?;
        let mut active: channel::ActiveModel = channel.clone().into();
        active.is_active = Set(is_active);
        if !is_active && channel.is_default {
            active.is_default = Set(false);
        }
        active.updated_at = Set(now);
        active.update(&txn).await?;

        if !is_active && channel.is_default {
            let promoted = channel::Entity::find()
                .filter(channel::Column::TenantId.eq(tenant_id))
                .filter(channel::Column::Id.ne(channel_id))
                .filter(channel::Column::IsActive.eq(true))
                .order_by_asc(channel::Column::CreatedAt)
                .order_by_asc(channel::Column::Id)
                .one(&txn)
                .await?;
            if let Some(promoted) = promoted {
                let mut promoted_active: channel::ActiveModel = promoted.into();
                promoted_active.is_default = Set(true);
                promoted_active.updated_at = Set(now);
                if let Err(error) = promoted_active.update(&txn).await {
                    txn.rollback().await?;
                    return Err(map_unique_violation(error, "promote the tenant default channel"));
                }
            }
        }

        txn.commit().await?;
        self.get_channel_for_tenant(tenant_id, channel_id).await
    }

    /// Deletes a channel of the caller's tenant.
    ///
    /// Targets, module bindings, OAuth app bindings and resolution rules that
    /// point at the channel are removed by the schema's cascading foreign keys, and
    /// the PostgreSQL tombstone trigger records the deletion for the Sales Channel
    /// Index bridge. Deleting the tenant default promotes the oldest remaining
    /// active channel in the same transaction.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, channel_id = %channel_id))]
    pub async fn delete_channel(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
    ) -> ChannelResult<ChannelResponse> {
        let channel = self.ensure_channel_for_tenant(tenant_id, channel_id).await?;
        let response = to_channel_response(channel.clone());
        let now = chrono::Utc::now().into();

        let txn = self.db.begin().await?;
        let deleted = channel::Entity::delete_many()
            .filter(channel::Column::Id.eq(channel_id))
            .filter(channel::Column::TenantId.eq(tenant_id))
            .exec(&txn)
            .await?;
        if deleted.rows_affected == 0 {
            txn.rollback().await?;
            return Err(ChannelError::NotFound(channel_id));
        }

        if channel.is_default {
            let promoted = channel::Entity::find()
                .filter(channel::Column::TenantId.eq(tenant_id))
                .filter(channel::Column::IsActive.eq(true))
                .order_by_asc(channel::Column::CreatedAt)
                .order_by_asc(channel::Column::Id)
                .one(&txn)
                .await?;
            if let Some(promoted) = promoted {
                let mut promoted_active: channel::ActiveModel = promoted.into();
                promoted_active.is_default = Set(true);
                promoted_active.updated_at = Set(now);
                if let Err(error) = promoted_active.update(&txn).await {
                    txn.rollback().await?;
                    return Err(map_unique_violation(error, "promote the tenant default channel"));
                }
            }
        }

        txn.commit().await?;
        Ok(response)
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, channel_id = %channel_id))]
    pub async fn get_channel_for_tenant(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
    ) -> ChannelResult<ChannelResponse> {
        if tenant_id.is_nil() || channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        let model = channel::Entity::find_by_id(channel_id)
            .filter(channel::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;
        Ok(to_channel_response(model))
    }

    /// Proves that a channel belongs to the caller's tenant and returns its model.
    ///
    /// Owner mutators are addressed by globally unique channel ids; every transport
    /// boundary must call this (or the equivalent read) before mutating so a
    /// leaked identifier cannot cross the tenant boundary.
    pub async fn ensure_channel_for_tenant(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
    ) -> ChannelResult<channel::Model> {
        if tenant_id.is_nil() || channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        channel::Entity::find_by_id(channel_id)
            .filter(channel::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id))]
    pub async fn get_channel(&self, channel_id: Uuid) -> ChannelResult<ChannelResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        let model = channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;
        Ok(to_channel_response(model))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id))]
    pub async fn get_channel_detail(
        &self,
        channel_id: Uuid,
    ) -> ChannelResult<ChannelDetailResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        let model = channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;
        self.build_channel_detail(model).await
    }

    pub async fn get_channel_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
    ) -> ChannelResult<Option<ChannelResponse>> {
        if tenant_id.is_nil() {
            return Ok(None);
        }
        let slug = slug.trim().to_ascii_lowercase();
        if slug.is_empty() {
            return Ok(None);
        }
        let model = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Slug.eq(&slug))
            .one(&self.db)
            .await?;
        Ok(model.map(to_channel_response))
    }

    pub async fn get_channel_detail_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
    ) -> ChannelResult<Option<ChannelDetailResponse>> {
        if tenant_id.is_nil() {
            return Ok(None);
        }
        let slug = slug.trim().to_ascii_lowercase();
        if slug.is_empty() {
            return Ok(None);
        }
        let model = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Slug.eq(&slug))
            .one(&self.db)
            .await?;

        match model {
            Some(model) => Ok(Some(self.build_channel_detail(model).await?)),
            None => Ok(None),
        }
    }

    pub async fn get_channel_by_host_target_value(
        &self,
        tenant_id: Uuid,
        target_value: &str,
    ) -> ChannelResult<Option<ChannelDetailResponse>> {
        if tenant_id.is_nil() {
            return Ok(None);
        }
        let normalized = match ChannelTargetType::WebDomain.normalize_value(target_value) {
            Some(value) => value,
            None => return Ok(None),
        };
        let target = channel_target::Entity::find()
            .filter(channel_target::Column::TargetType.eq(ChannelTargetType::WebDomain.as_str()))
            .filter(channel_target::Column::Value.eq(&normalized))
            .find_also_related(channel::Entity)
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::IsActive.eq(true))
            .one(&self.db)
            .await?;

        let Some((target, Some(channel_model))) = target else {
            return Ok(None);
        };

        let mut detail = self.build_channel_detail(channel_model).await?;
        if let Some(existing) = detail
            .targets
            .iter_mut()
            .find(|item| item.id == target.id && item.channel_id == target.channel_id)
        {
            existing.target_type = target.target_type;
            existing.value = target.value;
        }
        Ok(Some(detail))
    }

    pub async fn get_default_channel(
        &self,
        tenant_id: Uuid,
    ) -> ChannelResult<Option<ChannelDetailResponse>> {
        if tenant_id.is_nil() {
            return Ok(None);
        }
        let model = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::IsActive.eq(true))
            .filter(channel::Column::IsDefault.eq(true))
            .one(&self.db)
            .await?;

        match model {
            Some(model) => Ok(Some(self.build_channel_detail(model).await?)),
            None => Ok(None),
        }
    }

    pub async fn list_channels(
        &self,
        tenant_id: Uuid,
        page: u64,
        per_page: u64,
    ) -> ChannelResult<(Vec<ChannelResponse>, u64)> {
        if tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "channel tenant_id must not be nil".to_string(),
            ));
        }
        if page == 0 || per_page == 0 {
            return Err(ChannelError::Validation(
                "channel pagination requires non-zero page and per_page".to_string(),
            ));
        }
        let paginator = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .order_by_desc(channel::Column::IsDefault)
            .order_by_asc(channel::Column::CreatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page - 1).await?;
        let items = models.into_iter().map(to_channel_response).collect();
        Ok((items, total))
    }

    /// Returns the tenant's channel option list for selectors and filters.
    ///
    /// Consumers used to call `list_channels(tenant, 1, 250)` and silently lost
    /// every channel past the 250th entry, while storefront selectors also offered
    /// inactive channels that can never resolve. This read is explicit instead:
    /// `include_inactive` states the intent, the order matches `list_channels`, and
    /// the result is never truncated — a channel row is a small administrative
    /// entity, so an option list is bounded by tenant configuration, not by a
    /// magic page size.
    pub async fn list_channel_options(
        &self,
        tenant_id: Uuid,
        include_inactive: bool,
    ) -> ChannelResult<Vec<ChannelResponse>> {
        if tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "channel tenant_id must not be nil".to_string(),
            ));
        }
        let query = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .order_by_desc(channel::Column::IsDefault)
            .order_by_asc(channel::Column::CreatedAt);
        let query = if include_inactive {
            query
        } else {
            query.filter(channel::Column::IsActive.eq(true))
        };
        let models = query.all(&self.db).await?;
        Ok(models.into_iter().map(to_channel_response).collect())
    }

    pub async fn list_channel_details_page(
        &self,
        tenant_id: Uuid,
        page: u64,
        per_page: u64,
        include_inactive: bool,
    ) -> ChannelResult<(Vec<ChannelDetailResponse>, u64)> {
        if tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "channel tenant_id must not be nil".to_string(),
            ));
        }
        if page == 0 || per_page == 0 {
            return Err(ChannelError::Validation(
                "channel pagination requires non-zero page and per_page".to_string(),
            ));
        }
        let query = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .order_by_desc(channel::Column::IsDefault)
            .order_by_asc(channel::Column::CreatedAt);
        let query = if include_inactive {
            query
        } else {
            query.filter(channel::Column::IsActive.eq(true))
        };
        let paginator = query.paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page - 1).await?;

        let items = self.build_channel_details(models).await?;
        Ok((items, total))
    }

    pub async fn list_channel_details(
        &self,
        tenant_id: Uuid,
    ) -> ChannelResult<Vec<ChannelDetailResponse>> {
        if tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "channel tenant_id must not be nil".to_string(),
            ));
        }
        let models = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .order_by_desc(channel::Column::IsDefault)
            .order_by_asc(channel::Column::CreatedAt)
            .all(&self.db)
            .await?;

        self.build_channel_details(models).await
    }

    #[instrument(skip(self), fields(channel_id = %channel_id))]
    pub async fn set_default_channel(&self, channel_id: Uuid) -> ChannelResult<ChannelResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        let channel = channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;
        if !channel.is_active {
            return Err(ChannelError::InactiveChannel(channel_id));
        }

        self.replace_default_channel(channel.tenant_id, channel_id)
            .await?;
        self.get_channel(channel_id).await
    }

    /// Sets the explicit default channel for a tenant, proving the channel scope
    /// first. Admin transports use this form so a leaked channel id cannot move
    /// another tenant's default pointer.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, channel_id = %channel_id))]
    pub async fn set_default_channel_for_tenant(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
    ) -> ChannelResult<ChannelResponse> {
        let channel = self.ensure_channel_for_tenant(tenant_id, channel_id).await?;
        if !channel.is_active {
            return Err(ChannelError::InactiveChannel(channel_id));
        }
        self.replace_default_channel(tenant_id, channel_id).await?;
        self.get_channel_for_tenant(tenant_id, channel_id).await
    }

    /// Module gate that trusts its channel id.
    ///
    /// Owner-internal only: it proves nothing about the caller's tenant. Every path
    /// where the channel id came from a request (HTTP, GraphQL, storefront, server
    /// function) must use [`Self::is_module_enabled_for_tenant`] instead.
    pub async fn is_module_enabled(
        &self,
        channel_id: Uuid,
        module_slug: &str,
    ) -> ChannelResult<bool> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        let channel = self.ensure_channel_exists(channel_id).await?;
        if !channel.is_active {
            return Ok(false);
        }

        let binding = channel_module_binding::Entity::find()
            .filter(channel_module_binding::Column::ChannelId.eq(channel_id))
            .filter(channel_module_binding::Column::ModuleSlug.eq(module_slug))
            .one(&self.db)
            .await?;

        Ok(binding.map(|item| item.is_enabled).unwrap_or(true))
    }

    /// Verifies a set of channel slugs belongs to the selected tenant inside the
    /// caller's transaction. This is intended for consumer modules that persist
    /// slug-based visibility relations and cannot rely on a foreign key.
    pub async fn ensure_channel_slugs_exist_for_tenant_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        channel_slugs: &[String],
    ) -> ChannelResult<()> {
        if tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        let requested = channel_slugs
            .iter()
            .map(|slug| slug.trim().to_ascii_lowercase())
            .filter(|slug| !slug.is_empty())
            .collect::<std::collections::BTreeSet<_>>();
        if requested.is_empty() {
            return Ok(());
        }

        let found = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Slug.is_in(requested.iter().cloned().collect::<Vec<_>>()))
            .all(txn)
            .await?;

        if found.len() != requested.len() {
            let missing = requested
                .into_iter()
                .find(|slug| !found.iter().any(|channel| channel.slug == *slug))
                .unwrap_or_default();
            return Err(ChannelError::InvalidTargetValue(format!(
                "channel slug {} does not exist for this tenant",
                missing
            )));
        }

        Ok(())
    }

    /// Checks module availability while proving that the channel belongs to the
    /// caller-selected tenant. Consumers must use this form whenever both tenant
    /// and channel identifiers cross a transport boundary.
    pub async fn is_module_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
        module_slug: &str,
    ) -> ChannelResult<bool> {
        if tenant_id.is_nil() || channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        let channel = channel::Entity::find_by_id(channel_id)
            .filter(channel::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;
        if !channel.is_active {
            return Ok(false);
        }

        let binding = channel_module_binding::Entity::find()
            .filter(channel_module_binding::Column::ChannelId.eq(channel.id))
            .filter(channel_module_binding::Column::ModuleSlug.eq(module_slug))
            .one(&self.db)
            .await?;

        Ok(binding.map(|item| item.is_enabled).unwrap_or(true))
    }

    /// Adds a target to a channel.
    ///
    /// The database enforces both exclusive claims (`uq_channel_targets_tenant_host`
    /// for web hosts and `uq_channel_targets_channel_primary` for the primary
    /// target), so a concurrent writer cannot slip a duplicate past a
    /// check-then-insert window: the insert is the claim, and a violated index is
    /// reported as the domain conflict instead of a `500`.
    #[instrument(skip(self, input), fields(channel_id = %channel_id, target_type = %input.target_type))]
    pub async fn add_target(
        &self,
        channel_id: Uuid,
        input: CreateChannelTargetInput,
    ) -> ChannelResult<ChannelTargetResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        let Some(target_type) = ChannelTargetType::parse(&input.target_type) else {
            return Err(ChannelError::InvalidTargetType(input.target_type));
        };
        let target_value = normalize_target_value(target_type, &input.value)
            .ok_or_else(|| ChannelError::InvalidTargetValue(input.value.clone()))?;
        let channel = self.ensure_channel_exists(channel_id).await?;

        let now = chrono::Utc::now().into();
        let txn = self.db.begin().await?;
        if input.is_primary {
            clear_primary_target(&txn, channel_id, None, now).await?;
        }

        let inserted = ChannelTargetActiveModel {
            id: Set(generate_id()),
            channel_id: Set(channel_id),
            tenant_id: Set(channel.tenant_id),
            target_type: Set(target_type.as_str().to_string()),
            value: Set(target_value.clone()),
            is_primary: Set(input.is_primary),
            settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await;

        let model = match inserted {
            Ok(model) => model,
            Err(error) if is_unique_violation(&error) => {
                txn.rollback().await?;
                return Err(
                    classify_target_conflict(
                        &self.db,
                        channel.tenant_id,
                        target_type,
                        target_value.as_str(),
                        input.is_primary,
                    )
                    .await,
                );
            }
            Err(error) => {
                txn.rollback().await?;
                return Err(ChannelError::Database(error));
            }
        };

        txn.commit().await?;
        Ok(to_channel_target_response(model))
    }

    #[instrument(skip(self, input), fields(channel_id = %channel_id, target_id = %target_id))]
    pub async fn update_target(
        &self,
        channel_id: Uuid,
        target_id: Uuid,
        input: UpdateChannelTargetInput,
    ) -> ChannelResult<ChannelTargetResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        if target_id.is_nil() {
            return Err(ChannelError::NotFound(target_id));
        }
        let Some(target_type) = ChannelTargetType::parse(&input.target_type) else {
            return Err(ChannelError::InvalidTargetType(input.target_type));
        };
        let target_value = normalize_target_value(target_type, &input.value)
            .ok_or_else(|| ChannelError::InvalidTargetValue(input.value.clone()))?;

        let channel = self.ensure_channel_exists(channel_id).await?;
        let existing_target = channel_target::Entity::find_by_id(target_id)
            .filter(channel_target::Column::ChannelId.eq(channel_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(target_id))?;

        let now = chrono::Utc::now().into();
        let txn = self.db.begin().await?;
        if input.is_primary {
            clear_primary_target(&txn, channel_id, Some(target_id), now).await?;
        }

        let mut active: channel_target::ActiveModel = existing_target.into();
        active.target_type = Set(target_type.as_str().to_string());
        active.value = Set(target_value.clone());
        active.is_primary = Set(input.is_primary);
        active.settings = Set(input.settings.unwrap_or_else(|| serde_json::json!({})));
        active.updated_at = Set(now);

        let updated = match active.update(&txn).await {
            Ok(model) => model,
            Err(error) if is_unique_violation(&error) => {
                txn.rollback().await?;
                return Err(
                    classify_target_conflict(
                        &self.db,
                        channel.tenant_id,
                        target_type,
                        target_value.as_str(),
                        input.is_primary,
                    )
                    .await,
                );
            }
            Err(error) => {
                txn.rollback().await?;
                return Err(ChannelError::Database(error));
            }
        };

        txn.commit().await?;
        Ok(to_channel_target_response(updated))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id, target_id = %target_id))]
    pub async fn delete_target(
        &self,
        channel_id: Uuid,
        target_id: Uuid,
    ) -> ChannelResult<ChannelTargetResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        if target_id.is_nil() {
            return Err(ChannelError::NotFound(target_id));
        }
        let target = channel_target::Entity::find_by_id(target_id)
            .filter(channel_target::Column::ChannelId.eq(channel_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(target_id))?;

        let response = to_channel_target_response(target);
        let deleted = channel_target::Entity::delete_many()
            .filter(channel_target::Column::Id.eq(target_id))
            .filter(channel_target::Column::ChannelId.eq(channel_id))
            .exec(&self.db)
            .await?;
        if deleted.rows_affected == 0 {
            return Err(ChannelError::NotFound(target_id));
        }
        Ok(response)
    }

    #[instrument(skip(self, input), fields(channel_id = %channel_id, module_slug = %input.module_slug))]
    pub async fn bind_module(
        &self,
        channel_id: Uuid,
        input: BindChannelModuleInput,
    ) -> ChannelResult<ChannelModuleBindingResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        self.ensure_channel_exists(channel_id).await?;

        let module_slug = input.module_slug.trim().to_ascii_lowercase();
        if module_slug.is_empty() {
            return Err(ChannelError::Validation(
                "module_slug must not be empty".to_string(),
            ));
        }
        if module_slug.chars().count() > CHANNEL_MODULE_SLUG_MAX_CHARS {
            return Err(ChannelError::Validation(format!(
                "module_slug must not exceed {CHANNEL_MODULE_SLUG_MAX_CHARS} characters"
            )));
        }

        // The binding is upserted in one statement: `idx_channel_module_bindings_unique`
        // makes the database the arbiter, so two concurrent bind requests cannot
        // both take the "row does not exist" branch and collide on insert. The
        // read-back runs in the same transaction, so a concurrent unbind cannot
        // delete the row between the upsert and the read and turn a successful
        // bind into a spurious storage error.
        let now = chrono::Utc::now().into();
        let settings = input.settings.unwrap_or_else(|| serde_json::json!({}));
        let txn = self.db.begin().await?;
        channel_module_binding::Entity::insert(ChannelModuleBindingActiveModel {
            id: Set(generate_id()),
            channel_id: Set(channel_id),
            module_slug: Set(module_slug.clone()),
            is_enabled: Set(input.is_enabled),
            settings: Set(settings),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                channel_module_binding::Column::ChannelId,
                channel_module_binding::Column::ModuleSlug,
            ])
            .update_columns([
                channel_module_binding::Column::IsEnabled,
                channel_module_binding::Column::Settings,
                channel_module_binding::Column::UpdatedAt,
            ])
            .to_owned(),
        )
        .exec_without_returning(&txn)
        .await?;

        // The upsert above either inserted or updated the row, so a missing row here
        // is an owner defect rather than a missing channel: report it as such
        // instead of answering `404` for a channel that does exist.
        let model = channel_module_binding::Entity::find()
            .filter(channel_module_binding::Column::ChannelId.eq(channel_id))
            .filter(channel_module_binding::Column::ModuleSlug.eq(&module_slug))
            .one(&txn)
            .await?
            .ok_or(ChannelError::Database(DbErr::RecordNotInserted))?;
        txn.commit().await?;

        Ok(to_channel_module_binding_response(model))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id, binding_id = %binding_id))]
    pub async fn remove_module_binding(
        &self,
        channel_id: Uuid,
        binding_id: Uuid,
    ) -> ChannelResult<ChannelModuleBindingResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        if binding_id.is_nil() {
            return Err(ChannelError::NotFound(binding_id));
        }
        // One scoped statement decides both the lookup and the delete, so the
        // response can only be built for a row this channel actually owned.
        let binding = channel_module_binding::Entity::find_by_id(binding_id)
            .filter(channel_module_binding::Column::ChannelId.eq(channel_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(binding_id))?;
        let response = to_channel_module_binding_response(binding);
        let deleted = channel_module_binding::Entity::delete_many()
            .filter(channel_module_binding::Column::Id.eq(binding_id))
            .filter(channel_module_binding::Column::ChannelId.eq(channel_id))
            .exec(&self.db)
            .await?;
        if deleted.rows_affected == 0 {
            return Err(ChannelError::NotFound(binding_id));
        }
        Ok(response)
    }

    #[instrument(skip(self, input), fields(channel_id = %channel_id, oauth_app_id = %input.oauth_app_id))]
    pub async fn bind_oauth_app(
        &self,
        channel_id: Uuid,
        input: BindChannelOauthAppInput,
    ) -> ChannelResult<ChannelOauthAppResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        self.ensure_channel_exists(channel_id).await?;

        if input.oauth_app_id.is_nil() {
            return Err(ChannelError::Validation(
                "oauth_app_id must not be nil".to_string(),
            ));
        }

        // Same upsert reasoning as `bind_module`: `idx_channel_oauth_apps_unique`
        // decides the winner, so a retried bind cannot create a duplicate row and
        // the stored role is always the last writer's value.
        let now = chrono::Utc::now().into();
        let txn = self.db.begin().await?;
        channel_oauth_app::Entity::insert(ChannelOauthAppActiveModel {
            id: Set(generate_id()),
            channel_id: Set(channel_id),
            oauth_app_id: Set(input.oauth_app_id),
            role: Set(input.role),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                channel_oauth_app::Column::ChannelId,
                channel_oauth_app::Column::OauthAppId,
            ])
            .update_columns([channel_oauth_app::Column::Role])
            .to_owned(),
        )
        .exec_without_returning(&txn)
        .await?;

        let model = channel_oauth_app::Entity::find()
            .filter(channel_oauth_app::Column::ChannelId.eq(channel_id))
            .filter(channel_oauth_app::Column::OauthAppId.eq(input.oauth_app_id))
            .one(&txn)
            .await?
            .ok_or(ChannelError::Database(DbErr::RecordNotInserted))?;
        txn.commit().await?;

        Ok(to_channel_oauth_app_response(model))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id, binding_id = %binding_id))]
    pub async fn revoke_oauth_app_binding(
        &self,
        channel_id: Uuid,
        binding_id: Uuid,
    ) -> ChannelResult<ChannelOauthAppResponse> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        if binding_id.is_nil() {
            return Err(ChannelError::NotFound(binding_id));
        }
        let binding = channel_oauth_app::Entity::find_by_id(binding_id)
            .filter(channel_oauth_app::Column::ChannelId.eq(channel_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(binding_id))?;
        let response = to_channel_oauth_app_response(binding);
        let deleted = channel_oauth_app::Entity::delete_many()
            .filter(channel_oauth_app::Column::Id.eq(binding_id))
            .filter(channel_oauth_app::Column::ChannelId.eq(channel_id))
            .exec(&self.db)
            .await?;
        if deleted.rows_affected == 0 {
            return Err(ChannelError::NotFound(binding_id));
        }
        Ok(response)
    }

    #[instrument(skip(self, input), fields(tenant_id = %input.tenant_id, slug = %input.slug))]
    pub async fn create_resolution_policy_set(
        &self,
        input: CreateChannelResolutionPolicySetInput,
    ) -> ChannelResult<ChannelResolutionPolicySetResponse> {
        if input.tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "policy set tenant_id must not be nil".to_string(),
            ));
        }
        let slug = input.slug.trim().to_ascii_lowercase();
        if slug.is_empty() {
            return Err(ChannelError::Validation(
                "policy set slug must not be empty".to_string(),
            ));
        }
        if slug.chars().count() > CHANNEL_POLICY_SET_SLUG_MAX_CHARS {
            return Err(ChannelError::Validation(format!(
                "policy set slug must not exceed {CHANNEL_POLICY_SET_SLUG_MAX_CHARS} characters"
            )));
        }
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(ChannelError::Validation(
                "policy set name must not be empty".to_string(),
            ));
        }
        if name.chars().count() > CHANNEL_NAME_MAX_CHARS {
            return Err(ChannelError::Validation(format!(
                "policy set name must not exceed {CHANNEL_NAME_MAX_CHARS} characters"
            )));
        }

        // Creating an already-active set and activating it must be one atomic
        // step: with two transactions a concurrent activation could leave the
        // tenant with no active set (or with the wrong one) in between.
        let now = chrono::Utc::now().into();
        let txn = self.db.begin().await?;
        let inserted = ChannelResolutionPolicySetActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            slug: Set(slug.clone()),
            name: Set(name),
            schema_version: Set(CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION),
            is_active: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await;

        let inserted = match inserted {
            Ok(model) => model,
            Err(error) if is_unique_violation(&error) => {
                txn.rollback().await?;
                return Err(ChannelError::PolicySetSlugAlreadyExists(slug));
            }
            Err(error) => {
                txn.rollback().await?;
                return Err(ChannelError::Database(error));
            }
        };

        if input.is_active {
            if let Err(error) = self
                .replace_active_policy_set_in_tx(&txn, inserted.tenant_id, inserted.id)
                .await
            {
                txn.rollback().await?;
                return Err(error);
            }
        }

        txn.commit().await?;
        self.get_resolution_policy_set(inserted.id).await
    }

    #[instrument(skip(self, input), fields(policy_set_id = %policy_set_id, priority = input.priority))]
    pub async fn create_resolution_rule(
        &self,
        policy_set_id: Uuid,
        input: CreateChannelResolutionRuleInput,
    ) -> ChannelResult<ChannelResolutionRuleResponse> {
        if policy_set_id.is_nil() {
            return Err(ChannelError::NotFound(policy_set_id));
        }
        let policy_set = channel_resolution_policy_set::Entity::find_by_id(policy_set_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(policy_set_id))?;
        let definition = input.definition.validated()?;
        let action_channel_id = definition.action_channel_id();
        let action_channel = channel::Entity::find_by_id(action_channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(action_channel_id))?;

        if action_channel.tenant_id != policy_set.tenant_id {
            return Err(ChannelError::InvalidPolicyDefinition(format!(
                "policy action channel '{action_channel_id}' does not belong to tenant '{}'",
                policy_set.tenant_id
            )));
        }

        let now = chrono::Utc::now().into();
        let model = ChannelResolutionPolicyRuleActiveModel {
            id: Set(generate_id()),
            policy_set_id: Set(policy_set_id),
            priority: Set(input.priority),
            is_active: Set(input.is_active),
            action_channel_id: Set(action_channel_id),
            definition: Set(serde_json::to_value(&definition)?),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        to_channel_resolution_rule_response(model)
    }

    #[instrument(skip(self, input), fields(policy_set_id = %policy_set_id, rule_id = %rule_id))]
    pub async fn update_resolution_rule(
        &self,
        policy_set_id: Uuid,
        rule_id: Uuid,
        input: UpdateChannelResolutionRuleInput,
    ) -> ChannelResult<ChannelResolutionRuleResponse> {
        if policy_set_id.is_nil() {
            return Err(ChannelError::NotFound(policy_set_id));
        }
        if rule_id.is_nil() {
            return Err(ChannelError::NotFound(rule_id));
        }
        let has_definition_patch = input.action_channel_id.is_some()
            || input.host_equals.is_some()
            || input.host_suffix.is_some()
            || input.oauth_app_id.is_some()
            || input.surface.is_some()
            || input.locale.is_some();

        if input.priority.is_none() && input.is_active.is_none() && !has_definition_patch {
            return Err(ChannelError::InvalidPolicyOperation(
                "provide at least one field to update (priority, is_active, action_channel_id, host_equals, host_suffix, oauth_app_id, surface, locale)".to_string(),
            ));
        }

        let policy_set = channel_resolution_policy_set::Entity::find_by_id(policy_set_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(policy_set_id))?;
        let rule = channel_resolution_policy_rule::Entity::find_by_id(rule_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(rule_id))?;
        if rule.policy_set_id != policy_set_id {
            return Err(ChannelError::NotFound(rule_id));
        }

        let mut next_definition =
            serde_json::from_value::<ChannelResolutionRuleDefinition>(rule.definition.clone())?
                .validated()?;

        if has_definition_patch {
            let mut host_equals =
                next_definition
                    .predicates
                    .iter()
                    .find_map(|predicate| match predicate {
                        ResolutionPredicate::HostEquals(value) => Some(value.clone()),
                        _ => None,
                    });
            let mut host_suffix =
                next_definition
                    .predicates
                    .iter()
                    .find_map(|predicate| match predicate {
                        ResolutionPredicate::HostSuffix(value) => Some(value.clone()),
                        _ => None,
                    });
            let mut oauth_app_id =
                next_definition
                    .predicates
                    .iter()
                    .find_map(|predicate| match predicate {
                        ResolutionPredicate::OAuthAppEquals(value) => Some(*value),
                        _ => None,
                    });
            let mut surface =
                next_definition
                    .predicates
                    .iter()
                    .find_map(|predicate| match predicate {
                        ResolutionPredicate::SurfaceIs(value) => Some(*value),
                        _ => None,
                    });
            let mut locale =
                next_definition
                    .predicates
                    .iter()
                    .find_map(|predicate| match predicate {
                        ResolutionPredicate::LocaleEquals(value) => Some(value.clone()),
                        _ => None,
                    });

            if let Some(channel_id) = input.action_channel_id {
                if channel_id.is_nil() {
                    return Err(ChannelError::InvalidPolicyDefinition(
                        "action_channel_id must not be nil".to_string(),
                    ));
                }
                let action_channel = channel::Entity::find_by_id(channel_id)
                    .one(&self.db)
                    .await?
                    .ok_or(ChannelError::NotFound(channel_id))?;
                if action_channel.tenant_id != policy_set.tenant_id {
                    return Err(ChannelError::InvalidPolicyDefinition(format!(
                        "policy action channel '{channel_id}' does not belong to tenant '{}'",
                        policy_set.tenant_id
                    )));
                }
                next_definition.action = ResolutionAction::ResolveToChannel { channel_id };
            }

            if let Some(host_value) = input.host_equals {
                host_equals = normalize_optional_patch_text(host_value);
            }
            if let Some(host_suffix_value) = input.host_suffix {
                host_suffix = normalize_optional_patch_text(host_suffix_value);
            }
            if let Some(oauth_app_value) = input.oauth_app_id {
                oauth_app_id = normalize_optional_patch_text(oauth_app_value)
                    .map(|value| {
                        let parsed = Uuid::parse_str(value.as_str()).map_err(|e| {
                            ChannelError::InvalidPolicyDefinition(format!(
                                "oauth_app_id must be a valid UUID: {}",
                                e
                            ))
                        })?;
                        if parsed.is_nil() {
                            return Err(ChannelError::InvalidPolicyDefinition(
                                "oauth_app_id must not be a nil UUID".to_string(),
                            ));
                        }
                        Ok(parsed)
                    })
                    .transpose()?;
            }
            if let Some(surface_value) = input.surface {
                surface = normalize_optional_patch_text(surface_value)
                    .map(|value| parse_target_surface(value.as_str()))
                    .transpose()?;
            }
            if let Some(locale_value) = input.locale {
                locale = normalize_optional_patch_text(locale_value);
            }

            let mut predicates = Vec::new();
            if let Some(value) = host_equals {
                predicates.push(ResolutionPredicate::HostEquals(value));
            }
            if let Some(value) = host_suffix {
                predicates.push(ResolutionPredicate::HostSuffix(value));
            }
            if let Some(value) = oauth_app_id {
                predicates.push(ResolutionPredicate::OAuthAppEquals(value));
            }
            if let Some(value) = surface {
                predicates.push(ResolutionPredicate::SurfaceIs(value));
            }
            if let Some(value) = locale {
                predicates.push(ResolutionPredicate::LocaleEquals(value));
            }
            next_definition.predicates = predicates;
            next_definition = next_definition.validated()?;
        }

        let now = chrono::Utc::now().into();
        let mut active: channel_resolution_policy_rule::ActiveModel = rule.into();
        if let Some(priority) = input.priority {
            active.priority = Set(priority);
        }
        if let Some(is_active) = input.is_active {
            active.is_active = Set(is_active);
        }
        if has_definition_patch {
            active.action_channel_id = Set(next_definition.action_channel_id());
            active.definition = Set(serde_json::to_value(&next_definition)?);
        }
        active.updated_at = Set(now);

        to_channel_resolution_rule_response(active.update(&self.db).await?)
    }

    #[instrument(skip(self, input), fields(policy_set_id = %policy_set_id))]
    pub async fn reorder_resolution_rules(
        &self,
        policy_set_id: Uuid,
        input: ReorderChannelResolutionRulesInput,
    ) -> ChannelResult<Vec<ChannelResolutionRuleResponse>> {
        if policy_set_id.is_nil() {
            return Err(ChannelError::NotFound(policy_set_id));
        }
        if input.rule_ids.is_empty() {
            return Err(ChannelError::InvalidPolicyOperation(
                "rule_ids must not be empty".to_string(),
            ));
        }
        for rule_id in &input.rule_ids {
            if rule_id.is_nil() {
                return Err(ChannelError::InvalidPolicyOperation(
                    "rule_ids must not contain nil UUIDs".to_string(),
                ));
            }
        }

        let rules = channel_resolution_policy_rule::Entity::find()
            .filter(channel_resolution_policy_rule::Column::PolicySetId.eq(policy_set_id))
            .order_by_asc(channel_resolution_policy_rule::Column::Priority)
            .order_by_asc(channel_resolution_policy_rule::Column::CreatedAt)
            .all(&self.db)
            .await?;
        if rules.is_empty() {
            return Err(ChannelError::NotFound(policy_set_id));
        }

        if rules.len() != input.rule_ids.len() {
            return Err(ChannelError::InvalidPolicyOperation(format!(
                "rule_ids length ({}) does not match policy-set rule count ({})",
                input.rule_ids.len(),
                rules.len()
            )));
        }

        let existing_ids = rules
            .iter()
            .map(|rule| rule.id)
            .collect::<std::collections::HashSet<_>>();
        let mut seen = std::collections::HashSet::new();
        for rule_id in &input.rule_ids {
            if !existing_ids.contains(rule_id) {
                return Err(ChannelError::InvalidPolicyOperation(format!(
                    "rule '{rule_id}' does not belong to policy set '{policy_set_id}'"
                )));
            }
            if !seen.insert(*rule_id) {
                return Err(ChannelError::InvalidPolicyOperation(format!(
                    "rule '{rule_id}' is duplicated in reorder payload"
                )));
            }
        }

        let now = chrono::Utc::now().into();
        let mut order_by_id = std::collections::HashMap::with_capacity(input.rule_ids.len());
        for (index, rule_id) in input.rule_ids.iter().enumerate() {
            order_by_id.insert(*rule_id, index as i32);
        }

        let txn = self.db.begin().await?;
        for rule in rules {
            let Some(position) = order_by_id.get(&rule.id).copied() else {
                return Err(ChannelError::InvalidPolicyOperation(format!(
                    "rule '{}' is missing from reorder payload",
                    rule.id
                )));
            };

            let new_priority = (position + 1) * 10;
            if rule.priority == new_priority {
                continue;
            }

            let mut active: channel_resolution_policy_rule::ActiveModel = rule.into();
            active.priority = Set(new_priority);
            active.updated_at = Set(now);
            active.update(&txn).await?;
        }
        txn.commit().await?;

        channel_resolution_policy_rule::Entity::find()
            .filter(channel_resolution_policy_rule::Column::PolicySetId.eq(policy_set_id))
            .order_by_asc(channel_resolution_policy_rule::Column::Priority)
            .order_by_asc(channel_resolution_policy_rule::Column::CreatedAt)
            .all(&self.db)
            .await?
            .into_iter()
            .map(to_channel_resolution_rule_response)
            .collect()
    }

    #[instrument(skip(self), fields(policy_set_id = %policy_set_id))]
    pub async fn set_active_resolution_policy_set(
        &self,
        policy_set_id: Uuid,
    ) -> ChannelResult<ChannelResolutionPolicySetResponse> {
        if policy_set_id.is_nil() {
            return Err(ChannelError::NotFound(policy_set_id));
        }
        let policy_set = channel_resolution_policy_set::Entity::find_by_id(policy_set_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(policy_set_id))?;
        self.replace_active_policy_set(policy_set.tenant_id, policy_set_id)
            .await?;
        self.get_resolution_policy_set(policy_set_id).await
    }

    pub async fn get_resolution_policy_set(
        &self,
        policy_set_id: Uuid,
    ) -> ChannelResult<ChannelResolutionPolicySetResponse> {
        if policy_set_id.is_nil() {
            return Err(ChannelError::NotFound(policy_set_id));
        }
        let model = channel_resolution_policy_set::Entity::find_by_id(policy_set_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(policy_set_id))?;
        Ok(to_channel_resolution_policy_set_response(model))
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, policy_set_id = %policy_set_id))]
    pub async fn get_resolution_policy_set_for_tenant(
        &self,
        tenant_id: Uuid,
        policy_set_id: Uuid,
    ) -> ChannelResult<ChannelResolutionPolicySetResponse> {
        if tenant_id.is_nil() || policy_set_id.is_nil() {
            return Err(ChannelError::NotFound(policy_set_id));
        }
        let model = channel_resolution_policy_set::Entity::find_by_id(policy_set_id)
            .filter(channel_resolution_policy_set::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(policy_set_id))?;
        Ok(to_channel_resolution_policy_set_response(model))
    }

    pub async fn list_resolution_policy_sets(
        &self,
        tenant_id: Uuid,
    ) -> ChannelResult<Vec<ChannelResolutionPolicySetDetailResponse>> {
        if tenant_id.is_nil() {
            return Err(ChannelError::Validation(
                "policy set tenant_id must not be nil".to_string(),
            ));
        }
        let policy_sets = channel_resolution_policy_set::Entity::find()
            .filter(channel_resolution_policy_set::Column::TenantId.eq(tenant_id))
            .order_by_desc(channel_resolution_policy_set::Column::IsActive)
            .order_by_asc(channel_resolution_policy_set::Column::CreatedAt)
            .all(&self.db)
            .await?;

        // One set-based rule query for the whole tenant instead of one per policy
        // set: the bootstrap read used to issue 1 + N queries.
        let policy_set_ids: Vec<Uuid> =
            policy_sets.iter().map(|policy_set| policy_set.id).collect();
        let mut rules_by_set: HashMap<Uuid, Vec<ChannelResolutionRuleResponse>> = HashMap::new();
        if !policy_set_ids.is_empty() {
            let rules = channel_resolution_policy_rule::Entity::find()
                .filter(channel_resolution_policy_rule::Column::PolicySetId.is_in(policy_set_ids))
                .order_by_asc(channel_resolution_policy_rule::Column::Priority)
                .order_by_asc(channel_resolution_policy_rule::Column::CreatedAt)
                .all(&self.db)
                .await?;
            for rule in rules {
                let policy_set_id = rule.policy_set_id;
                let response = to_channel_resolution_rule_response(rule)?;
                rules_by_set.entry(policy_set_id).or_default().push(response);
            }
        }

        let items = policy_sets
            .into_iter()
            .map(|policy_set| {
                let policy_set_id = policy_set.id;
                ChannelResolutionPolicySetDetailResponse {
                    policy_set: to_channel_resolution_policy_set_response(policy_set),
                    rules: rules_by_set.remove(&policy_set_id).unwrap_or_default(),
                }
            })
            .collect();

        Ok(items)
    }

    #[instrument(skip(self), fields(policy_set_id = %policy_set_id, rule_id = %rule_id))]
    pub async fn remove_resolution_rule(
        &self,
        policy_set_id: Uuid,
        rule_id: Uuid,
    ) -> ChannelResult<ChannelResolutionRuleResponse> {
        if policy_set_id.is_nil() {
            return Err(ChannelError::NotFound(policy_set_id));
        }
        if rule_id.is_nil() {
            return Err(ChannelError::NotFound(rule_id));
        }
        let rule = channel_resolution_policy_rule::Entity::find_by_id(rule_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(rule_id))?;
        if rule.policy_set_id != policy_set_id {
            return Err(ChannelError::NotFound(rule_id));
        }

        let response = to_channel_resolution_rule_response(rule)?;

        // The delete is scoped by the policy set as well as the rule id, and a
        // zero row count means a concurrent writer removed the rule between the
        // read and this statement. `ActiveModel::delete()` reported that race as
        // `DbErr::RecordNotDeleted`, which surfaced as a `500`; a lost race is a
        // `404`, the same answer as a rule that never existed.
        let deleted = channel_resolution_policy_rule::Entity::delete_many()
            .filter(channel_resolution_policy_rule::Column::Id.eq(rule_id))
            .filter(channel_resolution_policy_rule::Column::PolicySetId.eq(policy_set_id))
            .exec(&self.db)
            .await?;
        if deleted.rows_affected == 0 {
            return Err(ChannelError::NotFound(rule_id));
        }

        Ok(response)
    }

    pub async fn list_active_resolution_rules(
        &self,
        tenant_id: Uuid,
    ) -> ChannelResult<Vec<StoredChannelResolutionRule>> {
        if tenant_id.is_nil() {
            return Ok(Vec::new());
        }
        let rows = channel_resolution_policy_rule::Entity::find()
            .filter(channel_resolution_policy_rule::Column::IsActive.eq(true))
            .find_also_related(channel_resolution_policy_set::Entity)
            .filter(channel_resolution_policy_set::Column::TenantId.eq(tenant_id))
            .filter(channel_resolution_policy_set::Column::IsActive.eq(true))
            .order_by_asc(channel_resolution_policy_rule::Column::Priority)
            .order_by_asc(channel_resolution_policy_rule::Column::CreatedAt)
            .all(&self.db)
            .await?;

        rows.into_iter()
            .map(|(rule, maybe_policy_set)| {
                let policy_set = maybe_policy_set.ok_or_else(|| {
                    ChannelError::InvalidPolicyDefinition(format!(
                        "policy rule '{}' is missing its policy set",
                        rule.id
                    ))
                })?;
                Ok(StoredChannelResolutionRule {
                    id: rule.id,
                    policy_set_id: rule.policy_set_id,
                    policy_set_slug: policy_set.slug,
                    policy_set_name: policy_set.name,
                    priority: rule.priority,
                    action_channel_id: rule.action_channel_id,
                    definition: serde_json::from_value::<ChannelResolutionRuleDefinition>(
                        rule.definition,
                    )?
                    .validated()?,
                })
            })
            .collect()
    }

    async fn ensure_channel_exists(&self, channel_id: Uuid) -> ChannelResult<channel::Model> {
        if channel_id.is_nil() {
            return Err(ChannelError::NotFound(channel_id));
        }
        channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))
    }

    async fn default_channel_exists_for_tenant(&self, tenant_id: Uuid) -> ChannelResult<bool> {
        let existing = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::IsActive.eq(true))
            .filter(channel::Column::IsDefault.eq(true))
            .one(&self.db)
            .await?;
        Ok(existing.is_some())
    }

    /// Moves the tenant default pointer in one transaction.
    ///
    /// The clear step runs before the set step because the partial unique index
    /// `uq_channels_tenant_default` rejects a second default row immediately, and
    /// a single `UPDATE ... SET is_default = (id = $1)` can visit rows in an order
    /// that trips it.
    async fn replace_default_channel(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
    ) -> ChannelResult<()> {
        let now = chrono::Utc::now().into();
        let txn = self.db.begin().await?;

        channel::Entity::update_many()
            .col_expr(channel::Column::IsDefault, Expr::value(false))
            .col_expr(channel::Column::UpdatedAt, Expr::value(now))
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Id.ne(channel_id))
            .filter(channel::Column::IsDefault.eq(true))
            .exec(&txn)
            .await?;

        let updated = channel::Entity::update_many()
            .col_expr(channel::Column::IsDefault, Expr::value(true))
            .col_expr(channel::Column::UpdatedAt, Expr::value(now))
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Id.eq(channel_id))
            .exec(&txn)
            .await
            // `uq_channels_tenant_default` decides the winner when two promotions
            // race; the loser must retry, not receive a `500`.
            .map_err(|error| map_unique_violation(error, "promote the tenant default channel"))?;
        if updated.rows_affected == 0 {
            txn.rollback().await?;
            return Err(ChannelError::NotFound(channel_id));
        }

        txn.commit().await?;
        Ok(())
    }

    /// Moves the tenant's active policy-set pointer in one transaction.
    ///
    /// `uq_channel_resolution_policy_sets_tenant_active` allows exactly one active
    /// set per tenant, so the clear step must complete before the set step.
    async fn replace_active_policy_set(
        &self,
        tenant_id: Uuid,
        policy_set_id: Uuid,
    ) -> ChannelResult<()> {
        let txn = self.db.begin().await?;
        self.replace_active_policy_set_in_tx(&txn, tenant_id, policy_set_id)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn replace_active_policy_set_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        policy_set_id: Uuid,
    ) -> ChannelResult<()> {
        let now = chrono::Utc::now().into();

        channel_resolution_policy_set::Entity::update_many()
            .col_expr(
                channel_resolution_policy_set::Column::IsActive,
                Expr::value(false),
            )
            .col_expr(
                channel_resolution_policy_set::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(channel_resolution_policy_set::Column::TenantId.eq(tenant_id))
            .filter(channel_resolution_policy_set::Column::Id.ne(policy_set_id))
            .filter(channel_resolution_policy_set::Column::IsActive.eq(true))
            .exec(txn)
            .await?;

        let updated = channel_resolution_policy_set::Entity::update_many()
            .col_expr(
                channel_resolution_policy_set::Column::IsActive,
                Expr::value(true),
            )
            .col_expr(
                channel_resolution_policy_set::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(channel_resolution_policy_set::Column::TenantId.eq(tenant_id))
            .filter(channel_resolution_policy_set::Column::Id.eq(policy_set_id))
            .exec(txn)
            .await
            // `uq_channel_resolution_policy_sets_tenant_active` decides the winner
            // when two activations race: the loser is retryable, not a server fault.
            .map_err(|error| {
                map_unique_violation(error, "activate the tenant resolution policy set")
            })?;
        if updated.rows_affected == 0 {
            return Err(ChannelError::NotFound(policy_set_id));
        }

        Ok(())
    }

    /// Loads complete channel details for a set of channels.
    ///
    /// Details are assembled with three set-based queries (targets, module
    /// bindings, OAuth app bindings) instead of three queries per channel, so the
    /// admin bootstrap and the tenant channel list stay O(1) in round trips
    /// instead of 3N+1.
    async fn build_channel_details(
        &self,
        channels: Vec<channel::Model>,
    ) -> ChannelResult<Vec<ChannelDetailResponse>> {
        if channels.is_empty() {
            return Ok(Vec::new());
        }

        let channel_ids: Vec<Uuid> = channels.iter().map(|channel| channel.id).collect();
        let targets = channel_target::Entity::find()
            .filter(channel_target::Column::ChannelId.is_in(channel_ids.clone()))
            .order_by_desc(channel_target::Column::IsPrimary)
            .order_by_asc(channel_target::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let module_bindings = channel_module_binding::Entity::find()
            .filter(channel_module_binding::Column::ChannelId.is_in(channel_ids.clone()))
            .order_by_asc(channel_module_binding::Column::ModuleSlug)
            .all(&self.db)
            .await?;
        let oauth_apps = channel_oauth_app::Entity::find()
            .filter(channel_oauth_app::Column::ChannelId.is_in(channel_ids))
            .order_by_asc(channel_oauth_app::Column::CreatedAt)
            .all(&self.db)
            .await?;

        let mut targets_by_channel: HashMap<Uuid, Vec<ChannelTargetResponse>> = HashMap::new();
        for target in targets {
            targets_by_channel
                .entry(target.channel_id)
                .or_default()
                .push(to_channel_target_response(target));
        }
        let mut modules_by_channel: HashMap<Uuid, Vec<ChannelModuleBindingResponse>> =
            HashMap::new();
        for binding in module_bindings {
            modules_by_channel
                .entry(binding.channel_id)
                .or_default()
                .push(to_channel_module_binding_response(binding));
        }
        let mut oauth_apps_by_channel: HashMap<Uuid, Vec<ChannelOauthAppResponse>> = HashMap::new();
        for binding in oauth_apps {
            oauth_apps_by_channel
                .entry(binding.channel_id)
                .or_default()
                .push(to_channel_oauth_app_response(binding));
        }

        Ok(channels
            .into_iter()
            .map(|model| {
                let id = model.id;
                ChannelDetailResponse {
                    channel: to_channel_response(model),
                    targets: targets_by_channel.remove(&id).unwrap_or_default(),
                    module_bindings: modules_by_channel.remove(&id).unwrap_or_default(),
                    oauth_apps: oauth_apps_by_channel.remove(&id).unwrap_or_default(),
                }
            })
            .collect())
    }

    async fn build_channel_detail(
        &self,
        channel_model: channel::Model,
    ) -> ChannelResult<ChannelDetailResponse> {
        let channel_id = channel_model.id;
        self.build_channel_details(vec![channel_model])
            .await?
            .pop()
            .ok_or(ChannelError::NotFound(channel_id))
    }
}

fn to_channel_response(model: channel::Model) -> ChannelResponse {
    ChannelResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        slug: model.slug,
        name: model.name,
        is_active: model.is_active,
        is_default: model.is_default,
        status: crate::ChannelStatus::from_is_active(model.is_active),
        settings: model.settings,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

fn to_channel_target_response(model: channel_target::Model) -> ChannelTargetResponse {
    ChannelTargetResponse {
        id: model.id,
        channel_id: model.channel_id,
        target_type: model.target_type,
        value: model.value,
        is_primary: model.is_primary,
        settings: model.settings,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

fn to_channel_module_binding_response(
    model: channel_module_binding::Model,
) -> ChannelModuleBindingResponse {
    ChannelModuleBindingResponse {
        id: model.id,
        channel_id: model.channel_id,
        module_slug: model.module_slug,
        is_enabled: model.is_enabled,
        settings: model.settings,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

fn to_channel_oauth_app_response(model: channel_oauth_app::Model) -> ChannelOauthAppResponse {
    ChannelOauthAppResponse {
        id: model.id,
        channel_id: model.channel_id,
        oauth_app_id: model.oauth_app_id,
        role: model.role,
        created_at: model.created_at.into(),
    }
}

fn to_channel_resolution_policy_set_response(
    model: channel_resolution_policy_set::Model,
) -> ChannelResolutionPolicySetResponse {
    ChannelResolutionPolicySetResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        slug: model.slug,
        name: model.name,
        schema_version: model.schema_version,
        is_active: model.is_active,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

fn to_channel_resolution_rule_response(
    model: channel_resolution_policy_rule::Model,
) -> ChannelResult<ChannelResolutionRuleResponse> {
    Ok(ChannelResolutionRuleResponse {
        id: model.id,
        policy_set_id: model.policy_set_id,
        priority: model.priority,
        is_active: model.is_active,
        action_channel_id: model.action_channel_id,
        definition: serde_json::from_value::<ChannelResolutionRuleDefinition>(model.definition)?
            .validated()?,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    })
}

/// Clears the per-channel primary target flag inside the caller's transaction.
///
/// `uq_channel_targets_channel_primary` allows only one primary target per
/// channel, so the previous primary must be demoted in the same transaction that
/// promotes the new one.
async fn clear_primary_target(
    txn: &DatabaseTransaction,
    channel_id: Uuid,
    keep_target_id: Option<Uuid>,
    now: sea_orm::prelude::DateTimeWithTimeZone,
) -> ChannelResult<()> {
    let mut update = channel_target::Entity::update_many()
        .col_expr(channel_target::Column::IsPrimary, Expr::value(false))
        .col_expr(channel_target::Column::UpdatedAt, Expr::value(now))
        .filter(channel_target::Column::ChannelId.eq(channel_id))
        .filter(channel_target::Column::IsPrimary.eq(true));
    if let Some(keep_target_id) = keep_target_id {
        update = update.filter(channel_target::Column::Id.ne(keep_target_id));
    }
    update.exec(txn).await?;
    Ok(())
}

/// Normalize and validate a channel slug.
///
/// The channel slug is immutable and is the identity consumers persist, so the
/// owner validates it in exactly one place.
fn normalize_channel_slug(raw: &str) -> ChannelResult<String> {
    let slug = raw.trim().to_ascii_lowercase();
    if slug.is_empty() {
        return Err(ChannelError::Validation(
            "channel slug must not be empty".to_string(),
        ));
    }
    if slug.chars().count() > CHANNEL_SLUG_MAX_CHARS {
        return Err(ChannelError::Validation(format!(
            "channel slug must not exceed {CHANNEL_SLUG_MAX_CHARS} characters"
        )));
    }
    Ok(slug)
}

/// Validates the shape of the open-ended channel `settings` bag.
///
/// Channel settings carry module-owned values, so the owner does not validate
/// their content, but every consumer reads the field as a JSON object: an array
/// or a bare scalar would only fail later, at read time, in whichever module
/// happened to fetch the channel first. The size bound keeps a single JSONB
/// document from becoming an unbounded write.
fn normalize_channel_settings(settings: serde_json::Value) -> ChannelResult<serde_json::Value> {
    if !settings.is_object() {
        return Err(ChannelError::Validation(
            "channel settings must be a JSON object".to_string(),
        ));
    }
    let size = serde_json::to_vec(&settings)?.len();
    if size > CHANNEL_SETTINGS_MAX_BYTES {
        return Err(ChannelError::Validation(format!(
            "channel settings must not exceed {CHANNEL_SETTINGS_MAX_BYTES} bytes"
        )));
    }
    Ok(settings)
}

fn normalize_channel_name(raw: &str) -> ChannelResult<String> {
    let name = raw.trim().to_string();
    if name.is_empty() {
        return Err(ChannelError::Validation(
            "channel name must not be empty".to_string(),
        ));
    }
    if name.chars().count() > CHANNEL_NAME_MAX_CHARS {
        return Err(ChannelError::Validation(format!(
            "channel name must not exceed {CHANNEL_NAME_MAX_CHARS} characters"
        )));
    }
    Ok(name)
}

/// Detects a database unique-constraint violation.
///
/// PostgreSQL reports SQLSTATE `23505`, SQLite reports
/// `UNIQUE constraint failed`, and sea-orm surfaces both as an opaque
/// `DbErr::Exec`/`DbErr::Query`, so the classification has to inspect the message.
fn is_unique_violation(error: &DbErr) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("23505")
        || message.contains("unique constraint")
        || message.contains("duplicate key value")
        || message.contains("unique_violation")
}

fn is_channel_slug_conflict(error: &DbErr) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("idx_channels_tenant_slug")
        || (message.contains("slug") && message.contains("channels"))
}

/// Classifies a unique violation on a target write by inspecting the state the
/// write collided with instead of matching index names in the driver message.
///
/// Two partial unique indexes can reject the same statement:
/// `uq_channel_targets_tenant_host` (another channel of the tenant already claims
/// the web host — a genuine duplicate) and `uq_channel_targets_channel_primary`
/// (another target of this channel became primary concurrently — a lost race the
/// caller may retry). PostgreSQL names the index for both, SQLite names only
/// columns, so the collision is decided by querying the tenant-scoped state.
async fn classify_target_conflict(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    target_type: ChannelTargetType,
    target_value: &str,
    requested_primary: bool,
) -> ChannelError {
    match channel_target::Entity::find()
        .filter(channel_target::Column::TenantId.eq(tenant_id))
        .filter(channel_target::Column::TargetType.eq(target_type.as_str()))
        .filter(channel_target::Column::Value.eq(target_value))
        .one(db)
        .await
    {
        // Only `uq_channel_targets_tenant_host` enforces value uniqueness, and only
        // for web hosts: every other target type may legitimately repeat a value on
        // another channel of the tenant, so a collision there can only be the
        // primary slot and must not be reported as a duplicate.
        Ok(Some(_)) if target_type == ChannelTargetType::WebDomain => {
            ChannelError::TargetAlreadyExists(
                target_type.as_str().to_string(),
                target_value.to_string(),
            )
        }
        Ok(_) if requested_primary => ChannelError::Conflict(
            "promote target to primary: a concurrent write won the primary slot; retry the operation"
                .to_string(),
        ),
        Ok(_) => ChannelError::Conflict(
            "write channel target: a concurrent write won the race; retry the operation".to_string(),
        ),
        Err(error) => ChannelError::Database(error),
    }
}

/// Maps an unexpected unique violation on an invariant write to a retryable
/// conflict instead of leaking a `500` for a race the caller can repeat.
fn map_unique_violation(error: DbErr, operation: &str) -> ChannelError {
    if is_unique_violation(&error) {
        ChannelError::Conflict(format!(
            "could not {operation}: a concurrent write won the race; retry the operation"
        ))
    } else {
        ChannelError::Database(error)
    }
}

/// Normalize a target value, rejecting anything the `channel_targets.value`
/// column cannot store.
///
/// Only the web-domain branch is bounded by hostname rules; every other target
/// type (mobile app, API client, embedded, external) is free-form text that went
/// straight into a `varchar(500)` column, so an over-long value surfaced as a
/// database error instead of a rejected input.
fn normalize_target_value(target_type: ChannelTargetType, raw: &str) -> Option<String> {
    target_type
        .normalize_value(raw)
        .filter(|value| value.chars().count() <= CHANNEL_TARGET_VALUE_MAX_CHARS)
}

fn normalize_optional_patch_text(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_target_surface(raw: &str) -> ChannelResult<TargetSurface> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "http" => Ok(TargetSurface::Http),
        other => Err(ChannelError::InvalidPolicyDefinition(format!(
            "unsupported surface `{other}`; only `http` is currently supported"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::ChannelService;
    use crate::dto::{
        CreateChannelInput, CreateChannelResolutionPolicySetInput,
        CreateChannelResolutionRuleInput, CreateChannelTargetInput,
        ReorderChannelResolutionRulesInput, UpdateChannelResolutionRuleInput,
        UpdateChannelTargetInput,
    };
    use crate::migrations;
    use crate::{
        ChannelError, ChannelResolutionRuleDefinition, ResolutionAction, ResolutionPredicate,
    };
    use rustok_test_utils::setup_test_db;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    async fn setup_channel_db() -> DatabaseConnection {
        let db = setup_test_db().await;
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE tenants (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                domain TEXT NULL UNIQUE,
                settings TEXT NOT NULL DEFAULT '{}',
                default_locale TEXT NOT NULL DEFAULT 'en',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ))
        .await
        .expect("tenants table should exist for channel foreign keys");
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE oauth_apps (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                app_type TEXT NOT NULL DEFAULT 'machine',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ))
        .await
        .expect("oauth_apps table should exist for channel foreign keys");
        let manager = SchemaManager::new(&db);
        for migration in migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("channel migration should apply");
        }
        db
    }

    async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
        db.execute_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                tenant_id.into(),
                format!("{slug} tenant").into(),
                slug.to_string().into(),
                "{}".to_string().into(),
                "en".to_string().into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");
    }

    async fn create_channel(service: &ChannelService, tenant_id: Uuid, slug: &str) -> Uuid {
        service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: slug.to_string(),
                name: slug.to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created")
            .id
    }

    #[tokio::test]
    async fn first_channel_becomes_explicit_default() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);

        let first_channel_id = create_channel(&service, tenant_id, "web").await;
        let second_channel_id = create_channel(&service, tenant_id, "blog").await;

        let first = service
            .get_channel(first_channel_id)
            .await
            .expect("first channel should load");
        let second = service
            .get_channel(second_channel_id)
            .await
            .expect("second channel should load");
        let default = service
            .get_default_channel(tenant_id)
            .await
            .expect("default channel should load")
            .expect("default channel should exist");

        assert!(first.is_default, "first channel should become default");
        assert!(
            !second.is_default,
            "subsequent channels must not auto-steal default"
        );
        assert_eq!(default.channel.id, first_channel_id);
    }

    #[tokio::test]
    async fn set_default_channel_reassigns_tenant_default() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);

        let first_channel_id = create_channel(&service, tenant_id, "web").await;
        let second_channel_id = create_channel(&service, tenant_id, "blog").await;

        let updated = service
            .set_default_channel(second_channel_id)
            .await
            .expect("default channel should be reassigned");
        let first = service
            .get_channel(first_channel_id)
            .await
            .expect("first channel should load");
        let second = service
            .get_channel(second_channel_id)
            .await
            .expect("second channel should load");
        let listed = service
            .list_channel_details(tenant_id)
            .await
            .expect("channel list should load");
        let default = service
            .get_default_channel(tenant_id)
            .await
            .expect("default channel should load")
            .expect("default channel should exist");

        assert_eq!(updated.id, second_channel_id);
        assert!(updated.is_default);
        assert!(!first.is_default, "old default must be cleared");
        assert!(second.is_default, "new default must be marked");
        assert_eq!(listed[0].channel.id, second_channel_id);
        assert_eq!(default.channel.id, second_channel_id);
    }

    #[tokio::test]
    async fn rejects_slug_beyond_the_column_width_and_counts_characters() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "limits").await;
        let service = ChannelService::new(db);

        // `channels.slug` is varchar(100): 101 characters must be rejected here
        // rather than reaching the insert and failing as a database error.
        let error = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "a".repeat(101),
                name: "Too long".to_string(),
                settings: None,
            })
            .await
            .expect_err("a slug beyond the column width must be rejected");
        assert!(matches!(error, ChannelError::Validation(_)), "{error:?}");

        // Exactly at the limit is accepted: the check must not be off by one.
        service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "a".repeat(100),
                name: "At limit".to_string(),
                settings: None,
            })
            .await
            .expect("a slug of exactly 100 characters must be accepted");

        // `channels.name` is varchar(255) counted in characters, so a 200-char
        // Cyrillic name (400 bytes) is valid and must not be rejected on byte
        // length alone.
        let multi_byte_name = "я".repeat(200);
        assert_eq!(
            multi_byte_name.len(),
            400,
            "test premise: two bytes per Cyrillic character"
        );
        service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "cyrillic".to_string(),
                name: multi_byte_name,
                settings: None,
            })
            .await
            .expect("a 200-character multi-byte name must be accepted");

        // One character past the name width is still rejected.
        let error = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "too-many-chars".to_string(),
                name: "я".repeat(256),
                settings: None,
            })
            .await
            .expect_err("a name beyond 255 characters must be rejected");
        assert!(matches!(error, ChannelError::Validation(_)), "{error:?}");
    }

    #[tokio::test]
    async fn rejects_unknown_target_type() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);
        let channel_id = create_channel(&service, tenant_id, "web").await;

        let error = service
            .add_target(
                channel_id,
                CreateChannelTargetInput {
                    target_type: "desktop_app".to_string(),
                    value: "desktop".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect_err("unknown target type must be rejected");

        assert!(matches!(error, ChannelError::InvalidTargetType(_)));
    }

    #[tokio::test]
    async fn normalizes_web_domain_and_ignores_non_web_targets_for_host_lookup() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);
        let web_channel_id = create_channel(&service, tenant_id, "web").await;
        let mobile_channel_id = create_channel(&service, tenant_id, "mobile").await;

        service
            .add_target(
                web_channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: " https://Example.TEST:443/ ".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("web target should be accepted");

        service
            .add_target(
                mobile_channel_id,
                CreateChannelTargetInput {
                    target_type: "mobile_app".to_string(),
                    value: "example.test".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("mobile target should be accepted");

        let detail = service
            .get_channel_by_host_target_value(tenant_id, "example.test")
            .await
            .expect("host lookup should succeed")
            .expect("web channel must be resolved");

        assert_eq!(detail.channel.id, web_channel_id);
        assert_eq!(detail.targets[0].value, "example.test");
    }

    #[tokio::test]
    async fn rejects_invalid_web_domain_target_value() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);
        let channel_id = create_channel(&service, tenant_id, "web").await;

        let error = service
            .add_target(
                channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: "bad host".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect_err("invalid web domain must be rejected");

        assert!(matches!(error, ChannelError::InvalidTargetValue(_)));
    }

    #[tokio::test]
    async fn rejects_duplicate_web_domain_within_same_tenant() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);
        let first_channel_id = create_channel(&service, tenant_id, "web").await;
        let second_channel_id = create_channel(&service, tenant_id, "blog").await;

        service
            .add_target(
                first_channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: "example.test".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("first web target should be accepted");

        let error = service
            .add_target(
                second_channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: "EXAMPLE.TEST".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect_err("duplicate host target must be rejected");

        assert!(matches!(error, ChannelError::TargetAlreadyExists(_, _)));
    }

    #[tokio::test]
    async fn updates_target_and_promotes_it_to_primary() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);
        let channel_id = create_channel(&service, tenant_id, "web").await;

        let first = service
            .add_target(
                channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: "example.test".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("first target should be created");
        let second = service
            .add_target(
                channel_id,
                CreateChannelTargetInput {
                    target_type: "mobile_app".to_string(),
                    value: "com.example.app".to_string(),
                    is_primary: false,
                    settings: None,
                },
            )
            .await
            .expect("second target should be created");

        let updated = service
            .update_target(
                channel_id,
                second.id,
                UpdateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: "blog.example.test".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("target should be updated");

        assert_eq!(updated.target_type, "web_domain");
        assert_eq!(updated.value, "blog.example.test");
        assert!(updated.is_primary);

        let detail = service
            .get_default_channel(tenant_id)
            .await
            .expect("channel detail should load")
            .expect("channel should exist");
        let first_after = detail
            .targets
            .iter()
            .find(|target| target.id == first.id)
            .expect("first target should still exist");
        assert!(
            !first_after.is_primary,
            "previous primary target must be demoted"
        );
    }

    #[tokio::test]
    async fn deletes_target_and_bindings() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);
        let channel_id = create_channel(&service, tenant_id, "web").await;

        let target = service
            .add_target(
                channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: "example.test".to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("target should be created");
        let module_binding = service
            .bind_module(
                channel_id,
                crate::dto::BindChannelModuleInput {
                    module_slug: "pages".to_string(),
                    is_enabled: false,
                    settings: None,
                },
            )
            .await
            .expect("module binding should be created");
        let oauth_binding = service
            .bind_oauth_app(
                channel_id,
                crate::dto::BindChannelOauthAppInput {
                    oauth_app_id: seed_oauth_app(&service.db, tenant_id, "storefront-app").await,
                    role: Some("storefront".to_string()),
                },
            )
            .await
            .expect("oauth binding should be created");

        service
            .delete_target(channel_id, target.id)
            .await
            .expect("target should be deleted");
        service
            .remove_module_binding(channel_id, module_binding.id)
            .await
            .expect("module binding should be deleted");
        service
            .revoke_oauth_app_binding(channel_id, oauth_binding.id)
            .await
            .expect("oauth binding should be deleted");

        let detail = service
            .get_default_channel(tenant_id)
            .await
            .expect("channel detail should load")
            .expect("channel should exist");
        assert!(detail.targets.is_empty());
        assert!(detail.module_bindings.is_empty());
        assert!(detail.oauth_apps.is_empty());
    }

    #[tokio::test]
    async fn creates_resolution_policy_set_and_rule_with_validated_definition() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());
        let channel_id = create_channel(&service, tenant_id, "policy-target").await;

        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");
        let rule = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![
                            ResolutionPredicate::HostSuffix(" SHOP.EXAMPLE.TEST ".to_string()),
                            ResolutionPredicate::LocaleEquals(" RU_BY ".to_string()),
                        ],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("policy rule should be created");
        let listed = service
            .list_resolution_policy_sets(tenant_id)
            .await
            .expect("policy sets should load");
        let active_rules = service
            .list_active_resolution_rules(tenant_id)
            .await
            .expect("active policy rules should load");

        assert!(policy_set.is_active);
        assert_eq!(rule.action_channel_id, channel_id);
        assert_eq!(
            rule.definition.predicates,
            vec![
                ResolutionPredicate::HostSuffix("shop.example.test".to_string()),
                ResolutionPredicate::LocaleEquals("ru-by".to_string()),
            ]
        );
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].rules.len(), 1);
        assert_eq!(active_rules.len(), 1);
        assert_eq!(active_rules[0].policy_set_slug, "default");
    }

    #[tokio::test]
    async fn rejects_policy_rule_that_targets_channel_from_another_tenant() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        let other_tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        seed_tenant(&db, other_tenant_id, "other").await;
        let service = ChannelService::new(db.clone());
        let foreign_channel_id = create_channel(&service, other_tenant_id, "foreign").await;
        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");

        let error = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::SurfaceIs(
                            crate::TargetSurface::Http,
                        )],
                        action: ResolutionAction::ResolveToChannel {
                            channel_id: foreign_channel_id,
                        },
                    },
                },
            )
            .await
            .expect_err("cross-tenant channel should be rejected");

        assert!(matches!(error, ChannelError::InvalidPolicyDefinition(_)));
    }

    #[tokio::test]
    async fn updates_resolution_rule_priority_and_active_state() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());
        let channel_id = create_channel(&service, tenant_id, "policy-target").await;

        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");
        let rule = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::SurfaceIs(
                            crate::TargetSurface::Http,
                        )],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("policy rule should be created");

        let updated = service
            .update_resolution_rule(
                policy_set.id,
                rule.id,
                UpdateChannelResolutionRuleInput {
                    priority: Some(40),
                    is_active: Some(false),
                    action_channel_id: None,
                    host_equals: None,
                    host_suffix: None,
                    oauth_app_id: None,
                    surface: None,
                    locale: None,
                },
            )
            .await
            .expect("policy rule should be updated");
        let active_rules = service
            .list_active_resolution_rules(tenant_id)
            .await
            .expect("active rules should load");

        assert_eq!(updated.priority, 40);
        assert!(!updated.is_active);
        assert!(active_rules.is_empty());
    }

    #[tokio::test]
    async fn updates_resolution_rule_definition_and_action_channel() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());
        let first_channel_id = create_channel(&service, tenant_id, "policy-target-a").await;
        let second_channel_id = create_channel(&service, tenant_id, "policy-target-b").await;
        let oauth_app_id = seed_oauth_app(&db, tenant_id, "channel-app").await;

        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");
        let rule = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![
                            ResolutionPredicate::HostSuffix("example.test".to_string()),
                            ResolutionPredicate::LocaleEquals("ru-by".to_string()),
                        ],
                        action: ResolutionAction::ResolveToChannel {
                            channel_id: first_channel_id,
                        },
                    },
                },
            )
            .await
            .expect("rule should be created");

        let updated = service
            .update_resolution_rule(
                policy_set.id,
                rule.id,
                UpdateChannelResolutionRuleInput {
                    priority: None,
                    is_active: Some(false),
                    action_channel_id: Some(second_channel_id),
                    host_equals: Some(" SHOP.EXAMPLE.TEST ".to_string()),
                    host_suffix: Some(" ".to_string()),
                    oauth_app_id: Some(oauth_app_id.to_string()),
                    surface: Some(" HTTP ".to_string()),
                    locale: Some(" EN_US ".to_string()),
                },
            )
            .await
            .expect("rule should be updated");

        assert!(!updated.is_active);
        assert_eq!(updated.action_channel_id, second_channel_id);
        assert_eq!(
            updated.definition.predicates,
            vec![
                ResolutionPredicate::HostEquals("shop.example.test".to_string()),
                ResolutionPredicate::OAuthAppEquals(oauth_app_id),
                ResolutionPredicate::SurfaceIs(crate::TargetSurface::Http),
                ResolutionPredicate::LocaleEquals("en-us".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn rejects_resolution_rule_action_channel_update_for_other_tenant() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        let other_tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        seed_tenant(&db, other_tenant_id, "other").await;
        let service = ChannelService::new(db.clone());
        let channel_id = create_channel(&service, tenant_id, "policy-target").await;
        let foreign_channel_id = create_channel(&service, other_tenant_id, "foreign-target").await;

        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");
        let rule = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::SurfaceIs(
                            crate::TargetSurface::Http,
                        )],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("rule should be created");

        let error = service
            .update_resolution_rule(
                policy_set.id,
                rule.id,
                UpdateChannelResolutionRuleInput {
                    priority: None,
                    is_active: None,
                    action_channel_id: Some(foreign_channel_id),
                    host_equals: None,
                    host_suffix: None,
                    oauth_app_id: None,
                    surface: None,
                    locale: None,
                },
            )
            .await
            .expect_err("cross-tenant action channel update should be rejected");

        assert!(matches!(error, ChannelError::InvalidPolicyDefinition(_)));
    }

    #[tokio::test]
    async fn rejects_resolution_rule_update_with_invalid_surface_patch() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());
        let channel_id = create_channel(&service, tenant_id, "policy-target").await;

        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");
        let rule = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::SurfaceIs(
                            crate::TargetSurface::Http,
                        )],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("rule should be created");

        let error = service
            .update_resolution_rule(
                policy_set.id,
                rule.id,
                UpdateChannelResolutionRuleInput {
                    priority: None,
                    is_active: None,
                    action_channel_id: None,
                    host_equals: None,
                    host_suffix: None,
                    oauth_app_id: None,
                    surface: Some("grpc".to_string()),
                    locale: None,
                },
            )
            .await
            .expect_err("invalid surface patch should be rejected");

        assert!(matches!(error, ChannelError::InvalidPolicyDefinition(_)));
    }

    #[tokio::test]
    async fn reorders_resolution_rules_inside_policy_set() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());
        let channel_id = create_channel(&service, tenant_id, "policy-target").await;

        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");

        let first = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::HostEquals(
                            "shop.example.test".to_string(),
                        )],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("first rule should be created");
        let second = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 20,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::HostSuffix(
                            "example.test".to_string(),
                        )],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("second rule should be created");
        let third = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 30,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::LocaleEquals("ru-by".to_string())],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("third rule should be created");

        let reordered = service
            .reorder_resolution_rules(
                policy_set.id,
                ReorderChannelResolutionRulesInput {
                    rule_ids: vec![third.id, first.id, second.id],
                },
            )
            .await
            .expect("rules should be reordered");

        let reordered_ids = reordered.iter().map(|rule| rule.id).collect::<Vec<_>>();
        let reordered_priorities = reordered
            .iter()
            .map(|rule| rule.priority)
            .collect::<Vec<_>>();

        assert_eq!(reordered_ids, vec![third.id, first.id, second.id]);
        assert_eq!(reordered_priorities, vec![10, 20, 30]);
    }

    #[tokio::test]
    async fn rejects_reorder_payload_with_missing_rule_ids() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());
        let channel_id = create_channel(&service, tenant_id, "policy-target").await;

        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .expect("policy set should be created");

        let rule = service
            .create_resolution_rule(
                policy_set.id,
                CreateChannelResolutionRuleInput {
                    priority: 10,
                    is_active: true,
                    definition: ChannelResolutionRuleDefinition {
                        predicates: vec![ResolutionPredicate::HostEquals(
                            "shop.example.test".to_string(),
                        )],
                        action: ResolutionAction::ResolveToChannel { channel_id },
                    },
                },
            )
            .await
            .expect("rule should be created");

        let error = service
            .reorder_resolution_rules(
                policy_set.id,
                ReorderChannelResolutionRulesInput { rule_ids: vec![] },
            )
            .await
            .expect_err("empty reorder payload should be rejected");

        assert!(matches!(error, ChannelError::InvalidPolicyOperation(_)));
        assert_eq!(rule.priority, 10);
    }

    async fn seed_oauth_app(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) -> Uuid {
        let oauth_app_id = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO oauth_apps (id, tenant_id, name, slug, app_type, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                oauth_app_id.into(),
                tenant_id.into(),
                format!("{slug} name").into(),
                slug.to_string().into(),
                "machine".to_string().into(),
                true.into(),
            ],
        ))
        .await
        .expect("oauth app should be inserted");
        oauth_app_id
    }

    #[tokio::test]
    async fn list_channels_rejects_zero_pagination() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-a").await;
        let service = ChannelService::new(db);

        let err_page_zero = service
            .list_channels(tenant_id, 0, 10)
            .await
            .expect_err("page 0 must be rejected");
        assert!(matches!(err_page_zero, ChannelError::Validation(_)));

        let err_per_page_zero = service
            .list_channels(tenant_id, 1, 0)
            .await
            .expect_err("per_page 0 must be rejected");
        assert!(matches!(err_per_page_zero, ChannelError::Validation(_)));
    }

    #[tokio::test]
    async fn list_channels_returns_paginated_results() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-b").await;
        let service = ChannelService::new(db);

        service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "first".to_string(),
                name: "First Channel".to_string(),
                settings: None,
            })
            .await
            .expect("first channel");

        service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "second".to_string(),
                name: "Second Channel".to_string(),
                settings: None,
            })
            .await
            .expect("second channel");

        let (items, total) = service
            .list_channels(tenant_id, 1, 10)
            .await
            .expect("list channels");
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);

        let (page1, total) = service
            .list_channels(tenant_id, 1, 1)
            .await
            .expect("page 1");
        assert_eq!(total, 2);
        assert_eq!(page1.len(), 1);

        let (page2, total) = service
            .list_channels(tenant_id, 2, 1)
            .await
            .expect("page 2");
        assert_eq!(total, 2);
        assert_eq!(page2.len(), 1);
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[tokio::test]
    async fn create_channel_rejects_nil_tenant_or_empty_slug_or_name() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-validation").await;
        let service = ChannelService::new(db);

        // Nil tenant_id
        let err_nil_tenant = service
            .create_channel(CreateChannelInput {
                tenant_id: Uuid::nil(),
                slug: "web".to_string(),
                name: "Web".to_string(),
                settings: None,
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err_nil_tenant, ChannelError::Validation(msg) if msg.contains("tenant_id"))
        );

        // Empty slug
        let err_empty_slug = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "   ".to_string(),
                name: "Web".to_string(),
                settings: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(err_empty_slug, ChannelError::Validation(msg) if msg.contains("slug")));

        // Empty name
        let err_empty_name = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "web".to_string(),
                name: "   ".to_string(),
                settings: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(err_empty_name, ChannelError::Validation(msg) if msg.contains("name")));
    }

    #[tokio::test]
    async fn create_channel_normalizes_slug_and_name_and_queries_case_insensitively() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-norm").await;
        let service = ChannelService::new(db);

        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: "  STOREFRONT_WEB  ".to_string(),
                name: "  Main Storefront  ".to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created with normalized values");

        assert_eq!(channel.slug, "storefront_web");
        assert_eq!(channel.name, "Main Storefront");

        // Case-insensitive query
        let by_upper = service
            .get_channel_by_slug(tenant_id, "  STOREFRONT_WEB  ")
            .await
            .unwrap()
            .expect("should find by uppercase with whitespace");
        assert_eq!(by_upper.id, channel.id);

        let detail = service
            .get_channel_detail_by_slug(tenant_id, "storefront_web")
            .await
            .unwrap()
            .expect("should find detail");
        assert_eq!(detail.channel.id, channel.id);
    }

    #[tokio::test]
    async fn create_resolution_policy_set_rejects_nil_tenant_or_empty_slug_or_name() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-policy-val").await;
        let service = ChannelService::new(db);

        let err_nil_tenant = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id: Uuid::nil(),
                slug: "default".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err_nil_tenant, ChannelError::Validation(msg) if msg.contains("tenant_id"))
        );

        let err_empty_slug = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "   ".to_string(),
                name: "Default".to_string(),
                is_active: true,
            })
            .await
            .unwrap_err();
        assert!(matches!(err_empty_slug, ChannelError::Validation(msg) if msg.contains("slug")));

        let err_empty_name = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id,
                slug: "default".to_string(),
                name: "  ".to_string(),
                is_active: true,
            })
            .await
            .unwrap_err();
        assert!(matches!(err_empty_name, ChannelError::Validation(msg) if msg.contains("name")));
    }

    #[tokio::test]
    async fn bind_module_rejects_empty_slug_and_bind_oauth_app_rejects_nil_id() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant-bindings").await;
        let service = ChannelService::new(db);
        let channel_id = create_channel(&service, tenant_id, "binding-test").await;

        let err_empty_module = service
            .bind_module(
                channel_id,
                crate::dto::BindChannelModuleInput {
                    module_slug: "  ".to_string(),
                    is_enabled: true,
                    settings: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err_empty_module, ChannelError::Validation(msg) if msg.contains("module_slug"))
        );

        let err_nil_oauth = service
            .bind_oauth_app(
                channel_id,
                crate::dto::BindChannelOauthAppInput {
                    oauth_app_id: Uuid::nil(),
                    role: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err_nil_oauth, ChannelError::Validation(msg) if msg.contains("oauth_app_id"))
        );
    }
}
