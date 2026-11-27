//! Row-lock ordering for delivery completion lease checks.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbErr, Statement};
use uuid::Uuid;

use crate::backend::SupportedBackend;

#[derive(Clone, Copy)]
pub(crate) enum DeliveryQueue {
    Event,
    Schedule,
}

/// Acquire the PostgreSQL row lock before evaluating a completion's expiry.
/// An UPDATE can evaluate its WHERE clause before waiting on a row that was
/// merely locked (not changed) by another transaction. A wall-clock predicate
/// in that UPDATE alone therefore does not prove the lease was live after the
/// wait. This lock has no lease predicate; the following guarded UPDATE runs
/// only after it is held and rechecks token, status, owner, attempt and time.
/// SQLite acquires its write lock before evaluating the guarded UPDATE.
pub(crate) async fn lock_for_completion(
    tx: &DatabaseTransaction,
    backend: SupportedBackend,
    queue: DeliveryQueue,
    tenant_id: Uuid,
    delivery_id: Uuid,
) -> Result<bool, DbErr> {
    if backend == SupportedBackend::Sqlite {
        return Ok(true);
    }
    let table = match queue {
        DeliveryQueue::Event => "module_artifact_event_deliveries",
        DeliveryQueue::Schedule => "module_artifact_schedule_deliveries",
    };
    Ok(tx.query_one_raw(Statement::from_sql_and_values(
        backend.db_backend(),
        format!("SELECT delivery_id FROM {table} WHERE tenant_id = $1 AND delivery_id = $2 FOR UPDATE"),
        vec![backend.uuid_value(tenant_id), backend.uuid_value(delivery_id)],
    )).await?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactEventDeliveryCompletion, ArtifactEventDeliveryConfig, ArtifactEventDeliveryError,
        ArtifactEventDeliveryOutcome, ArtifactScheduleDeliveryConfig,
        ArtifactScheduleDeliveryError, ArtifactScheduleDeliveryOutcome,
        SeaOrmArtifactEventDeliveryQueue, SeaOrmArtifactScheduleDeliveryQueue,
    };
    use sea_orm::{ConnectOptions, Database, DbBackend, TransactionTrait};

    async fn connection(url: &str) -> sea_orm::DatabaseConnection {
        let mut options = ConnectOptions::new(url);
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        Database::connect(options).await.unwrap()
    }

    // Separate single-connection pools allow session-local search_path while
    // exercising the actual queue completion APIs. A panic can leave an
    // isolated schema behind, so use only a disposable test database.
    #[tokio::test]
    #[ignore = "requires disposable RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema privileges"]
    async fn postgres_completion_rechecks_expiry_after_row_lock_wait() {
        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").expect("test database URL");
        let locker = connection(&url).await;
        let worker = connection(&url).await;
        let monitor = connection(&url).await;
        let schema = format!("delivery_lock_test_{}", Uuid::new_v4().simple());
        locker.execute_unprepared(&format!(
            "CREATE SCHEMA {schema}; SET search_path TO {schema}; \
             CREATE TABLE module_artifact_event_deliveries (delivery_id UUID PRIMARY KEY, tenant_id UUID, status TEXT, attempt INTEGER, claimed_by TEXT, claimed_until TIMESTAMPTZ, claim_token UUID, available_at TIMESTAMPTZ, last_error_code TEXT, completed_at TIMESTAMPTZ, dead_lettered_at TIMESTAMPTZ, cancelled_at TIMESTAMPTZ); \
             CREATE TABLE module_artifact_schedule_deliveries (LIKE module_artifact_event_deliveries INCLUDING ALL)"
        )).await.unwrap();
        worker
            .execute_unprepared(&format!("SET search_path TO {schema}"))
            .await
            .unwrap();
        let pid: i32 = worker
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT pg_backend_pid() AS pid".to_string(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "pid")
            .unwrap();
        let event_queue = SeaOrmArtifactEventDeliveryQueue::new(
            worker.clone(),
            ArtifactEventDeliveryConfig::default(),
        )
        .unwrap();
        let schedule_queue = SeaOrmArtifactScheduleDeliveryQueue::new(
            worker.clone(),
            ArtifactScheduleDeliveryConfig::default(),
        )
        .unwrap();
        for kind in [DeliveryQueue::Event, DeliveryQueue::Schedule] {
            let table = match kind {
                DeliveryQueue::Event => "module_artifact_event_deliveries",
                DeliveryQueue::Schedule => "module_artifact_schedule_deliveries",
            };
            let tenant = Uuid::new_v4();
            let delivery = Uuid::new_v4();
            let token = Uuid::new_v4();
            locker.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                format!("INSERT INTO {table} (tenant_id,delivery_id,status,attempt,claimed_by,claimed_until,claim_token,available_at) VALUES ($1,$2,'running',1,'worker',clock_timestamp() + interval '2 seconds',$3,clock_timestamp())"),
                vec![tenant.into(), delivery.into(), token.into()])).await.unwrap();
            let tx = locker.begin().await.unwrap();
            tx.query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!("SELECT delivery_id FROM {table} WHERE delivery_id=$1 FOR UPDATE"),
                vec![delivery.into()],
            ))
            .await
            .unwrap();
            let event = event_queue.clone();
            let schedule = schedule_queue.clone();
            let task = tokio::spawn(async move {
                match kind {
                    DeliveryQueue::Event => {
                        let result = event
                            .complete(
                                tenant,
                                "worker",
                                ArtifactEventDeliveryCompletion {
                                    delivery_id: delivery,
                                    attempt: 1,
                                    claim_token: token,
                                    outcome: ArtifactEventDeliveryOutcome::Succeeded,
                                },
                            )
                            .await;
                        (
                            matches!(&result, Err(ArtifactEventDeliveryError::ClaimLost)),
                            format!("{result:?}"),
                        )
                    }
                    DeliveryQueue::Schedule => {
                        let result = schedule
                            .complete(
                                tenant,
                                "worker",
                                delivery,
                                1,
                                token,
                                ArtifactScheduleDeliveryOutcome::Held {
                                    error_code: "policy_denied".into(),
                                },
                            )
                            .await;
                        (
                            matches!(&result, Err(ArtifactScheduleDeliveryError::ClaimLost)),
                            format!("{result:?}"),
                        )
                    }
                }
            });
            let mut blocked = false;
            for _ in 0..100 {
                let row = monitor.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND wait_event_type='Lock') AS blocked", vec![pid.into()])).await.unwrap().unwrap();
                if row.try_get::<bool>("", "blocked").unwrap() {
                    blocked = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            assert!(blocked, "completion did not wait for the row lock");
            tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
            tx.commit().await.unwrap();
            let (lost, result) = task.await.unwrap();
            assert!(lost, "expired completion unexpectedly returned {result}");
            let row = locker
                .query_one_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    format!("SELECT status,attempt,claim_token FROM {table} WHERE delivery_id=$1"),
                    vec![delivery.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<String>("", "status").unwrap(), "running");
            assert_eq!(row.try_get::<i32>("", "attempt").unwrap(), 1);
            assert_eq!(row.try_get::<Uuid>("", "claim_token").unwrap(), token);
        }
        locker
            .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
    }
}
