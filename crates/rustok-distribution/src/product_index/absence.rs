use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    EntityKey, EntityName, IndexSourceAbsenceProvider, IndexSourceAbsenceWatermark,
    IndexSourceFailure, ModuleName, PostgresIndexSourceFactory, SchemaRef, SchemaVersion,
    register_index_source_absence_provider, register_postgres_index_source_factory,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

pub(crate) const PRODUCT_ABSENCE_WATERMARK_FACTORY: &str =
    "product-locale-absence-watermark";
const PRODUCT_ABSENCE_PROVIDER: &str = "product-locale-absence-postgres";

const PRODUCT_V1_V2_ABSENCE_SQL: &str = r#"
SELECT CAST(product.index_revision AS TEXT) AS source_version_text
FROM products product
WHERE product.tenant_id = $1
  AND product.id = $2
  AND NOT EXISTS (
      SELECT 1
      FROM product_translations translation
      WHERE translation.tenant_id = product.tenant_id
        AND translation.product_id = product.id
        AND translation.locale = $3
  )
  AND NOT EXISTS (
      SELECT 1
      FROM product_index_tombstones tombstone
      WHERE tombstone.tenant_id = product.tenant_id
        AND tombstone.product_id = product.id
        AND tombstone.locale = $3
  )
LIMIT 1
"#;

const PRODUCT_V3_ABSENCE_SQL: &str = r#"
SELECT CAST(projection.projection_epoch AS TEXT) AS source_version_text
FROM products product
JOIN LATERAL (
    SELECT
        snapshot.projection_epoch,
        snapshot.product_source_version,
        snapshot.relation_epoch
    FROM product_index_graph_v3_projection_snapshots snapshot
    WHERE snapshot.tenant_id = product.tenant_id
      AND snapshot.product_id = product.id
    ORDER BY snapshot.projection_epoch DESC
    LIMIT 1
) projection ON TRUE
JOIN LATERAL (
    SELECT relation.relation_epoch
    FROM product_sales_channel_index_relation_snapshots relation
    WHERE relation.tenant_id = product.tenant_id
      AND relation.product_id = product.id
    ORDER BY relation.relation_epoch DESC
    LIMIT 1
) relation ON TRUE
WHERE product.tenant_id = $1
  AND product.id = $2
  AND projection.product_source_version = product.index_revision
  AND projection.relation_epoch = relation.relation_epoch
  AND NOT EXISTS (
      SELECT 1
      FROM product_translations translation
      WHERE translation.tenant_id = product.tenant_id
        AND translation.product_id = product.id
        AND translation.locale = $3
  )
  AND NOT EXISTS (
      SELECT 1
      FROM product_index_tombstones tombstone
      WHERE tombstone.tenant_id = product.tenant_id
        AND tombstone.product_id = product.id
        AND tombstone.locale = $3
  )
LIMIT 1
"#;

pub(crate) fn register(
    extensions: &mut ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    register_postgres_index_source_factory(
        extensions,
        "product",
        PRODUCT_ABSENCE_WATERMARK_FACTORY,
        ProductLocaleAbsencePostgresFactory,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index absence factory registration failed: {error}"
        ))
    })
}

#[derive(Clone, Copy, Debug)]
struct ProductLocaleAbsencePostgresFactory;

impl PostgresIndexSourceFactory for ProductLocaleAbsencePostgresFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        register_index_source_absence_provider(
            extensions,
            "product",
            PRODUCT_ABSENCE_PROVIDER,
            [
                product_schema_ref(1)?,
                product_schema_ref(2)?,
                product_schema_ref(3)?,
            ],
            ProductLocaleAbsenceProvider { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
struct ProductLocaleAbsenceProvider {
    db: DatabaseConnection,
}

#[async_trait]
impl IndexSourceAbsenceProvider for ProductLocaleAbsenceProvider {
    async fn load_absence_watermark(
        &self,
        key: EntityKey,
    ) -> Result<Option<IndexSourceAbsenceWatermark>, IndexSourceFailure> {
        let version = require_product_key(&self.db, &key)?;
        let locale = key
            .locale
            .as_ref()
            .ok_or_else(|| permanent("product_index_absence_locale_required"))?;
        let sql = if version == 3 {
            PRODUCT_V3_ABSENCE_SQL
        } else {
            PRODUCT_V1_V2_ABSENCE_SQL
        };

        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                vec![
                    key.tenant_id.into(),
                    key.entity_id.into(),
                    locale.as_str().to_owned().into(),
                ],
            ))
            .await
            .map_err(|_| retryable("product_index_absence_storage_unavailable"))?;

        let Some(row) = row else {
            return Ok(None);
        };
        let source_version = row
            .try_get::<String>("", "source_version_text")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| permanent("product_index_absence_version_invalid"))?;
        IndexSourceAbsenceWatermark::new(key, source_version)
            .map(Some)
            .map_err(|_| permanent("product_index_absence_contract_invalid"))
    }
}

fn require_product_key(
    db: &DatabaseConnection,
    key: &EntityKey,
) -> Result<u32, IndexSourceFailure> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(permanent("product_index_absence_backend_unsupported"));
    }
    let version = key.schema.version.get();
    let valid_schema = matches!(version, 1 | 2 | 3)
        && product_schema_ref(version).is_ok_and(|schema| schema == key.schema);
    if !valid_schema {
        return Err(permanent("product_index_absence_schema_mismatch"));
    }
    if key.locale.is_none() {
        return Err(permanent("product_index_absence_locale_required"));
    }
    Ok(version)
}

fn product_schema_ref(version: u32) -> Result<SchemaRef, String> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product").map_err(|error| error.to_string())?,
        entity: EntityName::new("product").map_err(|error| error.to_string())?,
        version: SchemaVersion::new(version),
    })
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static Product absence retry code is valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static Product absence failure code is valid")
}

#[cfg(test)]
mod tests {
    use super::{PRODUCT_V1_V2_ABSENCE_SQL, PRODUCT_V3_ABSENCE_SQL};

    #[test]
    fn product_v3_absence_uses_projection_epoch_and_exact_current_inputs() {
        assert!(PRODUCT_V3_ABSENCE_SQL.contains("product_index_graph_v3_projection_snapshots"));
        assert!(PRODUCT_V3_ABSENCE_SQL.contains("product_sales_channel_index_relation_snapshots"));
        assert!(PRODUCT_V3_ABSENCE_SQL.contains("projection.product_source_version = product.index_revision"));
        assert!(PRODUCT_V3_ABSENCE_SQL.contains("projection.relation_epoch = relation.relation_epoch"));
        assert!(PRODUCT_V3_ABSENCE_SQL.contains("projection.projection_epoch"));
        assert!(!PRODUCT_V1_V2_ABSENCE_SQL.contains("product_index_graph_v3_projection_snapshots"));
    }
}
