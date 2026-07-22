use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::{
    SecurityContext,
    error::{ErrorKind, RichError},
};
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::entities::{menu, menu_item, menu_item_translation, menu_translation, page};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_scope;

pub const MENU_LOCALE_NOT_FOUND_ERROR_CODE: &str = "MENU_LOCALE_NOT_FOUND";
pub const MENU_TRANSLATION_INTEGRITY_ERROR_CODE: &str = "MENU_TRANSLATION_INTEGRITY";

const MAX_MENU_NAME_CHARS: usize = 255;
const MAX_MENU_ITEM_TITLE_CHARS: usize = 255;

pub struct MenuService {
    db: DatabaseConnection,
}

impl MenuService {
    pub fn new(db: DatabaseConnection, _event_bus: TransactionalEventBus) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        effective_locale: &str,
        input: CreateMenuInput,
    ) -> PagesResult<MenuResponse> {
        enforce_scope(&security, Resource::Pages, Action::Create)?;
        let effective_locale = normalize_effective_locale(effective_locale)?;
        let translations = normalize_menu_translations(input.translations)?;
        let menu_locales = translation_locales(&translations);
        if !menu_locales.contains(&effective_locale) {
            return Err(PagesError::validation(format!(
                "Menu create response locale `{effective_locale}` must be present in menu translations"
            )));
        }
        let items = input
            .items
            .into_iter()
            .map(|item| normalize_menu_item(item, &menu_locales))
            .collect::<PagesResult<Vec<_>>>()?;

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

        for translation in translations {
            menu_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                menu_id: Set(menu_id),
                locale: Set(translation.locale),
                name: Set(translation.name),
            }
            .insert(&txn)
            .await?;
        }

        for item in items {
            self.create_menu_item_in_tx(&txn, tenant_id, menu_id, None, item)
                .await?;
        }

        txn.commit().await?;
        self.get(
            tenant_id,
            SecurityContext::system(),
            menu_id,
            &effective_locale,
        )
        .await
    }

    fn create_menu_item_in_tx<'a>(
        &'a self,
        txn: &'a DatabaseTransaction,
        tenant_id: Uuid,
        menu_id: Uuid,
        parent_item_id: Option<Uuid>,
        input: PreparedMenuItem,
    ) -> Pin<Box<dyn Future<Output = PagesResult<Uuid>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(page_id) = input.page_id {
                let page_exists = page::Entity::find_by_id(page_id)
                    .filter(page::Column::TenantId.eq(tenant_id))
                    .one(txn)
                    .await?
                    .is_some();
                if !page_exists {
                    return Err(PagesError::validation(format!(
                        "Menu item page `{page_id}` does not belong to tenant `{tenant_id}`"
                    )));
                }
            }

            let now = Utc::now();
            let item_id = Uuid::new_v4();
            menu_item::ActiveModel {
                id: Set(item_id),
                menu_id: Set(menu_id),
                tenant_id: Set(tenant_id),
                parent_item_id: Set(parent_item_id),
                page_id: Set(input.page_id),
                position: Set(input.position),
                url: Set(input.url),
                icon: Set(input.icon),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(txn)
            .await?;

            for translation in input.translations {
                menu_item_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    menu_id: Set(menu_id),
                    menu_item_id: Set(item_id),
                    locale: Set(translation.locale),
                    title: Set(translation.title),
                }
                .insert(txn)
                .await?;
            }

            for child in input.children {
                self.create_menu_item_in_tx(txn, tenant_id, menu_id, Some(item_id), child)
                    .await?;
            }

            Ok(item_id)
        })
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        menu_id: Uuid,
        effective_locale: &str,
    ) -> PagesResult<MenuResponse> {
        enforce_scope(&security, Resource::Pages, Action::Read)?;
        let effective_locale = normalize_effective_locale(effective_locale)?;
        let menu = menu::Entity::find_by_id(menu_id)
            .filter(menu::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| PagesError::menu_not_found(menu_id))?;

        let translations = menu_translation::Entity::find()
            .filter(menu_translation::Column::TenantId.eq(tenant_id))
            .filter(menu_translation::Column::MenuId.eq(menu.id))
            .order_by_asc(menu_translation::Column::Locale)
            .all(&self.db)
            .await?;
        let available_locales = translations
            .iter()
            .map(|translation| translation.locale.clone())
            .collect::<Vec<_>>();
        let name = translations
            .iter()
            .find(|translation| translation.locale == effective_locale)
            .map(|translation| translation.name.clone())
            .ok_or_else(|| menu_locale_not_found(menu.id, &effective_locale))?;
        let items = self
            .load_menu_items(tenant_id, menu.id, &effective_locale)
            .await?;

        Ok(MenuResponse {
            id: menu.id,
            effective_locale,
            available_locales,
            name,
            location: menu_location_from_storage(&menu.location)?,
            items,
        })
    }

    async fn load_menu_items(
        &self,
        tenant_id: Uuid,
        menu_id: Uuid,
        effective_locale: &str,
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

        let item_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
        let translations = menu_item_translation::Entity::find()
            .filter(menu_item_translation::Column::TenantId.eq(tenant_id))
            .filter(menu_item_translation::Column::MenuId.eq(menu_id))
            .filter(menu_item_translation::Column::MenuItemId.is_in(item_ids))
            .filter(menu_item_translation::Column::Locale.eq(effective_locale))
            .all(&self.db)
            .await?;
        let titles_by_item = translations
            .into_iter()
            .map(|translation| (translation.menu_item_id, translation.title))
            .collect::<HashMap<_, _>>();

        let mut items_by_parent: HashMap<Option<Uuid>, Vec<menu_item::Model>> = HashMap::new();
        for item in items {
            items_by_parent
                .entry(item.parent_item_id)
                .or_default()
                .push(item);
        }
        let tree = build_menu_tree(
            None,
            &mut items_by_parent,
            &titles_by_item,
            effective_locale,
        )?;
        if !items_by_parent.is_empty() {
            return Err(menu_integrity_error(format!(
                "Menu `{menu_id}` contains orphaned or cyclic items"
            )));
        }
        Ok(tree)
    }
}

#[derive(Debug)]
struct PreparedMenuTranslation {
    locale: String,
    name: String,
}

#[derive(Debug)]
struct PreparedMenuItemTranslation {
    locale: String,
    title: String,
}

#[derive(Debug)]
struct PreparedMenuItem {
    translations: Vec<PreparedMenuItemTranslation>,
    url: String,
    page_id: Option<Uuid>,
    icon: Option<String>,
    position: i32,
    children: Vec<PreparedMenuItem>,
}

fn normalize_menu_translations(
    translations: Vec<MenuTranslationInput>,
) -> PagesResult<Vec<PreparedMenuTranslation>> {
    if translations.is_empty() {
        return Err(PagesError::validation(
            "At least one menu translation is required",
        ));
    }
    let mut locales = BTreeSet::new();
    let mut prepared = Vec::with_capacity(translations.len());
    for translation in translations {
        let locale = normalize_effective_locale(&translation.locale)?;
        if !locales.insert(locale.clone()) {
            return Err(PagesError::validation(format!(
                "Duplicate normalized menu locale: {locale}"
            )));
        }
        let name = translation.name.trim().to_string();
        if name.is_empty() {
            return Err(PagesError::validation("Menu name cannot be empty"));
        }
        if name.chars().count() > MAX_MENU_NAME_CHARS {
            return Err(PagesError::validation(format!(
                "Menu name cannot exceed {MAX_MENU_NAME_CHARS} characters"
            )));
        }
        prepared.push(PreparedMenuTranslation { locale, name });
    }
    prepared.sort_by(|left, right| left.locale.cmp(&right.locale));
    Ok(prepared)
}

fn normalize_menu_item(
    input: MenuItemInput,
    menu_locales: &BTreeSet<String>,
) -> PagesResult<PreparedMenuItem> {
    if input.translations.is_empty() {
        return Err(PagesError::validation(
            "Every menu item requires translations",
        ));
    }
    let mut locales = BTreeSet::new();
    let mut translations = Vec::with_capacity(input.translations.len());
    for translation in input.translations {
        let locale = normalize_effective_locale(&translation.locale)?;
        if !locales.insert(locale.clone()) {
            return Err(PagesError::validation(format!(
                "Duplicate normalized menu item locale: {locale}"
            )));
        }
        let title = translation.title.trim().to_string();
        if title.is_empty() {
            return Err(PagesError::validation("Menu item title cannot be empty"));
        }
        if title.chars().count() > MAX_MENU_ITEM_TITLE_CHARS {
            return Err(PagesError::validation(format!(
                "Menu item title cannot exceed {MAX_MENU_ITEM_TITLE_CHARS} characters"
            )));
        }
        translations.push(PreparedMenuItemTranslation { locale, title });
    }
    if &locales != menu_locales {
        return Err(PagesError::validation(format!(
            "Menu item locales [{}] must exactly match menu locales [{}]",
            locales.iter().cloned().collect::<Vec<_>>().join(", "),
            menu_locales.iter().cloned().collect::<Vec<_>>().join(", ")
        )));
    }
    translations.sort_by(|left, right| left.locale.cmp(&right.locale));

    let url = input
        .url
        .unwrap_or_else(|| "/".to_string())
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(PagesError::validation("Menu item URL cannot be empty"));
    }
    if url.chars().count() > 2048 {
        return Err(PagesError::validation(
            "Menu item URL cannot exceed 2048 characters",
        ));
    }
    let icon = input
        .icon
        .map(|icon| icon.trim().to_string())
        .filter(|icon| !icon.is_empty());
    let children = input
        .children
        .unwrap_or_default()
        .into_iter()
        .map(|child| normalize_menu_item(child, menu_locales))
        .collect::<PagesResult<Vec<_>>>()?;

    Ok(PreparedMenuItem {
        translations,
        url,
        page_id: input.page_id,
        icon,
        position: input.position,
        children,
    })
}

fn translation_locales(translations: &[PreparedMenuTranslation]) -> BTreeSet<String> {
    translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect()
}

fn normalize_effective_locale(locale: &str) -> PagesResult<String> {
    normalize_locale_code(locale).ok_or_else(|| PagesError::validation("Invalid menu locale"))
}

fn build_menu_tree(
    parent_id: Option<Uuid>,
    items_by_parent: &mut HashMap<Option<Uuid>, Vec<menu_item::Model>>,
    titles_by_item: &HashMap<Uuid, String>,
    effective_locale: &str,
) -> PagesResult<Vec<MenuItemResponse>> {
    let Some(items) = items_by_parent.remove(&parent_id) else {
        return Ok(Vec::new());
    };

    items
        .into_iter()
        .map(|item| {
            let title = titles_by_item.get(&item.id).cloned().ok_or_else(|| {
                menu_integrity_error(format!(
                    "Menu item `{}` has no translation for effective locale `{effective_locale}`",
                    item.id
                ))
            })?;
            let children = build_menu_tree(
                Some(item.id),
                items_by_parent,
                titles_by_item,
                effective_locale,
            )?;
            Ok(MenuItemResponse {
                id: item.id,
                title,
                url: item.url,
                icon: item.icon,
                children,
            })
        })
        .collect()
}

fn menu_locale_not_found(menu_id: Uuid, locale: &str) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::NotFound,
            format!("Menu `{menu_id}` has no translation for effective locale `{locale}`"),
        )
        .with_user_message("The menu is unavailable in the selected language")
        .with_field("menu_id", menu_id.to_string())
        .with_field("locale", locale.to_string())
        .with_error_code(MENU_LOCALE_NOT_FOUND_ERROR_CODE),
    ))
}

fn menu_integrity_error(message: impl Into<String>) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::Internal, message)
            .with_user_message("The localized menu is temporarily unavailable")
            .with_error_code(MENU_TRANSLATION_INTEGRITY_ERROR_CODE),
    ))
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

    #[test]
    fn menu_item_requires_exact_menu_locale_set() {
        let menu_locales = BTreeSet::from(["en".to_string(), "ru".to_string()]);
        let error = normalize_menu_item(
            MenuItemInput {
                translations: vec![MenuItemTranslationInput {
                    locale: "en".to_string(),
                    title: "Home".to_string(),
                }],
                url: Some("/".to_string()),
                page_id: None,
                icon: None,
                position: 0,
                children: None,
            },
            &menu_locales,
        )
        .expect_err("partial locale set must fail");
        assert!(error.to_string().contains("must exactly match"));
    }
}
