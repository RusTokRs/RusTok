use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::dto::{
    BindChannelModuleInput, BindChannelOauthAppInput, ChannelDetailResponse,
    ChannelModuleBindingResponse, ChannelOauthAppResponse, ChannelResponse, ChannelTargetResponse,
    CreateChannelInput, CreateChannelTargetInput,
};
use crate::entities::channel::{self, ActiveModel as ChannelActiveModel};
use crate::entities::channel_module_binding::{
    self, ActiveModel as ChannelModuleBindingActiveModel,
};
use crate::entities::channel_oauth_app::{self, ActiveModel as ChannelOauthAppActiveModel};
use crate::entities::channel_target::{self, ActiveModel as ChannelTargetActiveModel};
use crate::error::{ChannelError, ChannelResult};

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

        let now = chrono::Utc::now().into();
        let model = ChannelActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            slug: Set(input.slug),
            name: Set(input.name),
            is_active: Set(true),
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

    pub async fn get_channel_by_target_value(
        &self,
        tenant_id: Uuid,
        target_value: &str,
    ) -> ChannelResult<Option<ChannelDetailResponse>> {
        let target = channel_target::Entity::find()
            .filter(channel_target::Column::Value.eq(target_value))
            .find_also_related(channel::Entity)
            .one(&self.db)
            .await?;

        let Some((target, Some(channel_model))) = target else {
            return Ok(None);
        };

        if channel_model.tenant_id != tenant_id {
            return Ok(None);
        }

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
            .order_by_asc(channel::Column::CreatedAt)
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
            .order_by_asc(channel::Column::CreatedAt)
            .all(&self.db)
            .await?;

        let mut items = Vec::with_capacity(models.len());
        for model in models {
            items.push(self.build_channel_detail(model).await?);
        }
        Ok(items)
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
        if input.target_type.trim().is_empty() {
            return Err(ChannelError::InvalidTargetType(input.target_type));
        }

        self.ensure_channel_exists(channel_id).await?;

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
            target_type: Set(input.target_type),
            value: Set(input.value),
            is_primary: Set(input.is_primary),
            settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        Ok(to_channel_target_response(model))
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

    async fn ensure_channel_exists(&self, channel_id: Uuid) -> ChannelResult<()> {
        channel::Entity::find_by_id(channel_id)
            .one(&self.db)
            .await?
            .ok_or(ChannelError::NotFound(channel_id))?;
        Ok(())
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
