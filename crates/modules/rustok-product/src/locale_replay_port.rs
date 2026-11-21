use std::{collections::BTreeSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortContext, PortError, normalize_locale_tag};
use rustok_channel_ports::{ChannelIndexIdentityReadPort, ChannelIndexIdentitySelector};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::index_channel_visibility::decode_product_visibility;
use crate::index_projection::{PRODUCT_ATTRIBUTE_TERMS_CTE, PRODUCT_ROW_SELECT, PRODUCT_ROWS_CTE};
use crate::{
    PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING, PRODUCT_LOCALE_REPLAY_READ_CLAIM,
    ProductLocaleReplayEntry, ProductLocaleReplayFields, ProductLocaleReplayKey,
    ProductLocaleReplayPage, ProductLocaleReplayPort, ProductLocaleReplayScan,
    ProductLocaleReplayState,
};

#[derive(Clone)]
pub struct PostgresProductLocaleReplay {
    db: DatabaseConnection,
    rows_cte: String,
    channel: Option<Arc<dyn ChannelIndexIdentityReadPort>>,
}

impl PostgresProductLocaleReplay {
    /// Uses the Channel-owned relation published for the trusted shared PostgreSQL deployment.
    pub fn embedded(db: DatabaseConnection, channel_relation: &'static str) -> Self {
        let generation = format!(
            "COALESCE((SELECT generation FROM {channel_relation} generations WHERE tenant_id = $1), 0)::bigint"
        );
        Self {
            db,
            rows_cte: PRODUCT_ROWS_CTE.replace("__channel_identity_generation__", &generation),
            channel: None,
        }
    }

    pub fn separate(
        db: DatabaseConnection,
        channel: Arc<dyn ChannelIndexIdentityReadPort>,
    ) -> Self {
        Self {
            db,
            rows_cte: PRODUCT_ROWS_CTE.replace("__channel_identity_generation__", "NULL::bigint"),
            channel: Some(channel),
        }
    }

    pub async fn verify_readable_schema(&self) -> Result<(), sea_orm::DbErr> {
        let sql = format!(
            "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{}\n{PRODUCT_ROW_SELECT}\nLIMIT 0",
            self.rows_cte
        );
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                [Uuid::nil().into()],
            ))
            .await?;
        Ok(())
    }

    fn authorize(&self, context: &PortContext) -> Result<Uuid, PortError> {
        context.require_read_semantics()?;
        if context.actor.kind != PortActorKind::Service
            || !context
                .claims
                .iter()
                .any(|claim| claim == PRODUCT_LOCALE_REPLAY_READ_CLAIM)
        {
            return Err(PortError::forbidden(
                "product.locale_replay_forbidden",
                "Product locale replay requires a service grant",
            ));
        }
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(invalid_row());
        }
        context
            .tenant_id
            .parse::<Uuid>()
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(invalid_request)
    }

    async fn decode_rows(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        rows: Vec<QueryResult>,
    ) -> Result<Vec<ProductLocaleReplayEntry>, PortError> {
        let mut has_live_row = false;
        for row in &rows {
            has_live_row |= !row
                .try_get::<bool>("", "is_deleted")
                .map_err(|_| invalid_row())?;
        }
        // The Channel generation is checked after the Product statement completes.
        // Tombstones only remove data and can replay while Channel is unavailable.
        let generation = match &self.channel {
            Some(channel) if has_live_row => {
                let mut channel_context = context;
                channel_context.deadline_ms =
                    Some(channel_context.deadline_ms.unwrap_or_default().min(30_000));
                channel_context
                    .claims
                    .push(rustok_channel_ports::CHANNEL_INDEX_IDENTITY_READ_CLAIM.to_owned());
                let snapshot = channel.read_index_identity(channel_context, ChannelIndexIdentitySelector::GenerationOnly).await
                    .map_err(|error| {
                        tracing::warn!(%tenant_id, %error, "Product locale replay could not read Channel generation");
                        storage_unavailable()
                    })?;
                // INVARIANT: Channel identity snapshot tenant must match port caller tenant.
                if snapshot.tenant_id != tenant_id || !snapshot.channel_ids.is_empty() {
                    return Err(invalid_row());
                }
                Some(snapshot.generation)
            }
            _ => None,
        };
        rows.into_iter()
            .map(|row| decode(row, tenant_id, generation))
            .collect()
    }

    async fn scan_rows(
        &self,
        tenant_id: Uuid,
        request: &ProductLocaleReplayScan,
    ) -> Result<Vec<QueryResult>, PortError> {
        let fetch_limit = i64::try_from(request.limit + 1).map_err(|_| invalid_request())?;
        let rows_cte = &self.rows_cte;
        let (sql, values): (String, Vec<Value>) = match (
            request.locale.as_deref(),
            request.after.as_ref(),
        ) {
            (Some(locale), Some(cursor)) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nWHERE row.locale = $2 AND row.product_id > $3\nORDER BY row.product_id ASC\nLIMIT $4"
                ),
                vec![
                    tenant_id.into(),
                    locale.to_owned().into(),
                    cursor.product_id.into(),
                    fetch_limit.into(),
                ],
            ),
            (Some(locale), None) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nWHERE row.locale = $2\nORDER BY row.product_id ASC\nLIMIT $3"
                ),
                vec![
                    tenant_id.into(),
                    locale.to_owned().into(),
                    fetch_limit.into(),
                ],
            ),
            (None, Some(cursor)) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nWHERE (row.product_id, row.locale) > ($2, $3)\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $4"
                ),
                vec![
                    tenant_id.into(),
                    cursor.product_id.into(),
                    cursor.locale.clone().into(),
                    fetch_limit.into(),
                ],
            ),
            (None, None) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $2"
                ),
                vec![tenant_id.into(), fetch_limit.into()],
            ),
        };
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| storage_unavailable())
    }

    async fn load_rows(
        &self,
        tenant_id: Uuid,
        keys: &[ProductLocaleReplayKey],
    ) -> Result<Vec<QueryResult>, PortError> {
        let mut values = Vec::<Value>::with_capacity(1 + keys.len() * 2);
        values.push(tenant_id.into());
        let mut tuples = Vec::with_capacity(keys.len());
        let mut parameter = 2usize;
        for key in keys {
            tuples.push(format!("(${parameter}::uuid, ${}::text)", parameter + 1));
            values.push(key.product_id.into());
            values.push(key.locale.clone().into());
            parameter += 2;
        }
        let rows_cte = &self.rows_cte;
        let sql = format!(
            "WITH requested(product_id, locale) AS (VALUES {}),\n{PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nJOIN requested requested_key ON requested_key.product_id = row.product_id AND requested_key.locale = row.locale\nORDER BY row.product_id ASC, row.locale ASC",
            tuples.join(", ")
        );
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| storage_unavailable())
    }
}

#[async_trait]
impl ProductLocaleReplayPort for PostgresProductLocaleReplay {
    async fn scan_locale_replay(
        &self,
        context: PortContext,
        request: ProductLocaleReplayScan,
    ) -> Result<ProductLocaleReplayPage, PortError> {
        let tenant_id = self.authorize(&context)?;
        if !(1..=1_000).contains(&request.limit)
            || request
                .locale
                .as_deref()
                .is_some_and(|locale| !canonical_locale(locale))
            || request.after.as_ref().is_some_and(|after| {
                !valid_key(after)
                    || request
                        .locale
                        .as_ref()
                        .is_some_and(|locale| *locale != after.locale)
            })
        {
            return Err(invalid_request());
        }
        within_deadline(&context, async {
            let mut rows = self.scan_rows(tenant_id, &request).await?;
            let has_more = rows.len() > request.limit;
            rows.truncate(request.limit);
            let entries = self.decode_rows(context.clone(), tenant_id, rows).await?;
            let next = if has_more {
                entries.last().map(ProductLocaleReplayEntry::key)
            } else {
                None
            };
            Ok(ProductLocaleReplayPage { entries, next })
        })
        .await
    }

    async fn load_locale_replay(
        &self,
        context: PortContext,
        keys: Vec<ProductLocaleReplayKey>,
    ) -> Result<Vec<ProductLocaleReplayEntry>, PortError> {
        let tenant_id = self.authorize(&context)?;
        let mut seen = BTreeSet::new();
        if keys.is_empty()
            || keys.len() > 256
            || keys
                .iter()
                .any(|key| !valid_key(key) || !seen.insert((key.product_id, &key.locale)))
        {
            return Err(invalid_request());
        }
        within_deadline(&context, async {
            let rows = self.load_rows(tenant_id, &keys).await?;
            self.decode_rows(context.clone(), tenant_id, rows).await
        })
        .await
    }
}

fn canonical_locale(locale: &str) -> bool {
    locale.len() <= 32 && normalize_locale_tag(locale).as_deref() == Some(locale)
}

fn valid_key(key: &ProductLocaleReplayKey) -> bool {
    !key.product_id.is_nil() && canonical_locale(&key.locale)
}

async fn within_deadline<T>(
    context: &PortContext,
    read: impl Future<Output = Result<T, PortError>>,
) -> Result<T, PortError> {
    tokio::time::timeout(
        Duration::from_millis(context.deadline_ms.unwrap_or_default().min(30_000)),
        read,
    )
    .await
    .map_err(|_| {
        PortError::timeout(
            "product.locale_replay_timeout",
            "Product locale replay timed out",
        )
    })?
}

fn invalid_request() -> PortError {
    PortError::validation(
        "product.locale_replay_invalid_request",
        "Invalid Product locale replay request",
    )
}

fn invalid_row() -> PortError {
    PortError::invariant_violation(
        "product.locale_replay_invalid_row",
        "Invalid Product locale replay row",
    )
}

fn freshness_pending() -> PortError {
    PortError::unavailable(
        PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING,
        "Product Channel membership is awaiting reconciliation",
    )
}

fn storage_unavailable() -> PortError {
    PortError::unavailable(
        "product_index_storage_unavailable",
        "Unable to read the Product locale replay source",
    )
}

fn decode(
    row: QueryResult,
    expected_tenant: Uuid,
    remote_generation: Option<u64>,
) -> Result<ProductLocaleReplayEntry, PortError> {
    let is_deleted = row
        .try_get::<bool>("", "is_deleted")
        .map_err(|_| invalid_row())?;
    let identity_count = row
        .try_get::<i64>("", "identity_count")
        .map_err(|_| invalid_row())?;
    let tenant_id = row
        .try_get::<Uuid>("", "tenant_id")
        .map_err(|_| invalid_row())?;
    let product_id = row
        .try_get::<Uuid>("", "product_id")
        .map_err(|_| invalid_row())?;
    let source_version = optional_positive_u64(&row, "source_version")?.ok_or_else(|| {
        if is_deleted {
            invalid_row()
        } else {
            freshness_pending()
        }
    })?;
    let observed_product_source_version = positive_u64(&row, "observed_product_source_version")?;
    let projected_product_source_version = positive_u64(&row, "projected_product_source_version")?;
    let _relation_epoch = positive_u64(&row, "relation_epoch")?;
    let raw_locale = row
        .try_get::<String>("", "locale")
        .map_err(|_| invalid_row())?;
    if identity_count != 1
        || tenant_id != expected_tenant
        || tenant_id.is_nil()
        || product_id.is_nil()
    {
        return Err(invalid_row());
    }
    let locale = raw_locale;
    if !canonical_locale(&locale) {
        return Err(invalid_row());
    }

    // Tombstones keep their retained graph identity even without a live Channel witness.
    let sales_channel_ids = decode_uuid_json_list(&row, "sales_channel_ids")?;

    if is_deleted {
        if projected_product_source_version < observed_product_source_version {
            return Err(invalid_row());
        }
        return Ok(ProductLocaleReplayEntry {
            tenant_id,
            product_id,
            source_version,
            product_source_version: observed_product_source_version,
            locale,
            state: ProductLocaleReplayState::Deleted,
        });
    }

    if projected_product_source_version != observed_product_source_version {
        return Err(invalid_row());
    }

    let metadata = row
        .try_get::<JsonValue>("", "metadata")
        .map_err(|_| invalid_row())?;
    let current_visibility_key = decode_product_visibility(&metadata)
        .map_err(|_| invalid_row())?
        .freshness_key();
    let freshness_product_source_version =
        optional_positive_u64(&row, "freshness_product_source_version")?
            .ok_or_else(freshness_pending)?;
    let freshness_visibility_key = row
        .try_get::<Option<String>>("", "freshness_visibility_key")
        .map_err(|_| invalid_row())?
        .filter(|value| !value.is_empty())
        .ok_or_else(freshness_pending)?;
    let freshness_channel_identity_generation =
        optional_non_negative_u64(&row, "freshness_channel_identity_generation")?
            .ok_or_else(freshness_pending)?;
    let current_channel_identity_generation = match remote_generation {
        Some(generation) => generation,
        None => non_negative_u64(&row, "current_channel_identity_generation")?,
    };

    // Stale witness where freshness_channel_identity_generation != current_channel_identity_generation is rejected:
    if freshness_product_source_version > observed_product_source_version.get()
        || freshness_channel_identity_generation > current_channel_identity_generation
    {
        return Err(invalid_row());
    }
    if freshness_visibility_key != current_visibility_key
        || freshness_channel_identity_generation < current_channel_identity_generation
    {
        return Err(freshness_pending());
    }

    let primary_category_id = row
        .try_get::<Option<Uuid>>("", "primary_category_id")
        .map_err(|_| invalid_row())?;
    if primary_category_id.is_some_and(|id| id.is_nil()) {
        return Err(invalid_row());
    }
    Ok(ProductLocaleReplayEntry {
        tenant_id,
        product_id,
        source_version,
        product_source_version: observed_product_source_version,
        locale,
        state: ProductLocaleReplayState::Live(ProductLocaleReplayFields {
            status: required_string(&row, "status")?,
            title: required_string(&row, "title")?,
            handle: required_string(&row, "handle")?,
            description: optional_string(&row, "description")?,
            seller_id: optional_string(&row, "seller_id")?,
            vendor: optional_string(&row, "vendor")?,
            product_type: optional_string(&row, "product_type")?,
            primary_category_id,
            tag_ids: decode_uuid_json_list(&row, "tag_ids")?,
            created_at: required_timestamp(&row, "created_at")?,
            published_at: optional_timestamp(&row, "published_at")?,
            attribute_terms: decode_string_json_list(&row, "attribute_terms")?,
            variant_ids: decode_uuid_json_list(&row, "variant_ids")?,
            sales_channel_ids,
        }),
    })
}

fn decode_uuid_json_list(row: &QueryResult, column: &str) -> Result<Vec<Uuid>, PortError> {
    let value = row
        .try_get::<JsonValue>("", column)
        .map_err(|_| invalid_row())?;
    let values = value.as_array().ok_or(invalid_row())?;
    let mut unique = BTreeSet::new();
    for value in values {
        let raw = value.as_str().ok_or(invalid_row())?;
        let id = Uuid::parse_str(raw).map_err(|_| invalid_row())?;
        if id.is_nil() || !unique.insert(id) {
            return Err(invalid_row());
        }
    }
    Ok(unique.into_iter().collect())
}

fn decode_string_json_list(row: &QueryResult, column: &str) -> Result<Vec<String>, PortError> {
    let value = row
        .try_get::<JsonValue>("", column)
        .map_err(|_| invalid_row())?;
    let values = value.as_array().ok_or(invalid_row())?;
    let mut unique = BTreeSet::new();
    for value in values {
        let raw = value.as_str().ok_or(invalid_row())?;
        if raw.is_empty() || !unique.insert(raw.to_owned()) {
            return Err(invalid_row());
        }
    }
    Ok(unique.into_iter().collect())
}

fn positive_u64(row: &QueryResult, column: &str) -> Result<std::num::NonZeroU64, PortError> {
    let value = row.try_get::<i64>("", column).map_err(|_| invalid_row())?;
    u64::try_from(value)
        .ok()
        .and_then(std::num::NonZeroU64::new)
        .ok_or_else(invalid_row)
}

fn optional_positive_u64(row: &QueryResult, column: &str) -> Result<Option<u64>, PortError> {
    row.try_get::<Option<i64>>("", column)
        .map_err(|_| invalid_row())?
        .map(|value| {
            if value <= 0 {
                return Err(invalid_row());
            }
            u64::try_from(value).map_err(|_| invalid_row())
        })
        .transpose()
}

fn non_negative_u64(row: &QueryResult, column: &str) -> Result<u64, PortError> {
    let value = row.try_get::<i64>("", column).map_err(|_| invalid_row())?;
    if value < 0 {
        return Err(invalid_row());
    }
    u64::try_from(value).map_err(|_| invalid_row())
}

fn optional_non_negative_u64(row: &QueryResult, column: &str) -> Result<Option<u64>, PortError> {
    row.try_get::<Option<i64>>("", column)
        .map_err(|_| invalid_row())?
        .map(|value| {
            if value < 0 {
                return Err(invalid_row());
            }
            u64::try_from(value).map_err(|_| invalid_row())
        })
        .transpose()
}

fn required_string(row: &QueryResult, column: &str) -> Result<String, PortError> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| invalid_row())?;
    if value.is_empty() {
        Err(invalid_row())
    } else {
        Ok(value)
    }
}

fn optional_string(row: &QueryResult, column: &str) -> Result<Option<String>, PortError> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| invalid_row())
}

fn required_timestamp(row: &QueryResult, column: &str) -> Result<DateTime<Utc>, PortError> {
    row.try_get::<DateTime<Utc>>("", column)
        .map_err(|_| invalid_row())
}

fn optional_timestamp(row: &QueryResult, column: &str) -> Result<Option<DateTime<Utc>>, PortError> {
    row.try_get::<Option<DateTime<Utc>>>("", column)
        .map_err(|_| invalid_row())
}
