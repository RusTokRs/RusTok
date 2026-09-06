use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_channel::{ChannelError, ChannelService};
use rustok_core::SecurityContext;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{ActiveMenuBindingResponse, MenuLocation, MenuResponse};
use crate::entities::{menu, menu_binding};
use crate::error::{NavigationError, NavigationResult};
use crate::services::MenuService;
use crate::services::rbac::enforce_scope;

pub struct MenuBindingService {
    db: DatabaseConnection,
}

impl MenuBindingService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn bind(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        channel_id: Uuid,
        location: MenuLocation,
        menu_id: Uuid,
    ) -> NavigationResult<ActiveMenuBindingResponse> {
        enforce_scope(&security, Resource::Navigation, Action::Update)?;
        self.ensure_channel_scope(tenant_id, channel_id).await?;

        let menu = menu::Entity::find_by_id(menu_id)
            .filter(menu::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| NavigationError::menu_not_found(menu_id))?;
        let storage_location = menu_location_to_storage(location);
        if menu.location != storage_location {
            return Err(NavigationError::validation(format!(
                "Menu `{menu_id}` belongs to location `{}` and cannot be bound to `{storage_location}`",
                menu.location,
            )));
        }

        let now = Utc::now();
        let txn = self.db.begin().await?;
        let existing = menu_binding::Entity::find()
            .filter(menu_binding::Column::TenantId.eq(tenant_id))
            .filter(menu_binding::Column::ChannelId.eq(channel_id))
            .filter(menu_binding::Column::Location.eq(storage_location))
            .one(&txn)
            .await?;

        let model = if let Some(existing) = existing {
            let mut active = existing.into_active_model();
            active.menu_id = Set(menu_id);
            active.updated_at = Set(now.into());
            active.update(&txn).await?
        } else {
            menu_binding::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                channel_id: Set(channel_id),
                location: Set(storage_location.to_string()),
                menu_id: Set(menu_id),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?
        };
        txn.commit().await?;

        binding_response(model)
    }

    pub async fn get_binding(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        channel_id: Uuid,
        location: MenuLocation,
    ) -> NavigationResult<Option<ActiveMenuBindingResponse>> {
        enforce_scope(&security, Resource::Navigation, Action::Read)?;
        let model = menu_binding::Entity::find()
            .filter(menu_binding::Column::TenantId.eq(tenant_id))
            .filter(menu_binding::Column::ChannelId.eq(channel_id))
            .filter(menu_binding::Column::Location.eq(menu_location_to_storage(location)))
            .one(&self.db)
            .await?;
        model.map(binding_response).transpose()
    }

    pub async fn get_active(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        channel_id: Uuid,
        location: MenuLocation,
        effective_locale: &str,
    ) -> NavigationResult<Option<MenuResponse>> {
        let Some(binding) = self
            .get_binding(tenant_id, security.clone(), channel_id, location)
            .await?
        else {
            return Ok(None);
        };

        let mut menu = MenuService::new(self.db.clone())
            .get(tenant_id, security, binding.menu_id, effective_locale)
            .await?;
        menu.location = binding.location;
        Ok(Some(menu))
    }

    async fn ensure_channel_scope(
        &self,
        tenant_id: Uuid,
        channel_id: Uuid,
    ) -> NavigationResult<()> {
        let channel = match ChannelService::new(self.db.clone())
            .get_channel(channel_id)
            .await
        {
            Ok(channel) => channel,
            Err(ChannelError::NotFound(_)) => {
                return Err(NavigationError::validation(format!(
                    "Channel `{channel_id}` does not exist for active menu binding"
                )));
            }
            Err(ChannelError::Database(error)) => return Err(NavigationError::Database(error)),
            Err(error) => {
                return Err(NavigationError::validation(format!(
                    "Unable to validate channel `{channel_id}` for active menu binding: {error}"
                )));
            }
        };
        if channel.tenant_id != tenant_id {
            return Err(NavigationError::validation(format!(
                "Channel `{channel_id}` does not belong to tenant `{tenant_id}`"
            )));
        }
        if !channel.is_active {
            return Err(NavigationError::validation(format!(
                "Channel `{channel_id}` is inactive"
            )));
        }
        Ok(())
    }
}

fn binding_response(model: menu_binding::Model) -> NavigationResult<ActiveMenuBindingResponse> {
    Ok(ActiveMenuBindingResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        channel_id: model.channel_id,
        location: menu_location_from_storage(&model.location)?,
        menu_id: model.menu_id,
    })
}

pub(crate) fn menu_location_to_storage(location: MenuLocation) -> &'static str {
    match location {
        MenuLocation::Header => "header",
        MenuLocation::Footer => "footer",
        MenuLocation::Sidebar => "sidebar",
        MenuLocation::Mobile => "mobile",
    }
}

fn menu_location_from_storage(value: &str) -> NavigationResult<MenuLocation> {
    match value {
        "header" => Ok(MenuLocation::Header),
        "footer" => Ok(MenuLocation::Footer),
        "sidebar" => Ok(MenuLocation::Sidebar),
        "mobile" => Ok(MenuLocation::Mobile),
        _ => Err(NavigationError::validation(format!(
            "Unsupported active menu location: {value}"
        ))),
    }
}
