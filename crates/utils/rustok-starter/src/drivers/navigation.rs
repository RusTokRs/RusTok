use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_core::SecurityContext;
use rustok_navigation::dto::{
    CreateMenuInput, MenuItemInput, MenuItemTranslationInput, MenuLocation, MenuTranslationInput,
};
use rustok_navigation::services::MenuService;

use crate::error::StarterResult;
use crate::model::NavigationMenuStarter;

/// Imports navigation menus and menu trees.
pub async fn import_navigation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: &SecurityContext,
    menus: &[NavigationMenuStarter],
    locale: &str,
) -> StarterResult<(usize, usize)> {
    let service = MenuService::new(db.clone());
    let mut created = 0;
    let mut skipped = 0;

    for menu in menus {
        let (location, location_str) = match menu.location.to_ascii_lowercase().as_str() {
            "header" => (MenuLocation::Header, "header"),
            "footer" => (MenuLocation::Footer, "footer"),
            "sidebar" => (MenuLocation::Sidebar, "sidebar"),
            "mobile" => (MenuLocation::Mobile, "mobile"),
            _ => (MenuLocation::Header, "header"),
        };

        // Idempotency: check if menu for this location already exists for this tenant
        let existing = rustok_navigation::entities::menu::Entity::find()
            .filter(rustok_navigation::entities::menu::Column::TenantId.eq(tenant_id))
            .filter(rustok_navigation::entities::menu::Column::Location.eq(location_str))
            .one(db)
            .await?;

        if existing.is_some() {
            tracing::info!(location = %location_str, "Navigation menu already exists, skipping");
            skipped += 1;
            continue;
        }

        let items: Vec<MenuItemInput> = menu
            .items
            .iter()
            .map(|item| MenuItemInput {
                translations: vec![MenuItemTranslationInput {
                    locale: locale.to_string(),
                    title: item.title.clone(),
                }],
                url: Some(item.url.clone()),
                icon: item.icon.clone(),
                position: item.position,
                children: None,
            })
            .collect();

        let created_menu = service
            .create(
                tenant_id,
                security.clone(),
                locale,
                CreateMenuInput {
                    translations: vec![MenuTranslationInput {
                        locale: locale.to_string(),
                        name: menu.name.clone(),
                    }],
                    location,
                    items,
                },
            )
            .await?;

        // If channels exist, bind the created menu to the default channel so it is immediately active on Storefront.
        // A channel lookup or binding failure is part of the import result; it must not be reported as success.
        let channels = rustok_channel::entities::channel::Entity::find()
            .filter(rustok_channel::entities::channel::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?;
        let target_channel = channels
            .iter()
            .find(|channel| channel.is_default)
            .or_else(|| channels.first());
        if let Some(channel) = target_channel {
            let binding_service = rustok_navigation::services::MenuBindingService::new(db.clone());
            binding_service
                .bind(
                    tenant_id,
                    security.clone(),
                    channel.id,
                    location,
                    created_menu.id,
                )
                .await?;
        }

        created += 1;
    }

    Ok((created, skipped))
}
