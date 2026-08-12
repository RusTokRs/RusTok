use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbBackend,
    Statement, TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

use super::{
    ProductIndexLocaleRefreshRecord, ProductIndexLocaleRefreshSource,
    ProductIndexRefreshCanonicalWriter, ProductIndexRefreshContract,
    ProductIndexRefreshPublicationError, ProductIndexVariantRefreshRecord,
    ProductIndexVariantRefreshSource,
};

const LOCALE_STREAM_KIND: &str = "locale";
const VARIANT_STREAM_KIND: &str = "variant";

/// Builds the sealed Product Index refresh family from immutable owner-ledger rows.
///
/// The associated events must already implement the Product-owned refresh contract.
/// No arbitrary JSON or unregistered event type can enter the canonical writer.
pub trait ProductIndexRefreshEventFactory: Send + Sync {
    type LocaleEvent: ProductIndexRefreshContract;
    type VariantEvent: ProductIndexRefreshContract;

    fn locale_event(&self, record: &ProductIndexLocaleRefreshRecord) -> Self::LocaleEvent;
    fn variant_event(&self, record: &ProductIndexVariantRefreshRecord) -> Self::VariantEvent;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductIndexRefreshRelayStepOutcome {
    /// No row existed after the locked durable cursor at this observation point.
    Idle { last_sequence_no: i64 },
    /// Another relay step advanced the cursor after this step read its candidate.
    CursorAdvanced { last_sequence_no: i64 },
    /// One exact ledger row and its cursor advancement committed atomically.
    Published { sequence_no: i64, refresh_id: Uuid },
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ProductIndexRefreshRelayError {
    #[error("Product Index refresh relay tenant identity is invalid")]
    InvalidTenant,
    #[error("Product Index refresh event does not match the immutable owner ledger row")]
    ContractMismatch,
    #[error("Product Index refresh ledger causation does not match a Product root envelope")]
    CausationMismatch,
    #[error("Product Index refresh envelope identity is already bound to different facts")]
    Conflict,
    #[error("Product Index refresh relay step is unavailable")]
    Unavailable,
}

/// One explicit, bounded Product refresh relay step.
///
/// The step never owns a background loop, lease, retry schedule, broker cursor or
/// acknowledgement. It reads at most one ledger row. The canonical outbox write
/// and monotonic tenant/stream cursor advancement commit in the same transaction.
pub struct ProductIndexRefreshRelayStep<F> {
    db: DatabaseConnection,
    factory: F,
}

impl<F> ProductIndexRefreshRelayStep<F>
where
    F: ProductIndexRefreshEventFactory,
{
    pub fn new(db: DatabaseConnection, factory: F) -> Self {
        Self { db, factory }
    }

    pub async fn publish_next_locale(
        &self,
        tenant_id: Uuid,
    ) -> Result<ProductIndexRefreshRelayStepOutcome, ProductIndexRefreshRelayError> {
        self.validate_database_and_tenant(tenant_id)?;
        let observed_cursor = load_cursor(&self.db, tenant_id, LOCALE_STREAM_KIND).await?;
        let record = ProductIndexLocaleRefreshSource::new(self.db.clone())
            .list(tenant_id, observed_cursor, 1)
            .await
            .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?
            .into_iter()
            .next();

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
        let locked_cursor = lock_cursor(&transaction, tenant_id, LOCALE_STREAM_KIND).await?;
        if locked_cursor != observed_cursor {
            transaction
                .rollback()
                .await
                .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
            return Ok(ProductIndexRefreshRelayStepOutcome::CursorAdvanced {
                last_sequence_no: locked_cursor,
            });
        }

        let Some(record) = record else {
            transaction
                .commit()
                .await
                .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
            return Ok(ProductIndexRefreshRelayStepOutcome::Idle {
                last_sequence_no: locked_cursor,
            });
        };

        let event = self.factory.locale_event(&record);
        let refresh_id =
            match ProductIndexRefreshCanonicalWriter::publish_locale_once_in_transaction(
                &transaction,
                &record,
                event,
            )
            .await
            {
                Ok(refresh_id) => refresh_id,
                Err(error) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
                    return Err(error.into());
                }
            };

        if let Err(error) = advance_cursor(
            &transaction,
            tenant_id,
            LOCALE_STREAM_KIND,
            locked_cursor,
            record.sequence_no(),
        )
        .await
        {
            transaction
                .rollback()
                .await
                .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
            return Err(error);
        }
        transaction
            .commit()
            .await
            .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;

        Ok(ProductIndexRefreshRelayStepOutcome::Published {
            sequence_no: record.sequence_no(),
            refresh_id,
        })
    }

    pub async fn publish_next_variant(
        &self,
        tenant_id: Uuid,
    ) -> Result<ProductIndexRefreshRelayStepOutcome, ProductIndexRefreshRelayError> {
        self.validate_database_and_tenant(tenant_id)?;
        let observed_cursor = load_cursor(&self.db, tenant_id, VARIANT_STREAM_KIND).await?;
        let record = ProductIndexVariantRefreshSource::new(self.db.clone())
            .list(tenant_id, observed_cursor, 1)
            .await
            .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?
            .into_iter()
            .next();

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
        let locked_cursor = lock_cursor(&transaction, tenant_id, VARIANT_STREAM_KIND).await?;
        if locked_cursor != observed_cursor {
            transaction
                .rollback()
                .await
                .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
            return Ok(ProductIndexRefreshRelayStepOutcome::CursorAdvanced {
                last_sequence_no: locked_cursor,
            });
        }

        let Some(record) = record else {
            transaction
                .commit()
                .await
                .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
            return Ok(ProductIndexRefreshRelayStepOutcome::Idle {
                last_sequence_no: locked_cursor,
            });
        };

        let event = self.factory.variant_event(&record);
        let refresh_id =
            match ProductIndexRefreshCanonicalWriter::publish_variant_once_in_transaction(
                &transaction,
                &record,
                event,
            )
            .await
            {
                Ok(refresh_id) => refresh_id,
                Err(error) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
                    return Err(error.into());
                }
            };

        if let Err(error) = advance_cursor(
            &transaction,
            tenant_id,
            VARIANT_STREAM_KIND,
            locked_cursor,
            record.sequence_no(),
        )
        .await
        {
            transaction
                .rollback()
                .await
                .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
            return Err(error);
        }
        transaction
            .commit()
            .await
            .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;

        Ok(ProductIndexRefreshRelayStepOutcome::Published {
            sequence_no: record.sequence_no(),
            refresh_id,
        })
    }

    fn validate_database_and_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<(), ProductIndexRefreshRelayError> {
        if tenant_id.is_nil() {
            return Err(ProductIndexRefreshRelayError::InvalidTenant);
        }
        if self.db.get_database_backend() != DatabaseBackend::Postgres {
            return Err(ProductIndexRefreshRelayError::Unavailable);
        }
        Ok(())
    }
}

async fn load_cursor(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    stream_kind: &str,
) -> Result<i64, ProductIndexRefreshRelayError> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT last_sequence_no
            FROM product_index_refresh_relay_cursors
            WHERE tenant_id = $1 AND stream_kind = $2
            "#,
            vec![tenant_id.into(), stream_kind.into()],
        ))
        .await
        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
    let Some(row) = row else {
        return Ok(0);
    };
    let last_sequence_no: i64 = row
        .try_get("", "last_sequence_no")
        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
    if last_sequence_no < 0 {
        return Err(ProductIndexRefreshRelayError::Unavailable);
    }
    Ok(last_sequence_no)
}

async fn lock_cursor(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    stream_kind: &str,
) -> Result<i64, ProductIndexRefreshRelayError> {
    transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO product_index_refresh_relay_cursors (
                tenant_id,
                stream_kind,
                last_sequence_no,
                updated_at
            ) VALUES ($1, $2, 0, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, stream_kind) DO NOTHING
            "#,
            vec![tenant_id.into(), stream_kind.into()],
        ))
        .await
        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;

    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT last_sequence_no
            FROM product_index_refresh_relay_cursors
            WHERE tenant_id = $1 AND stream_kind = $2
            FOR UPDATE
            "#,
            vec![tenant_id.into(), stream_kind.into()],
        ))
        .await
        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?
        .ok_or(ProductIndexRefreshRelayError::Unavailable)?;
    let last_sequence_no: i64 = row
        .try_get("", "last_sequence_no")
        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
    if last_sequence_no < 0 {
        return Err(ProductIndexRefreshRelayError::Unavailable);
    }
    Ok(last_sequence_no)
}

async fn advance_cursor(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    stream_kind: &str,
    expected_sequence_no: i64,
    next_sequence_no: i64,
) -> Result<(), ProductIndexRefreshRelayError> {
    if expected_sequence_no < 0 || next_sequence_no <= expected_sequence_no {
        return Err(ProductIndexRefreshRelayError::Unavailable);
    }
    let result = transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE product_index_refresh_relay_cursors
            SET last_sequence_no = $4,
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1
              AND stream_kind = $2
              AND last_sequence_no = $3
            "#,
            vec![
                tenant_id.into(),
                stream_kind.into(),
                expected_sequence_no.into(),
                next_sequence_no.into(),
            ],
        ))
        .await
        .map_err(|_| ProductIndexRefreshRelayError::Unavailable)?;
    if result.rows_affected() != 1 {
        return Err(ProductIndexRefreshRelayError::Unavailable);
    }
    Ok(())
}

impl From<ProductIndexRefreshPublicationError> for ProductIndexRefreshRelayError {
    fn from(error: ProductIndexRefreshPublicationError) -> Self {
        match error {
            ProductIndexRefreshPublicationError::ContractMismatch => Self::ContractMismatch,
            ProductIndexRefreshPublicationError::CausationMismatch => Self::CausationMismatch,
            ProductIndexRefreshPublicationError::Conflict => Self::Conflict,
            ProductIndexRefreshPublicationError::Unavailable => Self::Unavailable,
        }
    }
}
