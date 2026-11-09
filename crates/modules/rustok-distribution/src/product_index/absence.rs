use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    EntityKey, EntityName, IndexSourceAbsenceProvider, IndexSourceAbsenceWatermark,
    IndexSourceFailure, ModuleName, PostgresIndexSourceFactory, SchemaRef, SchemaVersion,
    register_index_source_absence_provider, register_postgres_index_source_factory,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde_json::Value as JsonValue;

use rustok_product::index_channel_visibility::decode_product_visibility;

use super::{PRODUCT_SCHEMA_ROUTING_KEY, with_channel_identity_source};

static PRODUCT_LOCALE_ABSENCE_SQL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    with_channel_identity_source(rustok_product::index_projection::PRODUCT_LOCALE_ABSENCE_SQL)
});

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
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                PRODUCT_LOCALE_ABSENCE_SQL.as_str(),
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
    match IndexSourceFailure::retryable(code) {
        Ok(failure) => failure,
        Err(_) => match IndexSourceFailure::retryable("internal_error") {
            Ok(fallback) => fallback,
            Err(_) => unreachable!("internal_error is a valid machine name"),
        },
    }
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    match IndexSourceFailure::permanent(code) {
        Ok(failure) => failure,
        Err(_) => match IndexSourceFailure::permanent("internal_error") {
            Ok(fallback) => fallback,
            Err(_) => unreachable!("internal_error is a valid machine name"),
        },
    }
}
