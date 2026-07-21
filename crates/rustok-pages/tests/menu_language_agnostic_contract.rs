#[test]
fn menu_runtime_uses_host_selected_effective_locale_only() {
    let service = include_str!("../src/services/menu.rs");
    let dto = include_str!("../src/dto/menu.rs");

    assert!(service.contains("normalize_effective_locale"));
    assert!(service.contains("Column::Locale.eq(effective_locale)"));
    assert!(service.contains("MENU_LOCALE_NOT_FOUND"));
    assert!(service.contains("MENU_TRANSLATION_INTEGRITY"));
    assert!(!service.contains("PLATFORM_FALLBACK_LOCALE"));
    assert!(!service.contains("translations.first()"));
    assert!(!service.contains("or_insert(translation.title)"));

    assert!(dto.contains("pub translations: Vec<MenuTranslationInput>"));
    assert!(dto.contains("pub translations: Vec<MenuItemTranslationInput>"));
    assert!(dto.contains("pub effective_locale: String"));
    assert!(dto.contains("pub available_locales: Vec<String>"));
    assert!(dto.contains("pub title: String"));
    assert!(!dto.contains("pub title: Option<String>"));
}

#[test]
fn menu_storage_is_tenant_composite_and_locale_bound() {
    let migration =
        include_str!("../src/migrations/m20260721_000005_enforce_menu_effective_locale.rs");
    let migrations = include_str!("../src/migrations/mod.rs");
    let menu_translation = include_str!("../src/entities/menu_translation.rs");
    let item_translation = include_str!("../src/entities/menu_item_translation.rs");

    for marker in [
        "fk_menu_translations_tenant_menu",
        "fk_menu_items_parent_same_menu",
        "fk_menu_items_tenant_page",
        "fk_menu_item_translations_tenant_item",
        "fk_menu_item_translations_menu_locale",
        "menu_item_translations_effective_locale_insert",
        "menu_items_tenant_contract_insert",
        "Irreversible by design",
    ] {
        assert!(migration.contains(marker), "missing DB marker: {marker}");
    }
    assert!(migrations.contains("mod m20260721_000005_enforce_menu_effective_locale;"));
    assert!(migrations.contains(
        "Box::new(m20260721_000005_enforce_menu_effective_locale::Migration)"
    ));
    assert!(menu_translation.contains("pub tenant_id: Uuid"));
    assert!(item_translation.contains("pub tenant_id: Uuid"));
    assert!(item_translation.contains("pub menu_id: Uuid"));
}
