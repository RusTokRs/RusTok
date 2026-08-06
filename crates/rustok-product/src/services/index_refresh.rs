use rustok_events::DomainEvent;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
};
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};

pub const MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE: usize = 256;
const MAX_PRODUCT_INDEX_LOCALE_TARGETS_PER_EVENT: usize = 256;

/// One immutable Product-owned instruction to refresh an exact localized Index identity.
///
/// `refresh_id` is reserved as the future typed event and inbox identity. `root_event_id`
/// preserves causation to the existing Product lifecycle envelope. The payload is intentionally
/// thin: the Product PostgreSQL source remains authoritative for the actual upsert or tombstone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductIndexLocaleRefreshRecord {
    sequence_no: i64,
    refresh_id: Uuid,
    root_event_id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: String,
    source_version: u64,
}

impl ProductIndexLocaleRefreshRecord {
    pub fn sequence_no(&self) -> i64 {
        self.sequence_no
    }

    pub fn refresh_id(&self) -> Uuid {
        self.refresh_id
    }

    pub fn root_event_id(&self) -> Uuid {
        self.root_event_id
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn product_id(&self) -> Uuid {
        self.product_id
    }

    pub fn locale(&self) -> &str {
        self.locale.as_str()
    }

    pub fn source_version(&self) -> u64 {
        self.source_version
    }
}

/// Bounded Product-owned reader used by a future relay/consumer composition.
///
/// The reader exposes no Product payload and performs no Index write. Callers page one tenant by
/// the append-only sequence and later resolve canonical state through the registered Product source.
#[derive(Clone)]
pub struct ProductIndexLocaleRefreshSource {
    db: DatabaseConnection,
}

impl ProductIndexLocaleRefreshSource {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        after_sequence_no: i64,
        limit: usize,
    ) -> CommerceResult<Vec<ProductIndexLocaleRefreshRecord>> {
        if tenant_id.is_nil() {
            return Err(CommerceError::Validation(
                "Product Index refresh tenant must not be nil".to_owned(),
            ));
        }
        if after_sequence_no < 0 {
            return Err(CommerceError::Validation(
                "Product Index refresh cursor must not be negative".to_owned(),
            ));
        }
        if !(1..=MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE).contains(&limit) {
            return Err(CommerceError::Validation(format!(
                "Product Index refresh page size must be between 1 and {MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE}"
            )));
        }
        if self.db.get_database_backend() != DatabaseBackend::Postgres {
            return Err(CommerceError::Validation(
                "Product Index locale refresh source requires PostgreSQL".to_owned(),
            ));
        }

        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    sequence_no,
                    refresh_id,
                    root_event_id,
                    tenant_id,
                    product_id,
                    locale,
                    source_version
                FROM product_index_locale_refresh_ledger
                WHERE tenant_id = $1
                  AND sequence_no > $2
                ORDER BY sequence_no ASC
                LIMIT $3
                "#,
                vec![
                    tenant_id.into(),
                    after_sequence_no.into(),
                    (limit as i64).into(),
                ],
            ))
            .await?;

        rows.into_iter()
            .map(|row| {
                let sequence_no: i64 = row.try_get("", "sequence_no")?;
                let refresh_id: Uuid = row.try_get("", "refresh_id")?;
                let root_event_id: Uuid = row.try_get("", "root_event_id")?;
                let row_tenant_id: Uuid = row.try_get("", "tenant_id")?;
                let product_id: Uuid = row.try_get("", "product_id")?;
                let locale: String = row.try_get("", "locale")?;
                let source_version: i64 = row.try_get("", "source_version")?;
                let source_version = u64::try_from(source_version).map_err(|_| {
                    CommerceError::Validation(
                        "Product Index refresh source version must be positive".to_owned(),
                    )
                })?;

                if sequence_no <= 0
                    || refresh_id.is_nil()
                    || root_event_id.is_nil()
                    || row_tenant_id != tenant_id
                    || product_id.is_nil()
                    || locale.trim().is_empty()
                    || locale.len() > 128
                    || source_version == 0
                {
                    return Err(CommerceError::Validation(
                        "Product Index refresh source returned an invalid ledger row".to_owned(),
                    ));
                }

                Ok(ProductIndexLocaleRefreshRecord {
                    sequence_no,
                    refresh_id,
                    root_event_id,
                    tenant_id: row_tenant_id,
                    product_id,
                    locale,
                    source_version,
                })
            })
            .collect()
    }
}

pub(crate) fn product_locale_refresh_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::ProductCreated { product_id }
        | DomainEvent::ProductUpdated { product_id }
        | DomainEvent::ProductPublished { product_id }
        | DomainEvent::ProductDeleted { product_id } => Some(*product_id),
        _ => None,
    }
}

/// Captures the canonical live and retained-delete state after one Product owner command.
///
/// Live locales use the final trigger-owned `products.index_revision`. Deleted locales use their
/// exact retained tombstone revision. Historical tombstones may be emitted again by a later Product
/// event; that is safe because the eventual Index worker applies source-version monotonicity while
/// the per-row `refresh_id` preserves delivery deduplication.
pub(crate) async fn record_product_locale_refreshes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
) -> CommerceResult<()> {
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(());
    }
    if tenant_id.is_nil() || product_id.is_nil() || root_event_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product Index locale refresh identity must not be nil".to_owned(),
        ));
    }

    let rows = txn
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            WITH live_locales AS (
                SELECT
                    translation.locale AS locale,
                    product.index_revision AS source_version
                FROM product_translations translation
                JOIN products product
                  ON product.tenant_id = translation.tenant_id
                 AND product.id = translation.product_id
                WHERE translation.tenant_id = $1
                  AND translation.product_id = $2
            ),
            retained_locales AS (
                SELECT
                    tombstone.locale AS locale,
                    tombstone.source_version AS source_version
                FROM product_index_tombstones tombstone
                WHERE tombstone.tenant_id = $1
                  AND tombstone.product_id = $2
                  AND NOT EXISTS (
                      SELECT 1
                      FROM product_translations translation
                      WHERE translation.tenant_id = tombstone.tenant_id
                        AND translation.product_id = tombstone.product_id
                        AND translation.locale = tombstone.locale
                  )
            ),
            exact_targets AS (
                SELECT locale, source_version FROM live_locales
                UNION ALL
                SELECT locale, source_version FROM retained_locales
            )
            SELECT locale, source_version
            FROM exact_targets
            ORDER BY locale ASC
            LIMIT $3
            "#,
            vec![
                tenant_id.into(),
                product_id.into(),
                ((MAX_PRODUCT_INDEX_LOCALE_TARGETS_PER_EVENT + 1) as i64).into(),
            ],
        ))
        .await?;

    if rows.len() > MAX_PRODUCT_INDEX_LOCALE_TARGETS_PER_EVENT {
        return Err(CommerceError::Validation(format!(
            "Product Index locale refresh target count exceeds {MAX_PRODUCT_INDEX_LOCALE_TARGETS_PER_EVENT}"
        )));
    }

    for row in rows {
        let locale: String = row.try_get("", "locale")?;
        let source_version: i64 = row.try_get("", "source_version")?;
        if locale.trim().is_empty() || locale.len() > 128 || source_version <= 0 {
            return Err(CommerceError::Validation(
                "Product Index locale refresh source returned an invalid target".to_owned(),
            ));
        }

        txn.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO product_index_locale_refresh_ledger (
                refresh_id,
                root_event_id,
                tenant_id,
                product_id,
                locale,
                source_version,
                created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
            "#,
            vec![
                Uuid::new_v4().into(),
                root_event_id.into(),
                tenant_id.into(),
                product_id.into(),
                locale.into(),
                source_version.into(),
            ],
        ))
        .await?;
    }

    Ok(())
}
