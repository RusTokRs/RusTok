use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::dto::{
    BindChannelModuleInput, BindChannelOauthAppInput, ChannelDetailResponse,
    ChannelModuleBindingResponse, ChannelOauthAppResponse, ChannelResponse, ChannelTargetResponse,
    CreateChannelInput, CreateChannelTargetInput, UpdateChannelTargetInput,
};
use crate::entities::channel::{self, ActiveModel as ChannelActiveModel};
use crate::entities::channel_module_binding::{
    self, ActiveModel as ChannelModuleBindingActiveModel,
};
use crate::entities::channel_oauth_app::{self, ActiveModel as ChannelOauthAppActiveModel};
use crate::entities::channel_target::{self, ActiveModel as ChannelTargetActiveModel};
use crate::error::{ChannelError, ChannelResult};
use crate::target_type::ChannelTargetType;

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
        if let Some(_existing) = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(input.tenant_id))
            .filter(channel::Column::Slug.eq(&input.slug))
            .one(&self.db)
            .await?
        {
            return Err(ChannelError::SlugAlreadyExists(input.slug));
        }

        let is_default = !self
            .default_channel_exists_for_tenant(input.tenant_id)
            .await?;
        let now = chrono::Utc::now().into();
        let model = ChannelActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            slug: Set(input.slug),
            name: Set(input.name),
            is_active: Set(true),
            is_default: Set(is_default),
            status: Set("experimental".to_string()),
            settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        Ok(to_channel_response(model))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id))]
    pub async fn get_channel(&self, channel_id: Uuid) -> ChannelResult<ChannelResponse> {
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
        let model = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Slug.eq(slug))
            .one(&self.db)
            .await?;
        Ok(model.map(to_channel_response))
    }

    pub async fn get_channel_detail_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
    ) -> ChannelResult<Option<ChannelDetailResponse>> {
        let model = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Slug.eq(slug))
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
        let target = channel_target::Entity::find()
            .filter(channel_target::Column::TargetType.eq(ChannelTargetType::WebDomain.as_str()))
            .filter(channel_target::Column::Value.eq(target_value))
            .find_also_related(channel::Entity)
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::IsActive.eq(true))
            .one(&self.db)
            .await?;

        let Some((target, Some(channel_model))) = target else {
            return Ok(None);
        };

        let detail = self.build_channel_detail(channel_model).await?;
        let mut detail = detail;
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
        let paginator = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .order_by_desc(channel::Column::IsDefault)
            .order_by_asc(channel::Column::CreatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let items = models.into_iter().map(to_channel_response).collect();
        Ok((items, total))
    }

    pub async fn list_channel_details(
        &self,
        tenant_id: Uuid,
    ) -> ChannelResult<Vec<ChannelDetailResponse>> {
        let models = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .order_by_desc(channel::Column::IsDefault)
            .order_by_asc(channel::Column::CreatedAt)
            .all(&self.db)
            .await?;

        let mut items = Vec::with_capacity(models.len());
        for model in models {
            items.push(self.build_channel_detail(model).await?);
        }
        Ok(items)
    }

    #[instrument(skip(self), fields(channel_id = %channel_id))]
    pub async fn set_default_channel(&self, channel_id: Uuid) -> ChannelResult<ChannelResponse> {
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

    pub async fn is_module_enabled(
        &self,
        channel_id: Uuid,
        module_slug: &str,
    ) -> ChannelResult<bool> {
        self.ensure_channel_exists(channel_id).await?;

        let binding = channel_module_binding::Entity::find()
            .filter(channel_module_binding::Column::ChannelId.eq(channel_id))
            .filter(channel_module_binding::Column::ModuleSlug.eq(module_slug))
            .one(&self.db)
            .await?;

        Ok(binding.map(|item| item.is_enabled).unwrap_or(true))
    }

    #[instrument(skip(self, input), fields(channel_id = %channel_id, target_type = %input.target_type))]
    pub async fn add_target(
        &self,
        channel_id: Uuid,
        input: CreateChannelTargetInput,
    ) -> ChannelResult<ChannelTargetResponse> {
        let Some(target_type) = ChannelTargetType::parse(&input.target_type) else {
            return Err(ChannelError::InvalidTargetType(input.target_type));
        };
        let target_value = normalize_target_value(target_type, &input.value)
            .ok_or_else(|| ChannelError::InvalidTargetValue(input.value.clone()))?;

        let channel = channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;

        if target_type.supports_host_resolution()
            && self
                .host_target_exists_for_tenant(channel.tenant_id, target_value.as_str())
                .await?
        {
            return Err(ChannelError::TargetAlreadyExists(
                target_type.as_str().to_string(),
                target_value,
            ));
        }

        if input.is_primary {
            let existing_targets = channel_target::Entity::find()
                .filter(channel_target::Column::ChannelId.eq(channel_id))
                .all(&self.db)
                .await?;
            for existing in existing_targets {
                if existing.is_primary {
                    let mut active: channel_target::ActiveModel = existing.into();
                    active.is_primary = Set(false);
                    active.update(&self.db).await?;
                }
            }
        }

        let now = chrono::Utc::now().into();
        let model = ChannelTargetActiveModel {
            id: Set(generate_id()),
            channel_id: Set(channel_id),
            target_type: Set(target_type.as_str().to_string()),
            value: Set(target_value),
            is_primary: Set(input.is_primary),
            settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        Ok(to_channel_target_response(model))
    }

    #[instrument(skip(self, input), fields(channel_id = %channel_id, target_id = %target_id))]
    pub async fn update_target(
        &self,
        channel_id: Uuid,
        target_id: Uuid,
        input: UpdateChannelTargetInput,
    ) -> ChannelResult<ChannelTargetResponse> {
        let Some(target_type) = ChannelTargetType::parse(&input.target_type) else {
            return Err(ChannelError::InvalidTargetType(input.target_type));
        };
        let target_value = normalize_target_value(target_type, &input.value)
            .ok_or_else(|| ChannelError::InvalidTargetValue(input.value.clone()))?;

        let channel = channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;

        let existing_target = channel_target::Entity::find_by_id(target_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(target_id))?;
        if existing_target.channel_id != channel_id {
            return Err(ChannelError::NotFound(target_id));
        }

        if target_type.supports_host_resolution()
            && self
                .host_target_exists_for_tenant_except(
                    channel.tenant_id,
                    target_id,
                    target_value.as_str(),
                )
                .await?
        {
            return Err(ChannelError::TargetAlreadyExists(
                target_type.as_str().to_string(),
                target_value,
            ));
        }

        if input.is_primary {
            let existing_targets = channel_target::Entity::find()
                .filter(channel_target::Column::ChannelId.eq(channel_id))
                .all(&self.db)
                .await?;
            for existing in existing_targets {
                if existing.id != target_id && existing.is_primary {
                    let mut active: channel_target::ActiveModel = existing.into();
                    active.is_primary = Set(false);
                    active.update(&self.db).await?;
                }
            }
        }

        let now = chrono::Utc::now().into();
        let mut active: channel_target::ActiveModel = existing_target.into();
        active.target_type = Set(target_type.as_str().to_string());
        active.value = Set(target_value);
        active.is_primary = Set(input.is_primary);
        active.settings = Set(input.settings.unwrap_or_else(|| serde_json::json!({})));
        active.updated_at = Set(now);

        Ok(to_channel_target_response(active.update(&self.db).await?))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id, target_id = %target_id))]
    pub async fn delete_target(
        &self,
        channel_id: Uuid,
        target_id: Uuid,
    ) -> ChannelResult<ChannelTargetResponse> {
        let target = channel_target::Entity::find_by_id(target_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(target_id))?;
        if target.channel_id != channel_id {
            return Err(ChannelError::NotFound(target_id));
        }

        let response = to_channel_target_response(target.clone());
        let active: channel_target::ActiveModel = target.into();
        active.delete(&self.db).await?;
        Ok(response)
    }

    #[instrument(skip(self, input), fields(channel_id = %channel_id, module_slug = %input.module_slug))]
    pub async fn bind_module(
        &self,
        channel_id: Uuid,
        input: BindChannelModuleInput,
    ) -> ChannelResult<ChannelModuleBindingResponse> {
        self.ensure_channel_exists(channel_id).await?;

        let now = chrono::Utc::now().into();
        let existing = channel_module_binding::Entity::find()
            .filter(channel_module_binding::Column::ChannelId.eq(channel_id))
            .filter(channel_module_binding::Column::ModuleSlug.eq(&input.module_slug))
            .one(&self.db)
            .await?;

        let model = match existing {
            Some(model) => {
                let mut active: channel_module_binding::ActiveModel = model.into();
                active.is_enabled = Set(input.is_enabled);
                active.settings = Set(input.settings.unwrap_or_else(|| serde_json::json!({})));
                active.updated_at = Set(now);
                active.update(&self.db).await?
            }
            None => {
                ChannelModuleBindingActiveModel {
                    id: Set(generate_id()),
                    channel_id: Set(channel_id),
                    module_slug: Set(input.module_slug),
                    is_enabled: Set(input.is_enabled),
                    settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&self.db)
                .await?
            }
        };

        Ok(to_channel_module_binding_response(model))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id, binding_id = %binding_id))]
    pub async fn remove_module_binding(
        &self,
        channel_id: Uuid,
        binding_id: Uuid,
    ) -> ChannelResult<ChannelModuleBindingResponse> {
        let binding = channel_module_binding::Entity::find_by_id(binding_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(binding_id))?;
        if binding.channel_id != channel_id {
            return Err(ChannelError::NotFound(binding_id));
        }

        let response = to_channel_module_binding_response(binding.clone());
        let active: channel_module_binding::ActiveModel = binding.into();
        active.delete(&self.db).await?;
        Ok(response)
    }

    #[instrument(skip(self, input), fields(channel_id = %channel_id, oauth_app_id = %input.oauth_app_id))]
    pub async fn bind_oauth_app(
        &self,
        channel_id: Uuid,
        input: BindChannelOauthAppInput,
    ) -> ChannelResult<ChannelOauthAppResponse> {
        self.ensure_channel_exists(channel_id).await?;

        let now = chrono::Utc::now().into();
        let existing = channel_oauth_app::Entity::find()
            .filter(channel_oauth_app::Column::ChannelId.eq(channel_id))
            .filter(channel_oauth_app::Column::OauthAppId.eq(input.oauth_app_id))
            .one(&self.db)
            .await?;

        let model = match existing {
            Some(model) => {
                let mut active: channel_oauth_app::ActiveModel = model.into();
                active.role = Set(input.role);
                active.update(&self.db).await?
            }
            None => {
                ChannelOauthAppActiveModel {
                    id: Set(generate_id()),
                    channel_id: Set(channel_id),
                    oauth_app_id: Set(input.oauth_app_id),
                    role: Set(input.role),
                    created_at: Set(now),
                }
                .insert(&self.db)
                .await?
            }
        };

        Ok(to_channel_oauth_app_response(model))
    }

    #[instrument(skip(self), fields(channel_id = %channel_id, binding_id = %binding_id))]
    pub async fn revoke_oauth_app_binding(
        &self,
        channel_id: Uuid,
        binding_id: Uuid,
    ) -> ChannelResult<ChannelOauthAppResponse> {
        let binding = channel_oauth_app::Entity::find_by_id(binding_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(binding_id))?;
        if binding.channel_id != channel_id {
            return Err(ChannelError::NotFound(binding_id));
        }

        let response = to_channel_oauth_app_response(binding.clone());
        let active: channel_oauth_app::ActiveModel = binding.into();
        active.delete(&self.db).await?;
        Ok(response)
    }

    async fn ensure_channel_exists(&self, channel_id: Uuid) -> ChannelResult<()> {
        channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;
        Ok(())
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

    async fn replace_default_channel(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
    ) -> ChannelResult<()> {
        let now = chrono::Utc::now().into();
        let txn = self.db.begin().await?;
        let channels = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;

        for existing in channels {
            let should_be_default = existing.id == channel_id;
            if existing.is_default == should_be_default {
                continue;
            }

            let mut active: channel::ActiveModel = existing.into();
            active.is_default = Set(should_be_default);
            active.updated_at = Set(now);
            active.update(&txn).await?;
        }

        txn.commit().await?;
        Ok(())
    }

    async fn host_target_exists_for_tenant(
        &self,
        tenant_id: Uuid,
        target_value: &str,
    ) -> ChannelResult<bool> {
        let existing = channel_target::Entity::find()
            .filter(channel_target::Column::TargetType.eq(ChannelTargetType::WebDomain.as_str()))
            .filter(channel_target::Column::Value.eq(target_value))
            .find_also_related(channel::Entity)
            .filter(channel::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        Ok(existing.is_some())
    }

    async fn host_target_exists_for_tenant_except(
        &self,
        tenant_id: Uuid,
        target_id: Uuid,
        target_value: &str,
    ) -> ChannelResult<bool> {
        let existing = channel_target::Entity::find()
            .filter(channel_target::Column::TargetType.eq(ChannelTargetType::WebDomain.as_str()))
            .filter(channel_target::Column::Value.eq(target_value))
            .filter(channel_target::Column::Id.ne(target_id))
            .find_also_related(channel::Entity)
            .filter(channel::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        Ok(existing.is_some())
    }

    async fn build_channel_detail(
        &self,
        channel_model: channel::Model,
    ) -> ChannelResult<ChannelDetailResponse> {
        let channel_id = channel_model.id;
        let targets = channel_target::Entity::find()
            .filter(channel_target::Column::ChannelId.eq(channel_id))
            .order_by_desc(channel_target::Column::IsPrimary)
            .order_by_asc(channel_target::Column::CreatedAt)
            .all(&self.db)
            .await?
            .into_iter()
            .map(to_channel_target_response)
            .collect();
        let module_bindings = channel_module_binding::Entity::find()
            .filter(channel_module_binding::Column::ChannelId.eq(channel_id))
            .order_by_asc(channel_module_binding::Column::ModuleSlug)
            .all(&self.db)
            .await?
            .into_iter()
            .map(to_channel_module_binding_response)
            .collect();
        let oauth_apps = channel_oauth_app::Entity::find()
            .filter(channel_oauth_app::Column::ChannelId.eq(channel_id))
            .order_by_asc(channel_oauth_app::Column::CreatedAt)
            .all(&self.db)
            .await?
            .into_iter()
            .map(to_channel_oauth_app_response)
            .collect();

        Ok(ChannelDetailResponse {
            channel: to_channel_response(channel_model),
            targets,
            module_bindings,
            oauth_apps,
        })
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
        status: model.status,
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

fn normalize_target_value(target_type: ChannelTargetType, raw: &str) -> Option<String> {
    target_type.normalize_value(raw)
}

#[cfg(test)]
mod tests {
    use super::ChannelService;
    use crate::ChannelError;
    use crate::dto::{CreateChannelInput, CreateChannelTargetInput, UpdateChannelTargetInput};
    use crate::migrations;
    use rustok_test_utils::setup_test_db;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    async fn setup_channel_db() -> DatabaseConnection {
        let db = setup_test_db().await;
        db.execute(Statement::from_string(
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
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE o_auth_apps (
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
        .expect("o_auth_apps table should exist for channel foreign keys");
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
        db.execute(Statement::from_sql_and_values(
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

    async fn seed_oauth_app(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) -> Uuid {
        let oauth_app_id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO o_auth_apps (id, tenant_id, name, slug, app_type, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
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
}
