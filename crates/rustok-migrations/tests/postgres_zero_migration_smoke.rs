use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustok_migrations::Migrator;
use sea_orm_migration::{
    prelude::MigratorTrait,
    sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    },
};

#[tokio::test]
#[ignore = "requires PostgreSQL admin access; run scripts/verify/verify-migration-smoke.sh"]
async fn postgres_zero_migration_smoke_applies_from_empty_database() {
    if let Err(error) = run_postgres_zero_migration_smoke().await {
        panic!("PostgreSQL migration smoke failed: {error}");
    }
}

async fn run_postgres_zero_migration_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name =
        std::env::var("RUSTOK_MIGRATION_SMOKE_DB_NAME").unwrap_or_else(|_| default_database_name());
    assert_valid_database_name(&database_name);

    let target_url = database_url_from_admin_url(&admin_url, &database_name);
    let keep_database = env_binary_flag("RUSTOK_MIGRATION_SMOKE_KEEP_DB")?;
    let incremental = env_binary_flag("RUSTOK_MIGRATION_SMOKE_INCREMENTAL")?;
    let reuse_database = env_binary_flag("RUSTOK_MIGRATION_SMOKE_REUSE_DB")?;

    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;

    if reuse_database {
        connect_postgres(&target_url)
            .await
            .map_err(|error| {
                format!(
                    "reused migration smoke database {database_name} must already exist and be reachable: {error}"
                )
            })?;
    } else {
        drop_database_if_exists(&admin, &database_name).await?;
        create_database(&admin, &database_name).await?;
    }

    let smoke_result = apply_migrations_and_assert_schema(&target_url, incremental).await;

    if keep_database {
        eprintln!("Keeping migration smoke database '{database_name}' at {target_url}");
    } else {
        drop_database_if_exists(&admin, &database_name).await?;
    }

    smoke_result
}

async fn apply_migrations_and_assert_schema(
    target_url: &str,
    incremental: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = connect_postgres(target_url)
        .await
        .map_err(|error| format!("smoke database must be reachable: {error}"))?;

    if incremental {
        apply_migrations_incrementally(&db).await?;
    } else {
        Migrator::up(&db, None).await.map_err(|error| {
            format!("server migrator must apply all pending PostgreSQL migrations: {error}")
        })?;
    }

    let pending = Migrator::get_pending_migrations(&db)
        .await
        .map_err(|error| format!("pending migration list must be readable: {error}"))?;
    if !pending.is_empty() {
        let pending_names = pending
            .iter()
            .map(|migration| migration.name().to_string())
            .collect::<Vec<_>>();
        return Err(format!("all migrations should be applied, pending: {pending_names:?}").into());
    }

    for table in [
        "tenants",
        "users",
        "product_variants",
        "prices",
        "inventory_items",
        "channels",
        "oauth_apps",
        "blog_post_tags",
        "forum_topic_tags",
        "taxonomy_terms",
    ] {
        assert_table_exists(&db, table).await?;
    }

    for constraint in [
        "uq_product_translations_tenant_id",
        "fk_product_translations_product_tenant",
        "fk_product_tags_product_tenant",
        "fk_product_tags_term_tenant",
        "chk_product_categories_no_primary_assignment",
        "chk_product_attribute_values_one_scalar",
        "chk_product_variant_attribute_values_one_scalar",
    ] {
        assert_constraint_exists(&db, constraint).await?;
    }
    for index in [
        "uq_product_translations_tenant_locale_handle",
        "uq_product_variants_tenant_sku",
        "uq_catalog_categories_tenant_root_slug",
        "idx_products_storefront_published",
        "idx_products_channel_visibility_jsonb",
    ] {
        assert_index_exists(&db, index).await?;
    }
    assert_trigger_exists(&db, "trg_products_normalize_channel_visibility").await?;

    Ok(())
}

async fn apply_migrations_incrementally(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let pending = Migrator::get_pending_migrations(db)
            .await
            .map_err(|error| format!("pending migration list must be readable: {error}"))?;
        let Some(next) = pending.first() else {
            return Ok(());
        };
        let next_name = next.name().to_string();

        Migrator::up(db, Some(1)).await.map_err(|error| {
            format!(
                "server migrator must apply incremental PostgreSQL migration {next_name}: {error}"
            )
        })?;
    }
}

async fn connect_postgres(
    url: &str,
) -> Result<DatabaseConnection, sea_orm_migration::sea_orm::DbErr> {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options).await
}

async fn create_database(
    admin: &DatabaseConnection,
    database_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    admin
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!("CREATE DATABASE {}", quoted_identifier(database_name)),
        ))
        .await
        .map_err(|error| format!("failed to create smoke database {database_name}: {error}"))?;
    Ok(())
}

async fn drop_database_if_exists(
    admin: &DatabaseConnection,
    database_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    admin
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                quoted_identifier(database_name)
            ),
        ))
        .await
        .map_err(|error| format!("failed to drop smoke database {database_name}: {error}"))?;
    Ok(())
}

async fn assert_table_exists(
    db: &DatabaseConnection,
    table: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1) IS NOT NULL AS exists",
            [format!("public.{table}").into()],
        ))
        .await
        .map_err(|error| format!("table existence query for {table} must succeed: {error}"))?
        .ok_or_else(|| format!("table existence query for {table} returned no row"))?;
    let exists: bool = row
        .try_get("", "exists")
        .map_err(|error| format!("table existence result for {table} must decode: {error}"))?;
    if !exists {
        return Err(format!("expected table {table} to exist after migrations").into());
    }
    Ok(())
}

async fn assert_constraint_exists(
    db: &DatabaseConnection,
    constraint: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_exists(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = $1) AS exists",
        constraint,
        "constraint",
    )
    .await
}

async fn assert_index_exists(
    db: &DatabaseConnection,
    index: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_exists(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_class WHERE relkind = 'i' AND relname = $1) AS exists",
        index,
        "index",
    )
    .await
}

async fn assert_trigger_exists(
    db: &DatabaseConnection,
    trigger: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_exists(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = $1 AND NOT tgisinternal) AS exists",
        trigger,
        "trigger",
    )
    .await
}

async fn assert_catalog_object_exists(
    db: &DatabaseConnection,
    query: &str,
    name: &str,
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            query,
            [name.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| format!("{kind} existence query returned no row"))?;
    let exists: bool = row.try_get("", "exists")?;
    if !exists {
        return Err(format!("expected {kind} `{name}` after migrations").into());
    }
    Ok(())
}

fn env_binary_flag(name: &str) -> Result<bool, Box<dyn std::error::Error>> {
    parse_binary_flag(name, std::env::var(name).ok().as_deref())
}

fn parse_binary_flag(name: &str, value: Option<&str>) -> Result<bool, Box<dyn std::error::Error>> {
    match value.unwrap_or("0") {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(format!("{name} must be 0 or 1, got {other:?}").into()),
    }
}

fn assert_postgres_url(url: &str) {
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "RUSTOK_MIGRATION_SMOKE_ADMIN_URL must use postgres:// or postgresql://"
    );
}

fn assert_valid_database_name(database_name: &str) {
    let mut chars = database_name.chars();
    let Some(first) = chars.next() else {
        panic!("RUSTOK_MIGRATION_SMOKE_DB_NAME must not be empty");
    };
    assert!(
        first == '_' || first.is_ascii_alphabetic(),
        "RUSTOK_MIGRATION_SMOKE_DB_NAME must start with a letter or underscore"
    );
    assert!(
        chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()),
        "RUSTOK_MIGRATION_SMOKE_DB_NAME may contain only letters, digits, and underscores"
    );
}

fn database_url_from_admin_url(admin_url: &str, database_name: &str) -> String {
    let (base, suffix) = admin_url
        .split_once('?')
        .map(|(base, query)| (base, format!("?{query}")))
        .unwrap_or((admin_url, String::new()));
    let scheme_end = base
        .find("://")
        .expect("PostgreSQL URL must include a scheme separator")
        + 3;
    let authority_end = base[scheme_end..]
        .find('/')
        .map(|offset| scheme_end + offset)
        .unwrap_or(base.len());
    format!(
        "{}{}/{}{}",
        &base[..authority_end],
        "",
        database_name,
        suffix
    )
}

fn default_database_name() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX_EPOCH")
        .as_millis();
    format!("rustok_migration_smoke_{millis}_{}", std::process::id())
}

fn quoted_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::parse_binary_flag;

    #[test]
    fn binary_flag_defaults_to_false_when_missing() {
        assert!(
            !parse_binary_flag("RUSTOK_MIGRATION_SMOKE_INCREMENTAL", None)
                .expect("missing flag should default to false")
        );
    }

    #[test]
    fn binary_flag_accepts_zero_and_one_only() {
        assert!(
            !parse_binary_flag("RUSTOK_MIGRATION_SMOKE_INCREMENTAL", Some("0"))
                .expect("0 should be accepted")
        );
        assert!(
            parse_binary_flag("RUSTOK_MIGRATION_SMOKE_INCREMENTAL", Some("1"))
                .expect("1 should be accepted")
        );
        assert!(
            parse_binary_flag("RUSTOK_MIGRATION_SMOKE_REUSE_DB", Some("true"))
                .expect_err("non-binary values should be rejected")
                .to_string()
                .contains("must be 0 or 1")
        );
    }
}
