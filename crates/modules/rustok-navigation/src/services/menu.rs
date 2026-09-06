use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use rustok_api::{Action, Resource, RuntimeLocale, TenantLocale};
use rustok_core::{
    SecurityContext,
    error::{ErrorKind, RichError},
};

use crate::dto::*;
use crate::entities::{menu, menu_item, menu_item_translation, menu_translation};
use crate::error::{NavigationError, NavigationResult};
use crate::services::rbac::enforce_scope;
use crate::translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx};

pub const MENU_LOCALE_NOT_FOUND_ERROR_CODE: &str = "MENU_LOCALE_NOT_FOUND";
pub const MENU_TRANSLATION_INTEGRITY_ERROR_CODE: &str = "MENU_TRANSLATION_INTEGRITY";

const MAX_MENU_NAME_CHARS: usize = 255;
const MAX_MENU_ITEM_TITLE_CHARS: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplyExactMenuTranslationInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub name: String,
    pub item_titles: BTreeMap<Uuid, String>,
    pub expected_resource_revision: i64,
    pub expected_source_revision: i64,
    pub expected_target_revision: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MenuTranslationApplyResult {
    pub resource_revision: i64,
    pub target_revision: i64,
}

pub struct MenuService {
    db: DatabaseConnection,
}

impl MenuService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        effective_locale: &str,
        input: CreateMenuInput,
    ) -> NavigationResult<MenuResponse> {
        enforce_scope(&security, Resource::Navigation, Action::Create)?;
        let effective_locale = normalize_effective_locale(effective_locale)?;
        let translations = normalize_menu_translations(input.translations)?;
        let menu_locales = translation_locales(&translations);
        if !menu_locales.contains(&effective_locale) {
            return Err(NavigationError::validation(format!(
                "Menu create response locale `{effective_locale}` must be present in menu translations"
            )));
        }
        let items = input
            .items
            .into_iter()
            .map(|item| normalize_menu_item(item, &menu_locales))
            .collect::<NavigationResult<Vec<_>>>()?;

        let now = Utc::now();
        let menu_id = Uuid::new_v4();
        let txn = self.db.begin().await?;
        menu::ActiveModel {
            id: Set(menu_id),
            tenant_id: Set(tenant_id),
            location: Set(menu_location_to_storage(&input.location).to_string()),
            revision: Set(1),
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
                revision: Set(1),
            }
            .insert(&txn)
            .await?;
        }

        for item in items {
            self.create_menu_item_in_tx(&txn, tenant_id, menu_id, None, item)
                .await?;
        }

        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                menu_id,
                locale: &effective_locale,
                resource_revision: 1,
                target_revision: 1,
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;

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
    ) -> Pin<Box<dyn Future<Output = NavigationResult<Uuid>> + Send + 'a>> {
        Box::pin(async move {
            let now = Utc::now();
            let item_id = Uuid::new_v4();
            menu_item::ActiveModel {
                id: Set(item_id),
                menu_id: Set(menu_id),
                tenant_id: Set(tenant_id),
                parent_item_id: Set(parent_item_id),
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

    pub(crate) async fn apply_exact_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        menu_id: Uuid,
        input: ApplyExactMenuTranslationInput,
    ) -> NavigationResult<MenuTranslationApplyResult> {
        if input.source_locale == input.target_locale {
            return Err(NavigationError::validation(
                "Source and target locales must differ for an exact menu translation",
            ));
        }

        let menu = menu::Entity::find_by_id(menu_id)
            .filter(menu::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| NavigationError::menu_not_found(menu_id))?;
        if menu.revision != input.expected_resource_revision {
            return Err(NavigationError::conflict(
                "menu resource revision does not match the translation proposal",
            ));
        }

        let source = menu_translation::Entity::find()
            .filter(menu_translation::Column::TenantId.eq(tenant_id))
            .filter(menu_translation::Column::MenuId.eq(menu_id))
            .filter(menu_translation::Column::Locale.eq(input.source_locale.as_str()))
            .one(txn)
            .await?
            .ok_or_else(|| {
                NavigationError::validation("Exact source menu locale is not present")
            })?;
        if source.revision != input.expected_source_revision {
            return Err(NavigationError::conflict(
                "source menu locale revision does not match the translation proposal",
            ));
        }

        let items = menu_item::Entity::find()
            .filter(menu_item::Column::TenantId.eq(tenant_id))
            .filter(menu_item::Column::MenuId.eq(menu_id))
            .order_by_asc(menu_item::Column::Id)
            .all(txn)
            .await?;
        let item_ids = items.iter().map(|item| item.id).collect::<BTreeSet<_>>();
        let source_item_translations = menu_item_translation::Entity::find()
            .filter(menu_item_translation::Column::TenantId.eq(tenant_id))
            .filter(menu_item_translation::Column::MenuId.eq(menu_id))
            .filter(menu_item_translation::Column::Locale.eq(input.source_locale.as_str()))
            .all(txn)
            .await?;
        let source_item_ids = source_item_translations
            .iter()
            .map(|translation| translation.menu_item_id)
            .collect::<BTreeSet<_>>();
        if source_item_ids != item_ids {
            return Err(NavigationError::validation(
                "Exact source menu locale does not cover every menu item",
            ));
        }
        let supplied_item_ids = input.item_titles.keys().copied().collect::<BTreeSet<_>>();
        if supplied_item_ids != item_ids {
            return Err(NavigationError::validation(
                "Menu translation must provide one title for every menu item",
            ));
        }

        let name = normalize_menu_name(&input.name)?;
        let item_titles = input
            .item_titles
            .into_iter()
            .map(|(item_id, title)| Ok((item_id, normalize_menu_item_title(&title)?)))
            .collect::<NavigationResult<BTreeMap<_, _>>>()?;

        let existing_target = menu_translation::Entity::find()
            .filter(menu_translation::Column::TenantId.eq(tenant_id))
            .filter(menu_translation::Column::MenuId.eq(menu_id))
            .filter(menu_translation::Column::Locale.eq(input.target_locale.as_str()))
            .one(txn)
            .await?;
        let target_item_translations = menu_item_translation::Entity::find()
            .filter(menu_item_translation::Column::TenantId.eq(tenant_id))
            .filter(menu_item_translation::Column::MenuId.eq(menu_id))
            .filter(menu_item_translation::Column::Locale.eq(input.target_locale.as_str()))
            .all(txn)
            .await?;
        let target_items_by_id = target_item_translations
            .into_iter()
            .map(|translation| (translation.menu_item_id, translation))
            .collect::<BTreeMap<_, _>>();
        if existing_target.is_none() && !target_items_by_id.is_empty() {
            return Err(NavigationError::conflict(
                "target menu item translations exist without a target menu translation",
            ));
        }
        if existing_target.is_some()
            && target_items_by_id.keys().copied().collect::<BTreeSet<_>>() != item_ids
        {
            return Err(NavigationError::conflict(
                "target menu locale does not cover every menu item",
            ));
        }

        let target_revision = match existing_target {
            Some(target) => {
                if input.expected_target_revision != Some(target.revision) {
                    return Err(NavigationError::conflict(
                        "target menu locale revision does not match the translation proposal",
                    ));
                }
                let revision =
                    next_menu_translation_revision(menu_id, &target.locale, target.revision)?;
                let updated = menu_translation::Entity::update_many()
                    .col_expr(menu_translation::Column::Name, Expr::value(name.clone()))
                    .col_expr(menu_translation::Column::Revision, Expr::value(revision))
                    .filter(menu_translation::Column::Id.eq(target.id))
                    .filter(menu_translation::Column::Revision.eq(target.revision))
                    .exec(txn)
                    .await?;
                if updated.rows_affected != 1 {
                    return Err(NavigationError::conflict(
                        "target menu locale changed before translation apply could commit",
                    ));
                }
                revision
            }
            None => {
                if input.expected_target_revision.is_some() {
                    return Err(NavigationError::conflict(
                        "translation proposal expected a target menu locale that does not exist",
                    ));
                }
                let inserted = menu_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    menu_id: Set(menu_id),
                    locale: Set(input.target_locale.as_str().to_string()),
                    name: Set(name.clone()),
                    revision: Set(1),
                }
                .insert(txn)
                .await;
                match inserted {
                    Ok(_) => 1,
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(NavigationError::conflict(
                            "target menu locale was created before translation apply could commit",
                        ));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };

        for item in items {
            let title = item_titles
                .get(&item.id)
                .expect("validated menu item title map must include every item")
                .clone();
            match target_items_by_id.get(&item.id) {
                Some(target) => {
                    menu_item_translation::Entity::update_many()
                        .col_expr(menu_item_translation::Column::Title, Expr::value(title))
                        .filter(menu_item_translation::Column::Id.eq(target.id))
                        .exec(txn)
                        .await?;
                }
                None => {
                    menu_item_translation::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        tenant_id: Set(tenant_id),
                        menu_id: Set(menu_id),
                        menu_item_id: Set(item.id),
                        locale: Set(input.target_locale.as_str().to_string()),
                        title: Set(title),
                    }
                    .insert(txn)
                    .await?;
                }
            }
        }

        let resource_revision = next_menu_revision(&menu)?;
        let updated = menu::Entity::update_many()
            .col_expr(menu::Column::Revision, Expr::value(resource_revision))
            .col_expr(
                menu::Column::UpdatedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(menu::Column::Id.eq(menu_id))
            .filter(menu::Column::TenantId.eq(tenant_id))
            .filter(menu::Column::Revision.eq(menu.revision))
            .exec(txn)
            .await?;
        if updated.rows_affected != 1 {
            return Err(NavigationError::conflict(
                "menu changed before translation apply could commit",
            ));
        }

        record_translation_change_in_tx(
            txn,
            TranslationChangeEvidence {
                tenant_id,
                menu_id,
                locale: input.target_locale.as_str(),
                resource_revision,
                target_revision,
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;

        Ok(MenuTranslationApplyResult {
            resource_revision,
            target_revision,
        })
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        menu_id: Uuid,
        effective_locale: &str,
    ) -> NavigationResult<MenuResponse> {
        enforce_scope(&security, Resource::Navigation, Action::Read)?;
        let effective_locale = normalize_effective_locale(effective_locale)?;
        let menu = menu::Entity::find_by_id(menu_id)
            .filter(menu::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| NavigationError::menu_not_found(menu_id))?;

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
    ) -> NavigationResult<Vec<MenuItemResponse>> {
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
    icon: Option<String>,
    position: i32,
    children: Vec<PreparedMenuItem>,
}

fn normalize_menu_translations(
    translations: Vec<MenuTranslationInput>,
) -> NavigationResult<Vec<PreparedMenuTranslation>> {
    if translations.is_empty() {
        return Err(NavigationError::validation(
            "At least one menu translation is required",
        ));
    }
    let mut locales = BTreeSet::new();
    let mut prepared = Vec::with_capacity(translations.len());
    for translation in translations {
        let locale = normalize_effective_locale(&translation.locale)?;
        if !locales.insert(locale.clone()) {
            return Err(NavigationError::validation(format!(
                "Duplicate normalized menu locale: {locale}"
            )));
        }
        let name = normalize_menu_name(&translation.name)?;
        prepared.push(PreparedMenuTranslation { locale, name });
    }
    prepared.sort_by(|left, right| left.locale.cmp(&right.locale));
    Ok(prepared)
}

fn normalize_menu_item(
    input: MenuItemInput,
    menu_locales: &BTreeSet<String>,
) -> NavigationResult<PreparedMenuItem> {
    if input.translations.is_empty() {
        return Err(NavigationError::validation(
            "Every menu item requires translations",
        ));
    }
    let mut locales = BTreeSet::new();
    let mut translations = Vec::with_capacity(input.translations.len());
    for translation in input.translations {
        let locale = normalize_effective_locale(&translation.locale)?;
        if !locales.insert(locale.clone()) {
            return Err(NavigationError::validation(format!(
                "Duplicate normalized menu item locale: {locale}"
            )));
        }
        let title = normalize_menu_item_title(&translation.title)?;
        translations.push(PreparedMenuItemTranslation { locale, title });
    }
    if &locales != menu_locales {
        return Err(NavigationError::validation(format!(
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
        return Err(NavigationError::validation("Menu item URL cannot be empty"));
    }
    if url.chars().count() > 2048 {
        return Err(NavigationError::validation(
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
        .collect::<NavigationResult<Vec<_>>>()?;

    Ok(PreparedMenuItem {
        translations,
        url,
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

fn normalize_menu_name(name: &str) -> NavigationResult<String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(NavigationError::validation("Menu name cannot be empty"));
    }
    if name.chars().count() > MAX_MENU_NAME_CHARS {
        return Err(NavigationError::validation(format!(
            "Menu name cannot exceed {MAX_MENU_NAME_CHARS} characters"
        )));
    }
    Ok(name)
}

fn normalize_menu_item_title(title: &str) -> NavigationResult<String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(NavigationError::validation(
            "Menu item title cannot be empty",
        ));
    }
    if title.chars().count() > MAX_MENU_ITEM_TITLE_CHARS {
        return Err(NavigationError::validation(format!(
            "Menu item title cannot exceed {MAX_MENU_ITEM_TITLE_CHARS} characters"
        )));
    }
    Ok(title)
}

fn next_menu_revision(menu: &menu::Model) -> NavigationResult<i64> {
    menu.revision
        .checked_add(1)
        .filter(|revision| *revision > 0)
        .ok_or_else(|| NavigationError::TranslationRevisionExhausted {
            menu_id: menu.id,
            locale: "resource".to_string(),
        })
}

fn next_menu_translation_revision(
    menu_id: Uuid,
    locale: &str,
    revision: i64,
) -> NavigationResult<i64> {
    revision
        .checked_add(1)
        .filter(|next| *next > 0)
        .ok_or_else(|| NavigationError::TranslationRevisionExhausted {
            menu_id,
            locale: locale.to_string(),
        })
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("unique constraint")
}

fn normalize_effective_locale(locale: &str) -> NavigationResult<String> {
    RuntimeLocale::new(locale)
        .map(RuntimeLocale::into_inner)
        .map_err(|_| NavigationError::validation("Invalid menu locale"))
}

fn build_menu_tree(
    parent_id: Option<Uuid>,
    items_by_parent: &mut HashMap<Option<Uuid>, Vec<menu_item::Model>>,
    titles_by_item: &HashMap<Uuid, String>,
    effective_locale: &str,
) -> NavigationResult<Vec<MenuItemResponse>> {
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

fn menu_locale_not_found(menu_id: Uuid, locale: &str) -> NavigationError {
    NavigationError::Rich(Box::new(
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

fn menu_integrity_error(message: impl Into<String>) -> NavigationError {
    NavigationError::Rich(Box::new(
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

fn menu_location_from_storage(value: &str) -> NavigationResult<MenuLocation> {
    Ok(match value {
        "header" => MenuLocation::Header,
        "footer" => MenuLocation::Footer,
        "sidebar" => MenuLocation::Sidebar,
        "mobile" => MenuLocation::Mobile,
        other => {
            return Err(NavigationError::validation(format!(
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
    fn menu_effective_locale_uses_runtime_locale_contract() {
        assert_eq!(
            normalize_effective_locale("pt_br").expect("valid runtime locale"),
            "pt-BR"
        );
        assert!(normalize_effective_locale("und").is_err());
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
