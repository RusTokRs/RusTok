use std::collections::{HashMap, HashSet};

use chrono::{DateTime, FixedOffset};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table("brand_products").await? {
            return Ok(());
        }

        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        if manager.has_table("product_brands").await? && manager.has_table("products").await? {
            backfill_brand_products(db, backend).await?;
        }

        db.execute_unprepared("DROP TABLE IF EXISTS brand_products;")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let is_postgres = manager.get_database_backend() == DatabaseBackend::Postgres;

        if is_postgres {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS brand_products (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    brand_id UUID NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_brand_products_unique UNIQUE (tenant_id, brand_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_brand_products_lookup
    ON brand_products (tenant_id, product_id);

CREATE INDEX IF NOT EXISTS idx_brand_products_brand
    ON brand_products (tenant_id, brand_id);
"#,
            )
            .await?;
        } else {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS brand_products (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    brand_id TEXT NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, brand_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_brand_products_lookup
    ON brand_products (tenant_id, product_id);

CREATE INDEX IF NOT EXISTS idx_brand_products_brand
    ON brand_products (tenant_id, brand_id);
"#,
            )
            .await?;
        }

        Ok(())
    }
}

async fn backfill_brand_products(
    db: &impl sea_orm_migration::sea_orm::ConnectionTrait,
    backend: DatabaseBackend,
) -> Result<(), DbErr> {
    let legacy_sql = r#"
SELECT
    bp.id,
    bp.tenant_id,
    bp.brand_id,
    bp.product_id,
    bp.created_at
FROM brand_products bp
INNER JOIN products p
    ON p.id = bp.product_id
    AND p.tenant_id = bp.tenant_id
INNER JOIN brands b
    ON b.id = bp.brand_id
    AND b.tenant_id = bp.tenant_id
ORDER BY bp.tenant_id, bp.product_id, bp.is_primary DESC, bp.created_at ASC, bp.id ASC
"#;

    let legacy_rows = db
        .query_all_raw(Statement::from_string(backend, legacy_sql))
        .await?;
    if legacy_rows.is_empty() {
        return Ok(());
    }

    let existing_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
SELECT
    tenant_id,
    product_id,
    brand_id,
    CAST(CASE WHEN is_primary THEN 1 ELSE 0 END AS BIGINT) AS is_primary_int,
    CAST(position AS BIGINT) AS position_int
FROM product_brands
"#
        }
        _ => {
            r#"
SELECT
    tenant_id,
    product_id,
    brand_id,
    CAST(is_primary AS INTEGER) AS is_primary_int,
    CAST(position AS INTEGER) AS position_int
FROM product_brands
"#
        }
    };

    let existing_rows = db
        .query_all_raw(Statement::from_string(backend, existing_sql))
        .await?;

    let mut existing_links: HashSet<(Uuid, Uuid, Uuid)> = HashSet::new();
    let mut products_with_primary: HashSet<(Uuid, Uuid)> = HashSet::new();
    let mut next_position_by_product: HashMap<(Uuid, Uuid), i32> = HashMap::new();

    for row in existing_rows {
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        let product_id: Uuid = row.try_get("", "product_id")?;
        let brand_id: Uuid = row.try_get("", "brand_id")?;
        let is_primary = row.try_get::<i64>("", "is_primary_int")? != 0;
        let position = row.try_get::<i64>("", "position_int")? as i32;

        existing_links.insert((tenant_id, product_id, brand_id));
        if is_primary {
            products_with_primary.insert((tenant_id, product_id));
        }
        let next_pos = next_position_by_product
            .entry((tenant_id, product_id))
            .or_insert(0);
        if position >= *next_pos {
            *next_pos = position.saturating_add(1);
        }
    }

    for row in legacy_rows {
        let id: Uuid = row.try_get("", "id")?;
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        let brand_id: Uuid = row.try_get("", "brand_id")?;
        let product_id: Uuid = row.try_get("", "product_id")?;

        if !existing_links.insert((tenant_id, product_id, brand_id)) {
            continue;
        }

        let product_key = (tenant_id, product_id);
        let is_primary = !products_with_primary.contains(&product_key);
        if is_primary {
            products_with_primary.insert(product_key);
        }

        let position_entry = next_position_by_product.entry(product_key).or_insert(0);
        let (role, position) = if is_primary {
            if *position_entry == 0 {
                *position_entry = 1;
            }
            ("manufacturer", 0)
        } else {
            let pos = if *position_entry == 0 {
                1
            } else {
                *position_entry
            };
            *position_entry = pos.saturating_add(1);
            ("collaboration", pos)
        };

        let stmt = match backend {
            DatabaseBackend::Postgres => {
                let created_at: DateTime<FixedOffset> = row.try_get("", "created_at")?;
                Statement::from_sql_and_values(
                    backend,
                    r#"
INSERT INTO product_brands (id, tenant_id, product_id, brand_id, is_primary, role, position, created_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT DO NOTHING
"#,
                    vec![
                        id.into(),
                        tenant_id.into(),
                        product_id.into(),
                        brand_id.into(),
                        is_primary.into(),
                        role.into(),
                        position.into(),
                        created_at.into(),
                    ],
                )
            }
            _ => {
                let created_at: String = row.try_get("", "created_at")?;
                Statement::from_sql_and_values(
                    backend,
                    r#"
INSERT OR IGNORE INTO product_brands (id, tenant_id, product_id, brand_id, is_primary, role, position, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
"#,
                    vec![
                        id.into(),
                        tenant_id.into(),
                        product_id.into(),
                        brand_id.into(),
                        i32::from(is_primary).into(),
                        role.into(),
                        position.into(),
                        created_at.into(),
                    ],
                )
            }
        };

        db.execute_raw(stmt).await?;
    }

    Ok(())
}
