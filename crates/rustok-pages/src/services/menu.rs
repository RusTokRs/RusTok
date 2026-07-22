use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::entities::{menu, menu_item, menu_item_translation, menu_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_scope;

pub struct MenuService {
    db: DatabaseConnection,
}

impl MenuService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let _ = event_bus;
        Self { db }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateMenuInput,
    ) -> PagesResult<MenuResponse> {
        enforce_scope(&security, Resource::Pages, Action::Create)?;
        let now = Utc::now();
        let menu_id = Uuid::new_v4();

        let txn = self.db.begin().await?;
        menu::ActiveModel {
            id: Set(menu_id),
            tenant_id: Set(tenant_id),
            location: Set(menu_location_to_storage(&input.location).to_string()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        menu_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            menu_id: Set(menu_id),
            locale: Set(PLATFORM_FALLBACK_LOCALE.to_string()),
            name: Set(input.name),
        }
        .insert(&txn)
        .await?;

        for item in input.items {
            self.create_menu_item_in_tx(&txn, tenant_id, menu_id, None, item)
                .await?;
        }

        txn.commit().await?;
        self.get(tenant_id, SecurityContext::system(), menu_id)
            .await
    }

    fn create_menu_item_in_tx<'a>(
        &'a self,
        txn: &'a DatabaseTransaction,
        tenant_id: Uuid,
        menu_id: Uuid,
        parent_item_id: Option<Uuid>,
        input: MenuItemInput,
    ) -> Pin<Box<dyn Future<Output = PagesResult<Uuid>> + Send + 'a>> {
        Box::pin(async move {
            let now = Utc::now();
            let item_id = Uuid::new_v4();
            let url = input.url.unwrap_or_else(|| "/".to_string());

            menu_item::ActiveModel {
                id: Set(item_id),
                menu_id: Set(menu_id),
                tenant_id: Set(tenant_id),
                parent_item_id: Set(parent_item_id),
                page_id: Set(input.page_id),
                position: Set(input.position),
                url: Set(url),
                icon: Set(input.icon),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(txn)
            .await?;

            menu_item_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                menu_item_id: Set(item_id),
                locale: Set(PLATFORM_FALLBACK_LOCALE.to_string()),
                title: Set(input.title),
            }
            .insert(txn)
            .await?;

            if let Some(children) = input.children {
                for child in children {
                    self.create_menu_item_in_tx(txn, tenant_id, menu_id, Some(item_id), child)
                        .await?;
                }
            }

            Ok(item_id)
        })
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        menu_id: Uuid,
    ) -> PagesResult<MenuResponse> {
        enforce_scope(&security, Resource::Pages, Action::Read)?;
        let menu = menu::Entity::find_by_id(menu_id)
            .filter(menu::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| PagesError::menu_not_found(menu_id))?;

        let location = menu_location_from_storage(&menu.location)?;
        let name = self.load_menu_name(menu.id).await?;
        let items = self.load_menu_items(tenant_id, menu.id).await?;

        Ok(MenuResponse {
            id: menu.id,
            name,
            location,
            items,
        })
    }

    async fn load_menu_name(&self, menu_id: Uuid) -> PagesResult<String> {
        let translations = menu_translation::Entity::find()
            .filter(menu_translation::Column::MenuId.eq(menu_id))
            .order_by_asc(menu_translation::Column::Locale)
            .all(&self.db)
            .await?;

        translations
            .iter()
            .find(|translation| translation.locale == PLATFORM_FALLBACK_LOCALE)
            .or_else(|| translations.first())
            .map(|translation| translation.name.clone())
            .ok_or_else(|| PagesError::menu_not_found(menu_id))
    }

    async fn load_menu_items(
        &self,
        tenant_id: Uuid,
        menu_id: Uuid,
    ) -> PagesResult<Vec<MenuItemResponse>> {
        let items = menu_item::Entity::find()
            .filter(menu_item::Column::TenantId.eq(tenant_id))
            .filter(menu_item::Column::MenuId.eq(menu_id))
            .order_by_asc(menu_item::Column::Position)
            .order_by_asc(menu_item::Column::CreatedAt)
            .all(&self.db)
            .await?;

        if items.is_empty() {
            return Ok(Vec::new());
        }

        let item_ids: Vec<Uuid> = items.iter().map(|item| item.id).collect();
        let translations = menu_item_translation::Entity::find()
            .filter(menu_item_translation::Column::MenuItemId.is_in(item_ids))
            .order_by_asc(menu_item_translation::Column::Locale)
            .all(&self.db)
            .await?;

        let mut titles_by_item: HashMap<Uuid, String> = HashMap::new();
        for translation in translations {
            titles_by_item
                .entry(translation.menu_item_id)
                .or_insert(translation.title);
        }

        let mut items_by_parent: HashMap<Option<Uuid>, Vec<menu_item::Model>> = HashMap::new();
        for item in items {
            items_by_parent
                .entry(item.parent_item_id)
                .or_default()
                .push(item);
        }

        Ok(build_menu_tree(None, &mut items_by_parent, &titles_by_item))
    }
}

fn build_menu_tree(
    parent_id: Option<Uuid>,
    items_by_parent: &mut HashMap<Option<Uuid>, Vec<menu_item::Model>>,
    titles_by_item: &HashMap<Uuid, String>,
) -> Vec<MenuItemResponse> {
    let Some(items) = items_by_parent.remove(&parent_id) else {
        return Vec::new();
    };

    items
        .into_iter()
        .map(|item| {
            let children = build_menu_tree(Some(item.id), items_by_parent, titles_by_item);
            MenuItemResponse {
                id: item.id,
                title: titles_by_item.get(&item.id).cloned(),
                url: item.url,
                icon: item.icon,
                children,
            }
        })
        .collect()
}

fn menu_location_to_storage(location: &MenuLocation) -> &'static str {
    match location {
        MenuLocation::Header => "header",
        MenuLocation::Footer => "footer",
        MenuLocation::Sidebar => "sidebar",
        MenuLocation::Mobile => "mobile",
    }
}

fn menu_location_from_storage(value: &str) -> PagesResult<MenuLocation> {
    Ok(match value {
        "header" => MenuLocation::Header,
        "footer" => MenuLocation::Footer,
        "sidebar" => MenuLocation::Sidebar,
        "mobile" => MenuLocation::Mobile,
        other => {
            return Err(PagesError::validation(format!(
                "Unknown menu location in storage: {other}"
            )));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_location_round_trip() {
        assert!(matches!(
            menu_location_from_storage(menu_location_to_storage(&MenuLocation::Header)),
            Ok(MenuLocation::Header)
        ));
        assert!(matches!(
            menu_location_from_storage(menu_location_to_storage(&MenuLocation::Footer)),
            Ok(MenuLocation::Footer)
        ));
    }
}
