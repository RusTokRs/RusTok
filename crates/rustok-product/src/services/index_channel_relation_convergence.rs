use std::time::Duration;

use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbBackend,
    QueryResult, Statement, TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

const MAX_LEASE_SECONDS: u64 = 86_400;
const MAX_RETRY_SECONDS: u64 = 86_400;
pub const MAX_PRODUCT_SALES_CHANNEL_CONVERGENCE_ERROR_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductSalesChannelIndexRelationConvergenceWork {
    VisibilityRequest {
        sequence_no: i64,
        product_id: Uuid,
        product_source_version: u64,
    },
    ChannelSweep {
        generation: u64,
        after_product_id: Option<Uuid>,
    },
}

impl ProductSalesChannelIndexRelationConvergenceWork {
    fn validate(&self) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
        match self {
            Self::VisibilityRequest {
                sequence_no,
                product_id,
                product_source_version,
            } => {
                if *sequence_no <= 0 || product_id.is_nil() || *product_source_version == 0 {
                    return Err(
                        ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState,
                    );
                }
            }
            Self::ChannelSweep {
                after_product_id, ..
            } => {
                if after_product_id.is_some_and(|value| value.is_nil()) {
                    return Err(
                        ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState,
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSalesChannelIndexRelationConvergenceClaim {
    tenant_id: Uuid,
    lease_token: Uuid,
    work: ProductSalesChannelIndexRelationConvergenceWork,
}

impl ProductSalesChannelIndexRelationConvergenceClaim {
    pub fn restore(
        tenant_id: Uuid,
        lease_token: Uuid,
        work: ProductSalesChannelIndexRelationConvergenceWork,
    ) -> Result<Self, ProductSalesChannelIndexRelationConvergenceError> {
        validate_tenant(tenant_id)?;
        if lease_token.is_nil() {
            return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
        }
        work.validate()?;
        Ok(Self {
            tenant_id,
            lease_token,
            work,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn lease_token(&self) -> Uuid {
        self.lease_token
    }

    pub fn work(&self) -> &ProductSalesChannelIndexRelationConvergenceWork {
        &self.work
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductSalesChannelIndexRelationConvergenceClaimOutcome {
    Idle,
    Busy,
    Claimed(ProductSalesChannelIndexRelationConvergenceClaim),
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProductSalesChannelIndexRelationConvergenceError {
    #[error("Product-SalesChannel convergence tenant identity is invalid")]
    InvalidTenant,
    #[error("Product-SalesChannel convergence Channel identity generation is invalid")]
    InvalidChannelIdentityGeneration,
    #[error("Product-SalesChannel convergence lease duration is invalid")]
    InvalidLeaseDuration,
    #[error("Product-SalesChannel convergence retry delay is invalid")]
    InvalidRetryDelay,
    #[error("Product-SalesChannel convergence error marker is invalid")]
    InvalidErrorMarker,
    #[error("Product-SalesChannel convergence Channel identity generation regressed")]
    WatermarkRegressed,
    #[error("Product-SalesChannel convergence lease was lost")]
    LeaseLost,
    #[error("Product-SalesChannel convergence stored state is invalid")]
    InvalidStoredState,
    #[error("Product-SalesChannel convergence storage is unavailable")]
    Unavailable,
}

#[derive(Clone)]
pub struct ProductSalesChannelIndexRelationConvergenceStore {
    db: DatabaseConnection,
}

impl ProductSalesChannelIndexRelationConvergenceStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn claim(
        &self,
        tenant_id: Uuid,
        observed_channel_identity_generation: u64,
        lease_duration: Duration,
    ) -> Result<
        ProductSalesChannelIndexRelationConvergenceClaimOutcome,
        ProductSalesChannelIndexRelationConvergenceError,
    > {
        validate_tenant(tenant_id)?;
        ensure_postgres(&self.db)?;
        let observed_channel_identity_generation = non_negative_i64(
            observed_channel_identity_generation,
            ProductSalesChannelIndexRelationConvergenceError::InvalidChannelIdentityGeneration,
        )?;
        let lease_seconds = positive_bounded_seconds(
            lease_duration,
            MAX_LEASE_SECONDS,
            ProductSalesChannelIndexRelationConvergenceError::InvalidLeaseDuration,
        )?;

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
        let result = self
            .claim_in_transaction(
                &transaction,
                tenant_id,
                observed_channel_identity_generation,
                lease_seconds,
            )
            .await;
        finish(transaction, result).await
    }

    pub async fn complete_visibility(
        &self,
        claim: &ProductSalesChannelIndexRelationConvergenceClaim,
    ) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
        ensure_postgres(&self.db)?;
        let sequence_no = match claim.work() {
            ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest {
                sequence_no,
                ..
            } => *sequence_no,
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep { .. } => {
                return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
            }
        };
        if sequence_no <= 0 {
            return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
        let result = complete_visibility_in_transaction(&transaction, claim, sequence_no).await;
        finish(transaction, result).await
    }

    pub async fn complete_sweep_page(
        &self,
        claim: &ProductSalesChannelIndexRelationConvergenceClaim,
        next_after_product_id: Option<Uuid>,
    ) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
        ensure_postgres(&self.db)?;
        if next_after_product_id.is_some_and(|value| value.is_nil()) {
            return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
        }
        let generation = match claim.work() {
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                generation, ..
            } => *generation,
            ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest { .. } => {
                return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
            }
        };
        let generation = non_negative_i64(
            generation,
            ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
        let result = complete_sweep_page_in_transaction(
            &transaction,
            claim,
            generation,
            next_after_product_id,
        )
        .await;
        finish(transaction, result).await
    }

    pub async fn retry(
        &self,
        claim: &ProductSalesChannelIndexRelationConvergenceClaim,
        delay: Duration,
        error_marker: impl Into<String>,
    ) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
        ensure_postgres(&self.db)?;
        let delay_seconds = positive_bounded_seconds(
            delay,
            MAX_RETRY_SECONDS,
            ProductSalesChannelIndexRelationConvergenceError::InvalidRetryDelay,
        )?;
        let error_marker = error_marker.into();
        if error_marker.is_empty()
            || error_marker.len() > MAX_PRODUCT_SALES_CHANNEL_CONVERGENCE_ERROR_BYTES
        {
            return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidErrorMarker);
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
        let result = retry_in_transaction(&transaction, claim, delay_seconds, error_marker).await;
        finish(transaction, result).await
    }

    async fn claim_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        observed_channel_identity_generation: i64,
        lease_seconds: i64,
    ) -> Result<
        ProductSalesChannelIndexRelationConvergenceClaimOutcome,
        ProductSalesChannelIndexRelationConvergenceError,
    > {
        transaction
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                INSERT INTO product_sales_channel_index_relation_convergence_state (tenant_id)
                VALUES ($1)
                ON CONFLICT (tenant_id) DO NOTHING
                "#,
                vec![tenant_id.into()],
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;

        let row = transaction
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    visibility_cursor,
                    channel_identity_generation,
                    sweep_generation,
                    sweep_after_product_id,
                    attempt_count,
                    lease_token IS NOT NULL
                        AND lease_expires_at > CURRENT_TIMESTAMP AS lease_active
                FROM product_sales_channel_index_relation_convergence_state
                WHERE tenant_id = $1
                FOR UPDATE
                "#,
                vec![tenant_id.into()],
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?
            .ok_or(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
        let state = decode_state(row)?;
        if state.lease_active {
            return Ok(ProductSalesChannelIndexRelationConvergenceClaimOutcome::Busy);
        }
        if state
            .channel_identity_generation
            .is_some_and(|value| value > observed_channel_identity_generation)
            || state
                .sweep_generation
                .is_some_and(|value| value > observed_channel_identity_generation)
        {
            return Err(ProductSalesChannelIndexRelationConvergenceError::WatermarkRegressed);
        }
        if state.attempt_count == i64::MAX {
            return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
        }

        let work = if let Some(sweep_generation) = state.sweep_generation {
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                generation: u64::try_from(sweep_generation).map_err(|_| {
                    ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState
                })?,
                after_product_id: state.sweep_after_product_id,
            }
        } else if let Some(request) =
            load_next_visibility_request(transaction, tenant_id, state.visibility_cursor).await?
        {
            ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest {
                sequence_no: request.sequence_no,
                product_id: request.product_id,
                product_source_version: request.product_source_version,
            }
        } else if state.channel_identity_generation.is_none()
            || state.channel_identity_generation < Some(observed_channel_identity_generation)
        {
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                generation: u64::try_from(observed_channel_identity_generation).map_err(|_| {
                    ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState
                })?,
                after_product_id: None,
            }
        } else {
            return Ok(ProductSalesChannelIndexRelationConvergenceClaimOutcome::Idle);
        };
        work.validate()?;

        let lease_token = Uuid::new_v4();
        let starting_sweep_generation = match &work {
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                generation, ..
            } if state.sweep_generation.is_none() => Some(non_negative_i64(
                *generation,
                ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState,
            )?),
            _ => None,
        };
        let result = transaction
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE product_sales_channel_index_relation_convergence_state
                SET sweep_generation = COALESCE(sweep_generation, $2),
                    sweep_after_product_id = CASE
                        WHEN sweep_generation IS NULL THEN NULL
                        ELSE sweep_after_product_id
                    END,
                    lease_token = $3,
                    lease_expires_at = CURRENT_TIMESTAMP + ($4 * INTERVAL '1 second'),
                    attempt_count = attempt_count + 1,
                    last_error = NULL,
                    updated_at = CURRENT_TIMESTAMP
                WHERE tenant_id = $1
                "#,
                vec![
                    tenant_id.into(),
                    starting_sweep_generation.into(),
                    lease_token.into(),
                    lease_seconds.into(),
                ],
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
        if result.rows_affected() != 1 {
            return Err(ProductSalesChannelIndexRelationConvergenceError::LeaseLost);
        }

        Ok(
            ProductSalesChannelIndexRelationConvergenceClaimOutcome::Claimed(
                ProductSalesChannelIndexRelationConvergenceClaim {
                    tenant_id,
                    lease_token,
                    work,
                },
            ),
        )
    }
}

#[derive(Debug)]
struct ConvergenceState {
    visibility_cursor: i64,
    channel_identity_generation: Option<i64>,
    sweep_generation: Option<i64>,
    sweep_after_product_id: Option<Uuid>,
    attempt_count: i64,
    lease_active: bool,
}

#[derive(Debug)]
struct VisibilityRequest {
    sequence_no: i64,
    product_id: Uuid,
    product_source_version: u64,
}

fn decode_state(
    row: QueryResult,
) -> Result<ConvergenceState, ProductSalesChannelIndexRelationConvergenceError> {
    let visibility_cursor = row
        .try_get::<i64>("", "visibility_cursor")
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
    let channel_identity_generation = row
        .try_get::<Option<i64>>("", "channel_identity_generation")
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
    let sweep_generation = row
        .try_get::<Option<i64>>("", "sweep_generation")
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
    let sweep_after_product_id = row
        .try_get::<Option<Uuid>>("", "sweep_after_product_id")
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
    let attempt_count = row
        .try_get::<i64>("", "attempt_count")
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
    let lease_active = row
        .try_get::<bool>("", "lease_active")
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
    if visibility_cursor < 0
        || channel_identity_generation.is_some_and(|value| value < 0)
        || sweep_generation.is_some_and(|value| value < 0)
        || sweep_after_product_id.is_some_and(|value| value.is_nil())
        || (sweep_after_product_id.is_some() && sweep_generation.is_none())
        || attempt_count < 0
    {
        return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
    }
    Ok(ConvergenceState {
        visibility_cursor,
        channel_identity_generation,
        sweep_generation,
        sweep_after_product_id,
        attempt_count,
        lease_active,
    })
}

async fn load_next_visibility_request(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    visibility_cursor: i64,
) -> Result<Option<VisibilityRequest>, ProductSalesChannelIndexRelationConvergenceError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT sequence_no, product_id, product_source_version
            FROM product_sales_channel_index_relation_convergence_requests
            WHERE tenant_id = $1
              AND sequence_no > $2
            ORDER BY sequence_no ASC
            LIMIT 1
            "#,
            vec![tenant_id.into(), visibility_cursor.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
    row.map(|row| {
        let sequence_no = row
            .try_get::<i64>("", "sequence_no")
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
        let product_source_version = row
            .try_get::<i64>("", "product_source_version")
            .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState)?;
        if sequence_no <= 0 || product_id.is_nil() || product_source_version <= 0 {
            return Err(ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState);
        }
        Ok(VisibilityRequest {
            sequence_no,
            product_id,
            product_source_version: u64::try_from(product_source_version).map_err(|_| {
                ProductSalesChannelIndexRelationConvergenceError::InvalidStoredState
            })?,
        })
    })
    .transpose()
}

async fn complete_visibility_in_transaction(
    transaction: &DatabaseTransaction,
    claim: &ProductSalesChannelIndexRelationConvergenceClaim,
    sequence_no: i64,
) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
    let result = transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE product_sales_channel_index_relation_convergence_state
            SET visibility_cursor = $3,
                lease_token = NULL,
                lease_expires_at = NULL,
                available_at = CURRENT_TIMESTAMP,
                last_error = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1
              AND lease_token = $2
              AND visibility_cursor < $3
            "#,
            vec![
                claim.tenant_id.into(),
                claim.lease_token.into(),
                sequence_no.into(),
            ],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
    if result.rows_affected() != 1 {
        return Err(ProductSalesChannelIndexRelationConvergenceError::LeaseLost);
    }
    Ok(())
}

async fn complete_sweep_page_in_transaction(
    transaction: &DatabaseTransaction,
    claim: &ProductSalesChannelIndexRelationConvergenceClaim,
    generation: i64,
    next_after_product_id: Option<Uuid>,
) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
    let (channel_identity_generation, sweep_generation, sweep_after_product_id) =
        if let Some(next_after_product_id) = next_after_product_id {
            (None::<i64>, Some(generation), Some(next_after_product_id))
        } else {
            (Some(generation), None::<i64>, None::<Uuid>)
        };
    let result = transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE product_sales_channel_index_relation_convergence_state
            SET channel_identity_generation = COALESCE($4, channel_identity_generation),
                sweep_generation = $5,
                sweep_after_product_id = $6,
                lease_token = NULL,
                lease_expires_at = NULL,
                available_at = CURRENT_TIMESTAMP,
                last_error = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1
              AND lease_token = $2
              AND sweep_generation = $3
            "#,
            vec![
                claim.tenant_id.into(),
                claim.lease_token.into(),
                generation.into(),
                channel_identity_generation.into(),
                sweep_generation.into(),
                sweep_after_product_id.into(),
            ],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
    if result.rows_affected() != 1 {
        return Err(ProductSalesChannelIndexRelationConvergenceError::LeaseLost);
    }
    Ok(())
}

async fn retry_in_transaction(
    transaction: &DatabaseTransaction,
    claim: &ProductSalesChannelIndexRelationConvergenceClaim,
    delay_seconds: i64,
    error_marker: String,
) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
    let result = transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE product_sales_channel_index_relation_convergence_state
            SET lease_token = NULL,
                lease_expires_at = NULL,
                available_at = CURRENT_TIMESTAMP + ($3 * INTERVAL '1 second'),
                last_error = $4,
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1
              AND lease_token = $2
            "#,
            vec![
                claim.tenant_id.into(),
                claim.lease_token.into(),
                delay_seconds.into(),
                error_marker.into(),
            ],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
    if result.rows_affected() != 1 {
        return Err(ProductSalesChannelIndexRelationConvergenceError::LeaseLost);
    }
    Ok(())
}

async fn finish<T>(
    transaction: DatabaseTransaction,
    result: Result<T, ProductSalesChannelIndexRelationConvergenceError>,
) -> Result<T, ProductSalesChannelIndexRelationConvergenceError> {
    match result {
        Ok(value) => {
            transaction
                .commit()
                .await
                .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
            Ok(value)
        }
        Err(error) => {
            transaction
                .rollback()
                .await
                .map_err(|_| ProductSalesChannelIndexRelationConvergenceError::Unavailable)?;
            Err(error)
        }
    }
}

fn validate_tenant(
    tenant_id: Uuid,
) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
    if tenant_id.is_nil() {
        Err(ProductSalesChannelIndexRelationConvergenceError::InvalidTenant)
    } else {
        Ok(())
    }
}

fn ensure_postgres(
    db: &DatabaseConnection,
) -> Result<(), ProductSalesChannelIndexRelationConvergenceError> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        Err(ProductSalesChannelIndexRelationConvergenceError::Unavailable)
    } else {
        Ok(())
    }
}

fn non_negative_i64(
    value: u64,
    error: ProductSalesChannelIndexRelationConvergenceError,
) -> Result<i64, ProductSalesChannelIndexRelationConvergenceError> {
    i64::try_from(value).map_err(|_| error)
}

fn positive_bounded_seconds(
    duration: Duration,
    maximum: u64,
    error: ProductSalesChannelIndexRelationConvergenceError,
) -> Result<i64, ProductSalesChannelIndexRelationConvergenceError> {
    let seconds = duration.as_secs();
    if seconds == 0 || seconds > maximum {
        return Err(error);
    }
    i64::try_from(seconds).map_err(|_| error)
}
