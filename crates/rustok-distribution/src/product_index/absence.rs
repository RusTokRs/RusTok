use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    EntityKey, EntityName, IndexSourceAbsenceProvider, IndexSourceAbsenceWatermark,
    IndexSourceFailure, ModuleName, PostgresIndexSourceFactory, SchemaRef, SchemaVersion,
    register_index_source_absence_provider, register_postgres_index_source_factory,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde_json::Value as JsonValue;

use super::{PRODUCT_SCHEMA_ROUTING_KEY, channel_visibility::decode_product_visibility};

pub(crate) const PRODUCT_ABSENCE_WATERMARK_FACTORY: &str = "product-locale-absence-watermark";
const PRODUCT_ABSENCE_PROVIDER: &str = "product-locale-absence-postgres";

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
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
            [product_schema_ref()?],
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
        require_product_key(&self.db, &key)?;
        let locale = key
            .locale
            .as_ref()
            .ok_or_else(|| permanent("product_index_absence_locale_required"))?;

        // Absence is authoritative only when the same projection epoch is backed by a current
        // Product-SalesChannel freshness witness. Missing or stale freshness returns no watermark.
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT
    CAST(projection.projection_epoch AS TEXT) AS source_version_text,
    product.index_revision AS observed_product_source_version,
    product.metadata,
    freshness.product_source_version AS freshness_product_source_version,
    freshness.visibility_key AS freshness_visibility_key,
    freshness.channel_identity_generation AS freshness_channel_identity_generation,
    COALESCE(
        (
            SELECT generation
            FROM channel_index_identity_generations
            WHERE tenant_id = product.tenant_id
        ),
        0
    )::bigint AS current_channel_identity_generation
FROM products product
JOIN LATERAL (
    SELECT
        projection.projection_epoch,
        projection.product_source_version,
        projection.relation_epoch
    FROM product_index_graph_projection_snapshots projection
    WHERE projection.tenant_id = product.tenant_id
      AND projection.product_id = product.id
    ORDER BY projection.projection_epoch DESC
    LIMIT 1
) projection
  ON projection.product_source_version = product.index_revision
JOIN product_sales_channel_index_relation_snapshots relation
  ON relation.tenant_id = product.tenant_id
 AND relation.product_id = product.id
 AND relation.relation_epoch = projection.relation_epoch
JOIN LATERAL (
    SELECT
        witness.product_source_version,
        witness.visibility_key,
        witness.channel_identity_generation
    FROM product_sales_channel_index_relation_freshness_snapshots witness
    WHERE witness.tenant_id = product.tenant_id
      AND witness.product_id = product.id
      AND witness.relation_epoch = projection.relation_epoch
    ORDER BY witness.sequence_no DESC
    LIMIT 1
) freshness ON TRUE
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
"#,
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
        let observed_product_source_version = positive_u64(
            row.try_get::<i64>("", "observed_product_source_version")
                .map_err(|_| permanent("product_index_absence_freshness_invalid"))?,
        )?;
        let freshness_product_source_version = positive_u64(
            row.try_get::<i64>("", "freshness_product_source_version")
                .map_err(|_| permanent("product_index_absence_freshness_invalid"))?,
        )?;
        if freshness_product_source_version > observed_product_source_version {
            return Err(permanent("product_index_absence_freshness_invalid"));
        }
        let metadata = row
            .try_get::<JsonValue>("", "metadata")
            .map_err(|_| permanent("product_index_absence_visibility_invalid"))?;
        let current_visibility_key = decode_product_visibility(&metadata)
            .map_err(|_| permanent("product_index_absence_visibility_invalid"))?
            .freshness_key();
        let freshness_visibility_key = row
            .try_get::<String>("", "freshness_visibility_key")
            .map_err(|_| permanent("product_index_absence_freshness_invalid"))?;
        let freshness_channel_identity_generation = non_negative_u64(
            row.try_get::<i64>("", "freshness_channel_identity_generation")
                .map_err(|_| permanent("product_index_absence_freshness_invalid"))?,
        )?;
        let current_channel_identity_generation = non_negative_u64(
            row.try_get::<i64>("", "current_channel_identity_generation")
                .map_err(|_| permanent("product_index_absence_freshness_invalid"))?,
        )?;

        if freshness_visibility_key != current_visibility_key
            || freshness_channel_identity_generation != current_channel_identity_generation
        {
            return Ok(None);
        }

        IndexSourceAbsenceWatermark::new(key, source_version)
            .map(Some)
            .map_err(|_| permanent("product_index_absence_contract_invalid"))
    }
}

fn require_product_key(db: &DatabaseConnection, key: &EntityKey) -> Result<(), IndexSourceFailure> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(permanent("product_index_absence_backend_unsupported"));
    }
    match product_schema_ref() {
        Ok(schema) if schema == key.schema => {}
        _ => return Err(permanent("product_index_absence_schema_mismatch")),
    }
    if key.locale.is_none() {
        return Err(permanent("product_index_absence_locale_required"));
    }
    Ok(())
}

fn product_schema_ref() -> Result<SchemaRef, String> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product").map_err(|error| error.to_string())?,
        entity: EntityName::new("product").map_err(|error| error.to_string())?,
        version: SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY),
    })
}

fn positive_u64(value: i64) -> Result<u64, IndexSourceFailure> {
    if value <= 0 {
        return Err(permanent("product_index_absence_freshness_invalid"));
    }
    u64::try_from(value).map_err(|_| permanent("product_index_absence_freshness_invalid"))
}

fn non_negative_u64(value: i64) -> Result<u64, IndexSourceFailure> {
    if value < 0 {
        return Err(permanent("product_index_absence_freshness_invalid"));
    }
    u64::try_from(value).map_err(|_| permanent("product_index_absence_freshness_invalid"))
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static Product absence retry code is valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static Product absence failure code is valid")
}
