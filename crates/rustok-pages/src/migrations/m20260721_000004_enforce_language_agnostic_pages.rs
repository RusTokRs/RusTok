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
                "rustok-pages language-agnostic migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible by design. Removing tenant/locale integrity would make
        // current localized records ambiguous.
        Ok(())
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE page_bodies ADD COLUMN IF NOT EXISTS tenant_id UUID;
UPDATE page_bodies body
SET tenant_id = page.tenant_id
FROM pages page
WHERE body.page_id = page.id
  AND body.tenant_id IS NULL;

ALTER TABLE page_published_landing_artifacts
    ADD COLUMN IF NOT EXISTS tenant_id UUID,
    ADD COLUMN IF NOT EXISTS page_id UUID,
    ADD COLUMN IF NOT EXISTS locale VARCHAR(32);
UPDATE page_published_landing_artifacts binding
SET tenant_id = body.tenant_id,
    page_id = body.page_id,
    locale = body.locale
FROM page_bodies body
WHERE binding.page_body_id = body.id
  AND (binding.tenant_id IS NULL OR binding.page_id IS NULL OR binding.locale IS NULL);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM page_translations translation
        JOIN pages page ON page.id = translation.page_id
        WHERE translation.tenant_id IS DISTINCT FROM page.tenant_id
    ) THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: translation tenant mismatch';
    END IF;
    IF EXISTS (SELECT 1 FROM page_bodies WHERE tenant_id IS NULL) THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: orphan page body';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM page_bodies body
        LEFT JOIN page_translations translation
          ON translation.tenant_id = body.tenant_id
         AND translation.page_id = body.page_id
         AND translation.locale = body.locale
        WHERE translation.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: body locale has no translation';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM page_channel_visibility visibility
        JOIN pages page ON page.id = visibility.page_id
        WHERE visibility.tenant_id IS DISTINCT FROM page.tenant_id
    ) THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: channel visibility tenant mismatch';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM page_static_landing_artifacts artifact
        LEFT JOIN page_translations translation
          ON translation.tenant_id = artifact.tenant_id
         AND translation.page_id = artifact.page_id
         AND translation.locale = artifact.locale
        WHERE translation.id IS NULL
    ) THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: artifact locale has no translation';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM page_published_landing_artifacts binding
        JOIN page_bodies body ON body.id = binding.page_body_id
        JOIN page_static_landing_artifacts artifact ON artifact.id = binding.artifact_id
        WHERE binding.tenant_id IS NULL
           OR binding.page_id IS NULL
           OR binding.locale IS NULL
           OR body.tenant_id IS DISTINCT FROM binding.tenant_id
           OR body.page_id IS DISTINCT FROM binding.page_id
           OR body.locale IS DISTINCT FROM binding.locale
           OR artifact.tenant_id IS DISTINCT FROM binding.tenant_id
           OR artifact.page_id IS DISTINCT FROM binding.page_id
           OR artifact.locale IS DISTINCT FROM binding.locale
    ) THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: published artifact locale mismatch';
    END IF;
    IF EXISTS (SELECT 1 FROM page_translations WHERE btrim(slug) = '') THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: empty localized slug';
    END IF;
    IF EXISTS (
        SELECT 1 FROM page_static_landing_artifacts
        WHERE char_length(locale) NOT BETWEEN 2 AND 32
    ) THEN
        RAISE EXCEPTION 'pages language-agnostic migration blocked: artifact locale width violation';
    END IF;
END $$;

ALTER TABLE page_bodies ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE page_published_landing_artifacts
    ALTER COLUMN tenant_id SET NOT NULL,
    ALTER COLUMN page_id SET NOT NULL,
    ALTER COLUMN locale SET NOT NULL;

UPDATE pages
SET metadata = metadata - 'seo'
WHERE metadata ? 'seo';

CREATE UNIQUE INDEX IF NOT EXISTS uq_pages_tenant_id
    ON pages (tenant_id, id);
DROP INDEX IF EXISTS idx_page_translations_page_locale;
CREATE UNIQUE INDEX IF NOT EXISTS uq_page_translations_tenant_page_locale
    ON page_translations (tenant_id, page_id, locale);
DROP INDEX IF EXISTS idx_page_bodies_page_locale;
CREATE UNIQUE INDEX IF NOT EXISTS uq_page_bodies_tenant_page_locale
    ON page_bodies (tenant_id, page_id, locale);
CREATE UNIQUE INDEX IF NOT EXISTS uq_page_bodies_tenant_page_locale_id
    ON page_bodies (tenant_id, page_id, locale, id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_page_static_landing_artifacts_tenant_page_locale_id
    ON page_static_landing_artifacts (tenant_id, page_id, locale, id);

ALTER TABLE page_translations
    DROP CONSTRAINT IF EXISTS fk_page_translations_page;
ALTER TABLE page_translations
    DROP CONSTRAINT IF EXISTS fk_page_translations_tenant_page;
ALTER TABLE page_translations
    ADD CONSTRAINT fk_page_translations_tenant_page
    FOREIGN KEY (tenant_id, page_id)
    REFERENCES pages (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;

ALTER TABLE page_bodies
    DROP CONSTRAINT IF EXISTS fk_page_bodies_page;
ALTER TABLE page_bodies
    DROP CONSTRAINT IF EXISTS fk_page_bodies_translation_locale;
ALTER TABLE page_bodies
    ADD CONSTRAINT fk_page_bodies_translation_locale
    FOREIGN KEY (tenant_id, page_id, locale)
    REFERENCES page_translations (tenant_id, page_id, locale)
    ON UPDATE CASCADE ON DELETE CASCADE;

ALTER TABLE page_channel_visibility
    DROP CONSTRAINT IF EXISTS fk_page_channel_visibility_page;
ALTER TABLE page_channel_visibility
    DROP CONSTRAINT IF EXISTS fk_page_channel_visibility_tenant_page;
ALTER TABLE page_channel_visibility
    ADD CONSTRAINT fk_page_channel_visibility_tenant_page
    FOREIGN KEY (tenant_id, page_id)
    REFERENCES pages (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;

ALTER TABLE page_static_landing_artifacts
    DROP CONSTRAINT IF EXISTS fk_page_static_landing_artifacts_page;
ALTER TABLE page_static_landing_artifacts
    DROP CONSTRAINT IF EXISTS fk_page_static_landing_artifacts_translation_locale;
ALTER TABLE page_static_landing_artifacts
    ADD CONSTRAINT fk_page_static_landing_artifacts_translation_locale
    FOREIGN KEY (tenant_id, page_id, locale)
    REFERENCES page_translations (tenant_id, page_id, locale)
    ON UPDATE CASCADE ON DELETE CASCADE;

ALTER TABLE page_published_landing_artifacts
    DROP CONSTRAINT IF EXISTS fk_page_published_landing_artifacts_body;
ALTER TABLE page_published_landing_artifacts
    DROP CONSTRAINT IF EXISTS fk_page_published_landing_artifacts_artifact;
ALTER TABLE page_published_landing_artifacts
    DROP CONSTRAINT IF EXISTS fk_page_published_landing_artifacts_body_locale;
ALTER TABLE page_published_landing_artifacts
    DROP CONSTRAINT IF EXISTS fk_page_published_landing_artifacts_artifact_locale;
ALTER TABLE page_published_landing_artifacts
    ADD CONSTRAINT fk_page_published_landing_artifacts_body_locale
    FOREIGN KEY (tenant_id, page_id, locale, page_body_id)
    REFERENCES page_bodies (tenant_id, page_id, locale, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE page_published_landing_artifacts
    ADD CONSTRAINT fk_page_published_landing_artifacts_artifact_locale
    FOREIGN KEY (tenant_id, page_id, locale, artifact_id)
    REFERENCES page_static_landing_artifacts (tenant_id, page_id, locale, id)
    ON UPDATE CASCADE ON DELETE RESTRICT;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_page_translations_slug_nonempty') THEN
        ALTER TABLE page_translations
            ADD CONSTRAINT ck_page_translations_slug_nonempty CHECK (btrim(slug) <> '');
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_page_translations_locale_width') THEN
        ALTER TABLE page_translations
            ADD CONSTRAINT ck_page_translations_locale_width CHECK (char_length(locale) BETWEEN 2 AND 32);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_page_bodies_locale_width') THEN
        ALTER TABLE page_bodies
            ADD CONSTRAINT ck_page_bodies_locale_width CHECK (char_length(locale) BETWEEN 2 AND 32);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ck_page_artifacts_locale_width') THEN
        ALTER TABLE page_static_landing_artifacts
            ADD CONSTRAINT ck_page_artifacts_locale_width CHECK (char_length(locale) BETWEEN 2 AND 32);
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
        "ALTER TABLE page_bodies ADD COLUMN tenant_id TEXT",
        "UPDATE page_bodies SET tenant_id = (SELECT page.tenant_id FROM pages page WHERE page.id = page_bodies.page_id) WHERE tenant_id IS NULL",
        "ALTER TABLE page_published_landing_artifacts ADD COLUMN tenant_id TEXT",
        "ALTER TABLE page_published_landing_artifacts ADD COLUMN page_id TEXT",
        "ALTER TABLE page_published_landing_artifacts ADD COLUMN locale TEXT",
        "UPDATE page_published_landing_artifacts SET tenant_id = (SELECT body.tenant_id FROM page_bodies body WHERE body.id = page_published_landing_artifacts.page_body_id), page_id = (SELECT body.page_id FROM page_bodies body WHERE body.id = page_published_landing_artifacts.page_body_id), locale = (SELECT body.locale FROM page_bodies body WHERE body.id = page_published_landing_artifacts.page_body_id) WHERE tenant_id IS NULL OR page_id IS NULL OR locale IS NULL",
        "UPDATE pages SET metadata = json_remove(metadata, '$.seo') WHERE json_valid(metadata) AND json_type(metadata, '$.seo') IS NOT NULL",
    ] {
        connection.execute_unprepared(statement).await?;
    }

    for (sql, message) in [
        (
            "SELECT COUNT(*) AS invalid_count FROM page_translations translation JOIN pages page ON page.id = translation.page_id WHERE translation.tenant_id <> page.tenant_id",
            "pages language-agnostic migration blocked: translation tenant mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM page_bodies WHERE tenant_id IS NULL",
            "pages language-agnostic migration blocked: orphan page body",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM page_bodies body LEFT JOIN page_translations translation ON translation.tenant_id = body.tenant_id AND translation.page_id = body.page_id AND translation.locale = body.locale WHERE translation.id IS NULL",
            "pages language-agnostic migration blocked: body locale has no translation",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM page_channel_visibility visibility JOIN pages page ON page.id = visibility.page_id WHERE visibility.tenant_id <> page.tenant_id",
            "pages language-agnostic migration blocked: channel visibility tenant mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM page_static_landing_artifacts artifact LEFT JOIN page_translations translation ON translation.tenant_id = artifact.tenant_id AND translation.page_id = artifact.page_id AND translation.locale = artifact.locale WHERE translation.id IS NULL",
            "pages language-agnostic migration blocked: artifact locale has no translation",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM page_published_landing_artifacts binding JOIN page_bodies body ON body.id = binding.page_body_id JOIN page_static_landing_artifacts artifact ON artifact.id = binding.artifact_id WHERE binding.tenant_id IS NULL OR binding.page_id IS NULL OR binding.locale IS NULL OR body.tenant_id <> binding.tenant_id OR body.page_id <> binding.page_id OR body.locale <> binding.locale OR artifact.tenant_id <> binding.tenant_id OR artifact.page_id <> binding.page_id OR artifact.locale <> binding.locale",
            "pages language-agnostic migration blocked: published artifact locale mismatch",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM page_translations WHERE trim(slug) = ''",
            "pages language-agnostic migration blocked: empty localized slug",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM page_static_landing_artifacts WHERE length(locale) NOT BETWEEN 2 AND 32",
            "pages language-agnostic migration blocked: artifact locale width violation",
        ),
    ] {
        ensure_sqlite_zero(manager, sql, message).await?;
    }

    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_pages_tenant_id ON pages (tenant_id, id)",
        "DROP INDEX IF EXISTS idx_page_translations_page_locale",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_page_translations_tenant_page_locale ON page_translations (tenant_id, page_id, locale)",
        "DROP INDEX IF EXISTS idx_page_bodies_page_locale",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_page_bodies_tenant_page_locale ON page_bodies (tenant_id, page_id, locale)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_page_bodies_tenant_page_locale_id ON page_bodies (tenant_id, page_id, locale, id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_page_static_landing_artifacts_tenant_page_locale_id ON page_static_landing_artifacts (tenant_id, page_id, locale, id)",
        r#"CREATE TRIGGER IF NOT EXISTS page_translations_language_agnostic_insert BEFORE INSERT ON page_translations FOR EACH ROW WHEN length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR trim(NEW.slug) = '' OR NOT EXISTS (SELECT 1 FROM pages page WHERE page.id = NEW.page_id AND page.tenant_id = NEW.tenant_id) BEGIN SELECT RAISE(ABORT, 'page translation tenant/locale contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_translations_language_agnostic_update BEFORE UPDATE OF tenant_id, page_id, locale, slug ON page_translations FOR EACH ROW WHEN length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR trim(NEW.slug) = '' OR NOT EXISTS (SELECT 1 FROM pages page WHERE page.id = NEW.page_id AND page.tenant_id = NEW.tenant_id) BEGIN SELECT RAISE(ABORT, 'page translation tenant/locale contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_bodies_language_agnostic_insert BEFORE INSERT ON page_bodies FOR EACH ROW WHEN NEW.tenant_id IS NULL OR length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR NOT EXISTS (SELECT 1 FROM page_translations translation WHERE translation.tenant_id = NEW.tenant_id AND translation.page_id = NEW.page_id AND translation.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'page body locale has no matching translation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_bodies_language_agnostic_update BEFORE UPDATE OF tenant_id, page_id, locale ON page_bodies FOR EACH ROW WHEN NEW.tenant_id IS NULL OR length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR NOT EXISTS (SELECT 1 FROM page_translations translation WHERE translation.tenant_id = NEW.tenant_id AND translation.page_id = NEW.page_id AND translation.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'page body locale has no matching translation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_channel_visibility_tenant_insert BEFORE INSERT ON page_channel_visibility FOR EACH ROW WHEN NOT EXISTS (SELECT 1 FROM pages page WHERE page.id = NEW.page_id AND page.tenant_id = NEW.tenant_id) BEGIN SELECT RAISE(ABORT, 'page channel visibility tenant mismatch'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_channel_visibility_tenant_update BEFORE UPDATE OF tenant_id, page_id ON page_channel_visibility FOR EACH ROW WHEN NOT EXISTS (SELECT 1 FROM pages page WHERE page.id = NEW.page_id AND page.tenant_id = NEW.tenant_id) BEGIN SELECT RAISE(ABORT, 'page channel visibility tenant mismatch'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_static_artifacts_language_agnostic_insert BEFORE INSERT ON page_static_landing_artifacts FOR EACH ROW WHEN length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR NOT EXISTS (SELECT 1 FROM page_translations translation WHERE translation.tenant_id = NEW.tenant_id AND translation.page_id = NEW.page_id AND translation.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'page artifact locale has no matching translation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_static_artifacts_language_agnostic_update BEFORE UPDATE OF tenant_id, page_id, locale ON page_static_landing_artifacts FOR EACH ROW WHEN length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR NOT EXISTS (SELECT 1 FROM page_translations translation WHERE translation.tenant_id = NEW.tenant_id AND translation.page_id = NEW.page_id AND translation.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'page artifact locale has no matching translation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_published_artifacts_language_agnostic_insert BEFORE INSERT ON page_published_landing_artifacts FOR EACH ROW WHEN NEW.tenant_id IS NULL OR NEW.page_id IS NULL OR NEW.locale IS NULL OR NOT EXISTS (SELECT 1 FROM page_bodies body WHERE body.id = NEW.page_body_id AND body.tenant_id = NEW.tenant_id AND body.page_id = NEW.page_id AND body.locale = NEW.locale) OR NOT EXISTS (SELECT 1 FROM page_static_landing_artifacts artifact WHERE artifact.id = NEW.artifact_id AND artifact.tenant_id = NEW.tenant_id AND artifact.page_id = NEW.page_id AND artifact.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'published artifact locale binding mismatch'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS page_published_artifacts_language_agnostic_update BEFORE UPDATE OF tenant_id, page_id, locale, page_body_id, artifact_id ON page_published_landing_artifacts FOR EACH ROW WHEN NEW.tenant_id IS NULL OR NEW.page_id IS NULL OR NEW.locale IS NULL OR NOT EXISTS (SELECT 1 FROM page_bodies body WHERE body.id = NEW.page_body_id AND body.tenant_id = NEW.tenant_id AND body.page_id = NEW.page_id AND body.locale = NEW.locale) OR NOT EXISTS (SELECT 1 FROM page_static_landing_artifacts artifact WHERE artifact.id = NEW.artifact_id AND artifact.tenant_id = NEW.tenant_id AND artifact.page_id = NEW.page_id AND artifact.locale = NEW.locale) BEGIN SELECT RAISE(ABORT, 'published artifact locale binding mismatch'); END"#,
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
        .ok_or_else(|| {
            DbErr::Custom("failed to validate Pages language-agnostic migration".into())
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(message.to_owned()));
    }
    Ok(())
}
