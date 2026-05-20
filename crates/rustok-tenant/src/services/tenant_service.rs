use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CreateTenantInput, TenantModuleResponse, TenantResponse, ToggleModuleInput, UpdateTenantInput,
};
use crate::entities::tenant::{self, ActiveModel as TenantActiveModel};
use crate::entities::tenant_module::{self, ActiveModel as TenantModuleActiveModel};
use crate::error::TenantError;
use crate::settings_schema::validate_tenant_settings;

pub type TenantResult<T> = Result<T, TenantError>;

pub struct TenantService {
    db: DatabaseConnection,
    event_bus: Option<TransactionalEventBus>,
}

impl TenantService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            event_bus: None,
        }
    }

    pub fn with_event_bus(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus: Some(event_bus),
        }
    }

    #[instrument(skip(self, input), fields(slug = %input.slug))]
    pub async fn create_tenant(&self, input: CreateTenantInput) -> TenantResult<TenantResponse> {
        let txn = self.db.begin().await?;
        if let Some(_existing) = tenant::Entity::find()
            .filter(tenant::Column::Slug.eq(&input.slug))
            .one(&txn)
            .await?
        {
            return Err(TenantError::SlugAlreadyExists(input.slug));
        }

        let now = chrono::Utc::now().into();
        let id = generate_id();
        let model = TenantActiveModel {
            id: Set(id),
            name: Set(input.name),
            slug: Set(input.slug),
            domain: Set(input.domain),
            settings: Set(serde_json::json!({})),
            default_locale: Set("en".to_string()),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await?;

        self.publish_event_in_tx(&txn, id, DomainEvent::TenantCreated { tenant_id: id })
            .await?;

        txn.commit().await?;

        Ok(to_tenant_response(model))
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn get_tenant(&self, tenant_id: Uuid) -> TenantResult<TenantResponse> {
        let model = tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;
        Ok(to_tenant_response(model))
    }

    #[instrument(skip(self), fields(slug = %slug))]
    pub async fn get_tenant_by_slug(&self, slug: &str) -> TenantResult<TenantResponse> {
        let model = tenant::Entity::find()
            .filter(tenant::Column::Slug.eq(slug))
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;
        Ok(to_tenant_response(model))
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn update_tenant(
        &self,
        tenant_id: Uuid,
        input: UpdateTenantInput,
    ) -> TenantResult<TenantResponse> {
        let txn = self.db.begin().await?;

        let existing = tenant::Entity::find_by_id(tenant_id)
            .one(&txn)
            .await?
            .ok_or(TenantError::NotFound)?;

        let now = chrono::Utc::now().into();
        let mut active: tenant::ActiveModel = existing.into();
        if let Some(name) = input.name {
            active.name = Set(name);
        }
        if let Some(domain) = input.domain {
            active.domain = Set(Some(domain));
        }
        if let Some(is_active) = input.is_active {
            active.is_active = Set(is_active);
        }
        if let Some(settings) = input.settings {
            validate_tenant_settings(&settings)?;
            active.settings = Set(settings);
        }
        active.updated_at = Set(now);

        let model = active.update(&txn).await?;

        self.publish_event_in_tx(
            &txn,
            tenant_id,
            DomainEvent::TenantUpdated { tenant_id },
        )
        .await?;

        txn.commit().await?;

        Ok(to_tenant_response(model))
    }

    pub async fn list_tenants(
        &self,
        page: u64,
        per_page: u64,
    ) -> TenantResult<(Vec<TenantResponse>, u64)> {
        let paginator = tenant::Entity::find().paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let items = models.into_iter().map(to_tenant_response).collect();
        Ok((items, total))
    }

    /// Deprecated low-level tenant override writer.
    ///
    /// Runtime module enable/disable paths must go through the host
    /// `ModuleLifecycleService` so policy resolution, dependency checks, hooks,
    /// and operation journaling stay consistent.
    #[deprecated(
        note = "use the host ModuleLifecycleService for runtime module enable/disable paths"
    )]
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, module_slug = %input.module_slug))]
    pub async fn toggle_module(
        &self,
        tenant_id: Uuid,
        input: ToggleModuleInput,
    ) -> TenantResult<TenantModuleResponse> {
        let txn = self.db.begin().await?;

        tenant::Entity::find_by_id(tenant_id)
            .one(&txn)
            .await?
            .ok_or(TenantError::NotFound)?;

        let existing = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(&input.module_slug))
            .one(&txn)
            .await?;

        let now = chrono::Utc::now().into();
        let module_slug = input.module_slug;
        let enabled = input.enabled;

        let model = match existing {
            Some(m) => {
                let mut active: tenant_module::ActiveModel = m.into();
                active.enabled = Set(enabled);
                active.updated_at = Set(now);
                active.update(&txn).await?
            }
            None => {
                TenantModuleActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    module_slug: Set(module_slug.clone()),
                    enabled: Set(enabled),
                    settings: Set(serde_json::json!({})),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&txn)
                .await?
            }
        };

        self.publish_event_in_tx(
            &txn,
            tenant_id,
            DomainEvent::TenantModuleToggled {
                tenant_id,
                module_slug,
                enabled,
            },
        )
        .await?;

        txn.commit().await?;

        Ok(to_module_response(model))
    }

    pub async fn list_tenant_modules(
        &self,
        tenant_id: Uuid,
    ) -> TenantResult<Vec<TenantModuleResponse>> {
        tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;

        let modules = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await?;

        Ok(modules.into_iter().map(to_module_response).collect())
    }

    async fn publish_event_in_tx<C>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        event: DomainEvent,
    ) -> TenantResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        if let Some(event_bus) = &self.event_bus {
            event_bus
                .publish_in_tx(txn, tenant_id, None, event)
                .await
                .map_err(|error| TenantError::EventPublish(error.to_string()))?;
        }

        Ok(())
    }
}

fn to_tenant_response(m: tenant::Model) -> TenantResponse {
    TenantResponse {
        id: m.id,
        name: m.name,
        slug: m.slug,
        domain: m.domain,
        is_active: m.is_active,
        settings: m.settings,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

fn to_module_response(m: tenant_module::Model) -> TenantModuleResponse {
    TenantModuleResponse {
        id: m.id,
        tenant_id: m.tenant_id,
        module_slug: m.module_slug,
        enabled: m.enabled,
        settings: m.settings,
    }
}
