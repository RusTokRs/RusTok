use rustok_events::DomainEvent;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
};
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};

pub const MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE: usize = 256;
pub const MAX_PRODUCT_INDEX_VARIANT_REFRESH_PAGE: usize = 256;
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
        validate_refresh_page(
            tenant_id,
            after_sequence_no,
            limit,
            MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE,
            "Product Index locale refresh",
        )?;
        ensure_postgres(
            &self.db,
            "Product Index locale refresh source requires PostgreSQL",
        )?;

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
                let source_version = positive_source_version(
                    row.try_get("", "source_version")?,
                    "Product Index locale refresh",
                )?;

                if sequence_no <= 0
                    || refresh_id.is_nil()
                    || root_event_id.is_nil()
                    || row_tenant_id != tenant_id
                    || product_id.is_nil()
                    || locale.trim().is_empty()
                    || locale.len() > 128
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

/// One immutable Product-owned instruction to refresh an exact ProductVariant Index identity.
///
/// The record deliberately includes the parent Product identity even though the ProductVariant
/// schema key is the variant UUID. The parent is required to enumerate retained deletes after the
/// live variant row has disappeared and to preserve Product graph causation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductIndexVariantRefreshRecord {
    sequence_no: i64,
    refresh_id: Uuid,
    root_event_id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    source_version: u64,
}

impl ProductIndexVariantRefreshRecord {
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

    pub fn variant_id(&self) -> Uuid {
        self.variant_id
    }

    pub fn source_version(&self) -> u64 {
        self.source_version
    }
}

/// Bounded Product-owned ProductVariant refresh reader.
///
/// Writers remain set-based and product-scoped so existing products are not assigned a new variant
/// count limit. Relays consume the append-only ledger through tenant-local pages of at most 256 rows.
#[derive(Clone)]
pub struct ProductIndexVariantRefreshSource {
    db: DatabaseConnection,
}

impl ProductIndexVariantRefreshSource {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        after_sequence_no: i64,
        limit: usize,
    ) -> CommerceResult<Vec<ProductIndexVariantRefreshRecord>> {
        validate_refresh_page(
            tenant_id,
            after_sequence_no,
            limit,
            MAX_PRODUCT_INDEX_VARIANT_REFRESH_PAGE,
            "ProductVariant Index refresh",
        )?;
        ensure_postgres(
            &self.db,
            "ProductVariant Index refresh source requires PostgreSQL",
        )?;

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
                    variant_id,
                    source_version
                FROM product_variant_index_refresh_ledger
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
                let variant_id: Uuid = row.try_get("", "variant_id")?;
                let source_version = positive_source_version(
                    row.try_get("", "source_version")?,
                    "ProductVariant Index refresh",
                )?;

                if sequence_no <= 0
                    || refresh_id.is_nil()
                    || root_event_id.is_nil()
                    || row_tenant_id != tenant_id
                    || product_id.is_nil()
                    || variant_id.is_nil()
                {
                    return Err(CommerceError::Validation(
                        "ProductVariant Index refresh source returned an invalid ledger row"
                            .to_owned(),
                    ));
                }

                Ok(ProductIndexVariantRefreshRecord {
                    sequence_no,
                    refresh_id,
                    root_event_id,
                    tenant_id: row_tenant_id,
                    product_id,
                    variant_id,
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
    validate_refresh_identity(
        tenant_id,
        product_id,
        root_event_id,
        "Product Index locale refresh",
    )?;

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

/// Captures every live or retained ProductVariant identity for one Product after the owner command.
///
/// The statement is set-based and product-scoped. Live rows use the final trigger-owned variant
/// revision. Deleted rows use the retained tombstone revision only when the tombstone has a known
/// parent Product. Historical parentless tombstones remain replayable but are deliberately excluded
/// from incremental publication because their Product causation cannot be reconstructed safely.
pub(crate) async fn record_product_variant_refreshes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
) -> CommerceResult<()> {
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(());
    }
    validate_refresh_identity(
        tenant_id,
        product_id,
        root_event_id,
        "ProductVariant Index refresh",
    )?;

    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        WITH live_variants AS (
            SELECT
                variant.id AS variant_id,
                variant.index_revision AS source_version
            FROM product_variants variant
            WHERE variant.tenant_id = $1
              AND variant.product_id = $2
        ),
        retained_variants AS (
            SELECT
                tombstone.variant_id AS variant_id,
                tombstone.source_version AS source_version
            FROM product_variant_index_tombstones tombstone
            WHERE tombstone.tenant_id = $1
              AND tombstone.product_id = $2
              AND NOT EXISTS (
                  SELECT 1
                  FROM product_variants variant
                  WHERE variant.tenant_id = tombstone.tenant_id
                    AND variant.id = tombstone.variant_id
              )
        ),
        exact_targets AS (
            SELECT variant_id, source_version FROM live_variants
            UNION ALL
            SELECT variant_id, source_version FROM retained_variants
        ),
        identified_targets AS (
            SELECT
                variant_id,
                source_version,
                md5(
                    $1::text || ':' ||
                    $2::text || ':' ||
                    $3::text || ':' ||
                    variant_id::text
                ) AS identity_digest
            FROM exact_targets
        )
        INSERT INTO product_variant_index_refresh_ledger (
            refresh_id,
            root_event_id,
            tenant_id,
            product_id,
            variant_id,
            source_version,
            created_at
        )
        SELECT
            (
                substr(identity_digest, 1, 8) || '-' ||
                substr(identity_digest, 9, 4) || '-' ||
                substr(identity_digest, 13, 4) || '-' ||
                substr(identity_digest, 17, 4) || '-' ||
                substr(identity_digest, 21, 12)
            )::uuid,
            $3,
            $1,
            $2,
            variant_id,
            source_version,
            CURRENT_TIMESTAMP
        FROM identified_targets
        ORDER BY variant_id ASC
        "#,
        vec![tenant_id.into(), product_id.into(), root_event_id.into()],
    ))
    .await?;

    Ok(())
}

fn validate_refresh_page(
    tenant_id: Uuid,
    after_sequence_no: i64,
    limit: usize,
    maximum: usize,
    boundary: &str,
) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(format!(
            "{boundary} tenant must not be nil"
        )));
    }
    if after_sequence_no < 0 {
        return Err(CommerceError::Validation(format!(
            "{boundary} cursor must not be negative"
        )));
    }
    if !(1..=maximum).contains(&limit) {
        return Err(CommerceError::Validation(format!(
            "{boundary} page size must be between 1 and {maximum}"
        )));
    }
    Ok(())
}

fn validate_refresh_identity(
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
    boundary: &str,
) -> CommerceResult<()> {
    if tenant_id.is_nil() || product_id.is_nil() || root_event_id.is_nil() {
        return Err(CommerceError::Validation(format!(
            "{boundary} identity must not be nil"
        )));
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection, message: &str) -> CommerceResult<()> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(CommerceError::Validation(message.to_owned()));
    }
    Ok(())
}

fn positive_source_version(value: i64, boundary: &str) -> CommerceResult<u64> {
    let value = u64::try_from(value).map_err(|_| {
        CommerceError::Validation(format!("{boundary} source version must be positive"))
    })?;
    if value == 0 {
        return Err(CommerceError::Validation(format!(
            "{boundary} source version must be positive"
        )));
    }
    Ok(value)
}
