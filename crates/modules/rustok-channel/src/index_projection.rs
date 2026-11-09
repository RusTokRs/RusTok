//! Channel-owned read projections for embedded Index and relation adapters.

use sea_orm::{DbBackend, Statement, Value};
use uuid::Uuid;

pub const SALES_CHANNEL_ROWS_CTE: &str = r#"
sales_channel_index_union AS (
    SELECT
        FALSE AS is_deleted,
        c.tenant_id,
        c.id AS channel_id,
        c.index_revision,
        c.slug,
        c.name,
        c.is_active,
        c.is_default,
        c.status
    FROM channels c
    WHERE c.tenant_id = $1

    UNION ALL

    SELECT
        TRUE AS is_deleted,
        tombstone.tenant_id,
        tombstone.channel_id,
        tombstone.source_version AS index_revision,
        NULL::text AS slug,
        NULL::text AS name,
        NULL::boolean AS is_active,
        NULL::boolean AS is_default,
        NULL::text AS status
    FROM channel_index_tombstones tombstone
    WHERE tombstone.tenant_id = $1
),
sales_channel_index_rows AS (
    SELECT
        row.*,
        COUNT(*) OVER (
            PARTITION BY row.tenant_id, row.channel_id
        ) AS identity_count
    FROM sales_channel_index_union row
)
"#;

pub const SALES_CHANNEL_ROW_SELECT: &str = r#"
SELECT
    row.is_deleted,
    row.identity_count,
    row.tenant_id,
    row.channel_id,
    row.index_revision,
    row.slug,
    row.name,
    row.is_active,
    row.is_default,
    row.status
FROM sales_channel_index_rows row
"#;

pub const SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS: &str = r#"
EXISTS (
    SELECT 1
    FROM channels owner_channel
    WHERE owner_channel.tenant_id = {{entity}}.tenant_id
      AND owner_channel.id = {{entity}}.entity_id
      AND owner_channel.index_revision = {{entity}}.source_version
)
"#;

/// Native relation that callers can compose in the same PostgreSQL statement.
pub const POSTGRES_IDENTITY_GENERATIONS: &str =
    "(SELECT tenant_id, generation FROM channel_index_identity_generations)";

/// An absent generation row means the tenant has not published a generation.
pub fn identity_generation(tenant_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT generation FROM {POSTGRES_IDENTITY_GENERATIONS} generations WHERE tenant_id = $1"
        ),
        [tenant_id.into()],
    )
}

/// Ordered identity projection. Inactive channels remain valid relation members.
pub fn relation_channel_ids(
    tenant_id: Uuid,
    slugs: Option<&[String]>,
    fetch_limit: i64,
) -> Statement {
    let (sql, values) = match slugs {
        None => (
            "SELECT id FROM channels WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2".to_owned(),
            vec![tenant_id.into(), fetch_limit.into()],
        ),
        Some([]) => (
            "SELECT id FROM channels WHERE tenant_id = $1 AND FALSE LIMIT $2".to_owned(),
            vec![tenant_id.into(), fetch_limit.into()],
        ),
        Some(slugs) => {
            let mut values = Vec::<Value>::with_capacity(slugs.len() + 2);
            values.push(tenant_id.into());
            let mut placeholders = Vec::with_capacity(slugs.len());
            for (offset, slug) in slugs.iter().enumerate() {
                placeholders.push(format!("${}", offset + 2));
                values.push(slug.clone().into());
            }
            let limit_parameter = slugs.len() + 2;
            values.push(fetch_limit.into());
            (
                format!(
                    "SELECT id FROM channels WHERE tenant_id = $1 AND lower(btrim(slug)) IN ({}) ORDER BY id ASC LIMIT ${limit_parameter}",
                    placeholders.join(", ")
                ),
                values,
            )
        }
    };
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}
