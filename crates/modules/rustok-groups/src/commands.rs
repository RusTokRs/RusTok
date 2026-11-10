//! Durable command admission. Acquire before any owner reads or row locks.
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};
use chrono::Utc;
use rustok_api::PortContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, Set, Statement,
};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) async fn reserve_command(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    key: &str,
) -> GroupsResult<()> {
    if key.trim().is_empty() || key.trim().len() > 160 {
        return Err(GroupsError::Validation(
            "idempotency key must contain 1..160 bytes".into(),
        ));
    }
    match transaction.get_database_backend() {
        DbBackend::Postgres => {
            let digest =
                Sha256::digest([tenant_id.as_bytes().as_slice(), key.trim().as_bytes()].concat());
            let mut bytes = [0; 8];
            bytes.copy_from_slice(&digest[..8]);
            // Hash collisions only serialize unrelated commands; receipts still compare exact keys.
            transaction
                .query_one_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT pg_advisory_xact_lock($1)",
                    [i64::from_be_bytes(bytes).into()],
                ))
                .await?;
        }
        DbBackend::Sqlite => reserve_sqlite_writer(transaction).await?,
        _ => {
            return Err(GroupsError::Invariant(
                "Groups command admission supports PostgreSQL and SQLite only".into(),
            ));
        }
    }
    Ok(())
}

/// SQLite serializes writers database-wide. Reserve before locator reads as well as commands:
/// a later promotion of an existing read snapshot can fail even when the target row is locked.
/// The empty UPDATE does not mutate receipts or fire per-row triggers.
pub(crate) async fn reserve_sqlite_writer(transaction: &DatabaseTransaction) -> GroupsResult<()> {
    if transaction.get_database_backend() == DbBackend::Sqlite {
        transaction
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE group_command_receipts SET idempotency_key = idempotency_key WHERE 1 = 0",
            ))
            .await?;
    }
    Ok(())
}

pub(crate) struct Command {
    tenant_id: Uuid,
    actor_id: Uuid,
    key: String,
    kind: &'static str,
    hash: String,
}

impl Command {
    pub(crate) async fn admit<T: Serialize>(
        transaction: &DatabaseTransaction,
        context: &PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        kind: &'static str,
        request: &T,
    ) -> GroupsResult<Self> {
        let key = context
            .idempotency_key
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string();
        reserve_command(transaction, tenant_id, &key).await?;
        let bytes =
            serde_json::to_vec(request).map_err(|e| GroupsError::Validation(e.to_string()))?;
        Ok(Self {
            tenant_id,
            actor_id,
            key,
            kind,
            hash: crate::domain::sha256_hex(&bytes),
        })
    }

    pub(crate) async fn replay<T: DeserializeOwned>(
        &self,
        transaction: &DatabaseTransaction,
    ) -> GroupsResult<Option<T>> {
        let Some(row) = command_receipt::Entity::find()
            .filter(command_receipt::Column::TenantId.eq(self.tenant_id))
            .filter(command_receipt::Column::IdempotencyKey.eq(&self.key))
            .one(transaction)
            .await?
        else {
            return Ok(None);
        };
        if row.actor_user_id != self.actor_id
            || row.command_type != self.kind
            || row.request_hash != self.hash
        {
            return Err(GroupsError::Conflict(
                "idempotency key was already used for another group command".into(),
            ));
        }
        serde_json::from_value(row.response)
            .map(Some)
            .map_err(|e| GroupsError::Invariant(format!("invalid command receipt: {e}")))
    }

    pub(crate) async fn finish<T: Serialize>(
        &self,
        transaction: &DatabaseTransaction,
        context: &PortContext,
        group_id: Uuid,
        result: &T,
    ) -> GroupsResult<()> {
        let now = Utc::now().fixed_offset();
        command_receipt::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(self.tenant_id),
            group_id: Set(group_id),
            actor_user_id: Set(self.actor_id),
            idempotency_key: Set(self.key.clone()),
            command_type: Set(self.kind.into()),
            request_hash: Set(self.hash.clone()),
            response: Set(
                serde_json::to_value(result).map_err(|e| GroupsError::Invariant(e.to_string()))?
            ),
            created_at: Set(now),
        }
        .insert(transaction)
        .await?;
        audit_entry::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(self.tenant_id),
            group_id: Set(group_id),
            actor_user_id: Set(Some(self.actor_id)),
            action: Set(self.kind.into()),
            target_user_id: Set(None),
            details: Set(serde_json::json!({"command_type": self.kind})),
            correlation_id: Set(context.correlation_id.clone()),
            created_at: Set(now),
        }
        .insert(transaction)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, Database, TransactionTrait};

    #[tokio::test]
    async fn sqlite_writer_reservation_precedes_locator_reads_without_mutating_receipts() {
        let path = std::env::temp_dir().join(format!("groups-admission-{}.sqlite", Uuid::new_v4()));
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let mut options = ConnectOptions::new(url.clone());
        options.max_connections(1).min_connections(1);
        let first = Database::connect(options).await.unwrap();
        let mut options = ConnectOptions::new(url);
        options.max_connections(1).min_connections(1);
        let second = Database::connect(options).await.unwrap();
        first
            .execute_unprepared(
                "CREATE TABLE group_command_receipts (idempotency_key TEXT PRIMARY KEY)",
            )
            .await
            .unwrap();
        first
            .execute_unprepared("INSERT INTO group_command_receipts VALUES ('unchanged')")
            .await
            .unwrap();
        second
            .execute_unprepared("PRAGMA busy_timeout = 0")
            .await
            .unwrap();

        let first_tx = first.begin().await.unwrap();
        reserve_sqlite_writer(&first_tx).await.unwrap();
        let second_tx = second.begin().await.unwrap();
        assert!(reserve_sqlite_writer(&second_tx).await.is_err());
        second_tx.rollback().await.unwrap();
        first_tx.commit().await.unwrap();

        let second_tx = second.begin().await.unwrap();
        reserve_sqlite_writer(&second_tx).await.unwrap();
        let row = second_tx
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT idempotency_key FROM group_command_receipts",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<String>("", "idempotency_key").unwrap(),
            "unchanged"
        );
        second_tx.commit().await.unwrap();
        first.close().await.unwrap();
        second.close().await.unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
