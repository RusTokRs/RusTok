use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-pages menu effective-locale migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible by design. Removing tenant-composite menu ownership would
        // make localized navigation records ambiguous.
        Ok(())
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE menu_translations ADD COLUMN IF NOT EXISTS tenant_id UUID;
UPDATE menu_translations translation
SET tenant_id = menu.tenant_id
FROM menus menu
WHERE translation.menu_id = menu.id
  AND translation.tenant_id IS NULL;

ALTER TABLE menu_item_translations
    ADD COLUMN IF NOT EXISTS tenant_id UUID,
    ADD COLUMN IF NOT EXISTS menu_id UUID;
UPDATE menu_item_translations translation
SET tenant_id = item.tenant_id,
    menu_id = item.menu_id
FROM menu_items item
WHERE translation.menu_item_id = item.id
  AND (translation.tenant_id IS NULL OR translation.menu_id IS NULL);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM menu_translations translation
        LEFT JOIN menus menu
          ON menu.id = translation.menu_id
         AND menu.tenant_id = translation.tenant_id
        WHERE translation.tenant_id IS NULL OR menu.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: menu translation tenant mismatch';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM menu_items item
        LEFT JOIN menus menu
          ON menu.id = item.menu_id
         AND menu.tenant_id = item.tenant_id
        WHERE menu.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: menu item tenant mismatch';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM menu_items item
        LEFT JOIN menu_items parent
          ON parent.id = item.parent_item_id
         AND parent.tenant_id = item.tenant_id
         AND parent.menu_id = item.menu_id
        WHERE item.parent_item_id IS NOT NULL AND parent.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: menu item parent mismatch';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM menu_items item
        LEFT JOIN pages page
          ON page.id = item.page_id
         AND page.tenant_id = item.tenant_id
        WHERE item.page_id IS NOT NULL AND page.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: menu item page tenant mismatch';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM menu_item_translations translation
        LEFT JOIN menu_items item
          ON item.id = translation.menu_item_id
         AND item.tenant_id = translation.tenant_id
         AND item.menu_id = translation.menu_id
        WHERE translation.tenant_id IS NULL
           OR translation.menu_id IS NULL
           OR item.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: item translation tenant mismatch';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM menu_item_translations item_translation
        LEFT JOIN menu_translations menu_translation
          ON menu_translation.tenant_id = item_translation.tenant_id
         AND menu_translation.menu_id = item_translation.menu_id
         AND menu_translation.locale = item_translation.locale
        WHERE menu_translation.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: item locale has no menu translation';
    END IF;
    IF EXISTS (
        SELECT 1 FROM menu_translations
        WHERE char_length(locale) NOT BETWEEN 2 AND 32
           OR btrim(name) = ''
           OR char_length(name) > 255
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: invalid menu translation';
    END IF;
    IF EXISTS (
        SELECT 1 FROM menu_item_translations
        WHERE char_length(locale) NOT BETWEEN 2 AND 32
           OR btrim(title) = ''
           OR char_length(title) > 255
    ) THEN
        RAISE EXCEPTION 'pages menu migration blocked: invalid menu item translation';
    END IF;
    IF EXISTS (SELECT 1 FROM menu_items WHERE btrim(url) = '') THEN
        RAISE EXCEPTION 'pages menu migration blocked: empty menu item URL';
    END IF;
END $$;

ALTER TABLE menu_translations ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE menu_item_translations
    ALTER COLUMN tenant_id SET NOT NULL,
    ALTER COLUMN menu_id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_menus_tenant_id
    ON menus (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_menu_items_tenant_menu_id
    ON menu_items (tenant_id, menu_id, id);
DROP INDEX IF EXISTS idx_menu_translations_menu_locale;
CREATE UNIQUE INDEX IF NOT EXISTS uq_menu_translations_tenant_menu_locale
    ON menu_translations (tenant_id, menu_id, locale);
DROP INDEX IF EXISTS idx_menu_item_translations_item_locale;
CREATE UNIQUE INDEX IF NOT EXISTS uq_menu_item_translations_tenant_menu_item_locale
    ON menu_item_translations (tenant_id, menu_id, menu_item_id, locale);

ALTER TABLE menu_translations
    DROP CONSTRAINT IF EXISTS fk_menu_translations_menu;
ALTER TABLE menu_translations
    DROP CONSTRAINT IF EXISTS fk_menu_translations_tenant_menu;
ALTER TABLE menu_translations
    ADD CONSTRAINT fk_menu_translations_tenant_menu
    FOREIGN KEY (tenant_id, menu_id)
    REFERENCES menus (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;

ALTER TABLE menu_items
    DROP CONSTRAINT IF EXISTS fk_menu_items_menu;
ALTER TABLE menu_items
    DROP CONSTRAINT IF EXISTS fk_menu_items_parent;
ALTER TABLE menu_items
    DROP CONSTRAINT IF EXISTS fk_menu_items_tenant_menu;
ALTER TABLE menu_items
    DROP CONSTRAINT IF EXISTS fk_menu_items_parent_same_menu;
ALTER TABLE menu_items
    DROP CONSTRAINT IF EXISTS fk_menu_items_tenant_page;
ALTER TABLE menu_items
    ADD CONSTRAINT fk_menu_items_tenant_menu
    FOREIGN KEY (tenant_id, menu_id)
    REFERENCES menus (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE menu_items
    ADD CONSTRAINT fk_menu_items_parent_same_menu
    FOREIGN KEY (tenant_id, menu_id, parent_item_id)
    REFERENCES menu_items (tenant_id, menu_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE menu_items
    ADD CONSTRAINT fk_menu_items_tenant_page
    FOREIGN KEY (tenant_id, page_id)
    REFERENCES pages (tenant_id, id)
    ON UPDATE CASCADE ON DELETE RESTRICT;

ALTER TABLE menu_item_translations
    DROP CONSTRAINT IF EXISTS fk_menu_item_translations_menu_item;
ALTER TABLE menu_item_translations
    DROP CONSTRAINT IF EXISTS fk_menu_item_translations_tenant_item;
ALTER TABLE menu_item_translations
    DROP CONSTRAINT IF EXISTS fk_menu_item_translations_menu_locale;
ALTER TABLE menu_item_translations
    ADD CONSTRAINT fk_menu_item_translations_tenant_item
    FOREIGN KEY (tenant_id, menu_id, menu_item_id)
    REFERENCES menu_items (tenant_id, menu_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE menu_item_translations
    ADD CONSTRAINT fk_menu_item_translations_menu_locale
    FOREIGN KEY (tenant_id, menu_id, locale)
    REFERENCES menu_translations (tenant_id, menu_id, locale)
    ON UPDATE CASCADE ON DELETE CASCADE;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_menu_translations_locale_shape') THEN
        ALTER TABLE menu_translations
            ADD CONSTRAINT ck_menu_translations_locale_shape
            CHECK (char_length(locale) BETWEEN 2 AND 32);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_menu_translations_name_shape') THEN
        ALTER TABLE menu_translations
            ADD CONSTRAINT ck_menu_translations_name_shape
            CHECK (btrim(name) <> '' AND char_length(name) <= 255);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_menu_item_translations_locale_shape') THEN
        ALTER TABLE menu_item_translations
            ADD CONSTRAINT ck_menu_item_translations_locale_shape
            CHECK (char_length(locale) BETWEEN 2 AND 32);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_menu_item_translations_title_shape') THEN
        ALTER TABLE menu_item_translations
            ADD CONSTRAINT ck_menu_item_translations_title_shape
            CHECK (btrim(title) <> '' AND char_length(title) <= 255);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_menu_items_url_nonempty') THEN
        ALTER TABLE menu_items
            ADD CONSTRAINT ck_menu_items_url_nonempty CHECK (btrim(url) <> '');
    END IF;
END $$;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "ALTER TABLE menu_translations ADD COLUMN tenant_id TEXT",
        "UPDATE menu_translations SET tenant_id = (SELECT menu.tenant_id FROM menus menu WHERE menu.id = menu_translations.menu_id) WHERE tenant_id IS NULL",
        "ALTER TABLE menu_item_translations ADD COLUMN tenant_id TEXT",
        "ALTER TABLE menu_item_translations ADD COLUMN menu_id TEXT",
        "UPDATE menu_item_translations SET tenant_id = (SELECT item.tenant_id FROM menu_items item WHERE item.id = menu_item_translations.menu_item_id), menu_id = (SELECT item.menu_id FROM menu_items item WHERE item.id = menu_item_translations.menu_item_id) WHERE tenant_id IS NULL OR menu_id IS NULL",
    ] {
        connection.execute_unprepared(statement).await?;
    }

    for (sql, message) in [
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_translations translation LEFT JOIN menus menu ON menu.id = translation.menu_id AND menu.tenant_id = translation.tenant_id WHERE translation.tenant_id IS NULL OR menu.id IS NULL",
            "pages menu migration blocked: menu translation tenant mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_items item LEFT JOIN menus menu ON menu.id = item.menu_id AND menu.tenant_id = item.tenant_id WHERE menu.id IS NULL",
            "pages menu migration blocked: menu item tenant mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_items item LEFT JOIN menu_items parent ON parent.id = item.parent_item_id AND parent.tenant_id = item.tenant_id AND parent.menu_id = item.menu_id WHERE item.parent_item_id IS NOT NULL AND parent.id IS NULL",
            "pages menu migration blocked: menu item parent mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_items item LEFT JOIN pages page ON page.id = item.page_id AND page.tenant_id = item.tenant_id WHERE item.page_id IS NOT NULL AND page.id IS NULL",
            "pages menu migration blocked: menu item page tenant mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_item_translations translation LEFT JOIN menu_items item ON item.id = translation.menu_item_id AND item.tenant_id = translation.tenant_id AND item.menu_id = translation.menu_id WHERE translation.tenant_id IS NULL OR translation.menu_id IS NULL OR item.id IS NULL",
            "pages menu migration blocked: item translation tenant mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_item_translations item_translation LEFT JOIN menu_translations menu_translation ON menu_translation.tenant_id = item_translation.tenant_id AND menu_translation.menu_id = item_translation.menu_id AND menu_translation.locale = item_translation.locale WHERE menu_translation.id IS NULL",
            "pages menu migration blocked: item locale has no menu translation",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_translations WHERE length(locale) NOT BETWEEN 2 AND 32 OR trim(name) = '' OR length(name) > 255",
            "pages menu migration blocked: invalid menu translation",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_item_translations WHERE length(locale) NOT BETWEEN 2 AND 32 OR trim(title) = '' OR length(title) > 255",
            "pages menu migration blocked: invalid menu item translation",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM menu_items WHERE trim(url) = ''",
            "pages menu migration blocked: empty menu item URL",
        ),
    ] {
        ensure_sqlite_zero(manager, sql, message).await?;
    }

    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_menus_tenant_id ON menus (tenant_id, id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_menu_items_tenant_menu_id ON menu_items (tenant_id, menu_id, id)",
        "DROP INDEX IF EXISTS idx_menu_translations_menu_locale",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_menu_translations_tenant_menu_locale ON menu_translations (tenant_id, menu_id, locale)",
        "DROP INDEX IF EXISTS idx_menu_item_translations_item_locale",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_menu_item_translations_tenant_menu_item_locale ON menu_item_translations (tenant_id, menu_id, menu_item_id, locale)",
        r#"CREATE TRIGGER IF NOT EXISTS menu_translations_effective_locale_insert BEFORE INSERT ON menu_translations FOR EACH ROW WHEN NEW.tenant_id IS NULL OR length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR trim(NEW.name) = '' OR length(NEW.name) > 255 OR NOT EXISTS (SELECT 1 FROM menus menu WHERE menu.id = NEW.menu_id AND menu.tenant_id = NEW.tenant_id) BEGIN SELECT RAISE(ABORT, 'menu translation tenant/locale contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS menu_translations_effective_locale_update BEFORE UPDATE OF tenant_id, menu_id, locale, name ON menu_translations FOR EACH ROW WHEN NEW.tenant_id IS NULL OR length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR trim(NEW.name) = '' OR length(NEW.name) > 255 OR NOT EXISTS (SELECT 1 FROM menus menu WHERE menu.id = NEW.menu_id AND menu.tenant_id = NEW.tenant_id) BEGIN SELECT RAISE(ABORT, 'menu translation tenant/locale contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS menu_translations_effective_locale_cascade_update AFTER UPDATE OF tenant_id, menu_id, locale ON menu_translations FOR EACH ROW BEGIN UPDATE menu_item_translations SET tenant_id = NEW.tenant_id, menu_id = NEW.menu_id, locale = NEW.locale WHERE tenant_id = OLD.tenant_id AND menu_id = OLD.menu_id AND locale = OLD.locale; END"#,
        r#"CREATE TRIGGER IF NOT EXISTS menu_translations_effective_locale_cascade_delete AFTER DELETE ON menu_translations FOR EACH ROW BEGIN DELETE FROM menu_item_translations WHERE tenant_id = OLD.tenant_id AND menu_id = OLD.menu_id AND locale = OLD.locale; END"#,
        r#"CREATE TRIGGER IF NOT EXISTS menu_items_tenant_contract_insert BEFORE INSERT ON menu_items FOR EACH ROW WHEN trim(NEW.url) = '' OR NOT EXISTS (SELECT 1 FROM menus menu WHERE menu.id = NEW.menu_id AND menu.tenant_id = NEW.tenant_id) OR (NEW.parent_item_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM menu_items parent WHERE parent.id = NEW.parent_item_id AND parent.tenant_id = NEW.tenant_id AND parent.menu_id = NEW.menu_id)) OR (NEW.page_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM pages page WHERE page.id = NEW.page_id AND page.tenant_id = NEW.tenant_id)) BEGIN SELECT RAISE(ABORT, 'menu item tenant contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS menu_items_tenant_contract_update BEFORE UPDATE OF tenant_id, menu_id, parent_item_id, page_id, url ON menu_items FOR EACH ROW WHEN trim(NEW.url) = '' OR NOT EXISTS (SELECT 1 FROM menus menu WHERE menu.id = NEW.menu_id AND menu.tenant_id = NEW.tenant_id) OR (NEW.parent_item_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM menu_items parent WHERE parent.id = NEW.parent_item_id AND parent.tenant_id = NEW.tenant_id AND parent.menu_id = NEW.menu_id)) OR (NEW.page_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM pages page WHERE page.id = NEW.page_id AND page.tenant_id = NEW.tenant_id)) BEGIN SELECT RAISE(ABORT, 'menu item tenant contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS menu_item_translations_effective_locale_insert BEFORE INSERT ON menu_item_translations FOR EACH ROW WHEN NEW.tenant_id IS NULL OR NEW.menu_id IS NULL OR length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR trim(NEW.title) = '' OR length(NEW.title) > 255 OR NOT EXISTS (SELECT 1 FROM menu_items item WHERE item.id = NEW.menu_item_id AND item.tenant_id = NEW.tenant_id AND item.menu_id = NEW.menu_id) OR NOT EXISTS (SELECT 1 FROM menu_translations translation WHERE translation.tenant_id = NEW.tenant_id AND translation.menu_id = NEW.menu_id AND translation.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'menu item translation tenant/locale contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS menu_item_translations_effective_locale_update BEFORE UPDATE OF tenant_id, menu_id, menu_item_id, locale, title ON menu_item_translations FOR EACH ROW WHEN NEW.tenant_id IS NULL OR NEW.menu_id IS NULL OR length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR trim(NEW.title) = '' OR length(NEW.title) > 255 OR NOT EXISTS (SELECT 1 FROM menu_items item WHERE item.id = NEW.menu_item_id AND item.tenant_id = NEW.tenant_id AND item.menu_id = NEW.menu_id) OR NOT EXISTS (SELECT 1 FROM menu_translations translation WHERE translation.tenant_id = NEW.tenant_id AND translation.menu_id = NEW.menu_id AND translation.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'menu item translation tenant/locale contract violation'); END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_sqlite_zero(
    manager: &SchemaManager<'_>,
    sql: &str,
    message: &str,
) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_owned(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("failed to validate Pages menu migration".into()))?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(message.to_owned()));
    }
    Ok(())
}
