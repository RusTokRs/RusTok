//! Governance transitions for release yanking and ownership transfer.

use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait, Value};

use super::helpers::*;
use super::mapping::*;
use super::receipts::*;
use super::*;

impl SeaOrmModuleGovernanceService {
    pub async fn yank_release(
        &self,
        command: ModuleReleaseYankCommand,
    ) -> Result<ModuleReleaseYankResult, ModuleGovernanceError> {
        command.validate()?;
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let release_lock = if backend == DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let release = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id, request_id, status, CAST(publisher_principal AS TEXT) AS publisher_principal \
                     FROM registry_module_releases WHERE slug = {} AND version = {}{release_lock}",
                    mark(1),
                    mark(2),
                ),
                vec![command.slug.clone().into(), command.version.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::ReleaseNotFound)?;
        let release_id: String = required_column(&release, "id")?;
        let request_id: Option<String> = optional_column(&release, "request_id")?;
        let status: String = required_column(&release, "status")?;
        let publisher = required_json_text(&release, "publisher_principal")?;
        let receipt = ReleaseYankReceipt {
            release_id: &release_id,
            context: &command.context,
            actor_principal: &command.actor_principal,
            actor_can_manage_modules: command.actor_can_manage_modules,
            reason: &command.reason,
            reason_code: &command.reason_code,
        };
        if release_yank_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(ModuleReleaseYankResult {
                request_id,
                status: "yanked".to_string(),
            });
        }
        let owner_principal = if command.actor_can_manage_modules {
            None
        } else {
            governance_owner_principal_for_slug(&tx, backend, &command.slug, true).await?
        };
        if !governance_actor_can_manage_release(
            &publisher,
            owner_principal.as_ref(),
            &command.actor_principal,
            command.actor_can_manage_modules,
        ) {
            return Err(ModuleGovernanceError::ReleaseYankUnauthorized);
        }
        if status == "yanked" {
            return Err(ModuleGovernanceError::ReleaseCannotBeYanked(status));
        }
        if status != "active" {
            return Err(ModuleGovernanceError::ReleaseCannotBeYanked(status));
        }
        let update = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_module_releases SET status = 'yanked', yanked_reason = {}, \
                     yanked_by_principal = {}, yanked_at = {now}, updated_at = {now} WHERE id = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                ),
                vec![
                    command.reason.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    release_id.clone().into(),
                ],
            ))
            .await
            .map_err(store_error)?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::ReleaseNotFound);
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, {}, 'release_yanked', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
                mark(7),
            ),
            vec![
                self.infrastructure.prefixed_id("rge").into(),
                command.slug.clone().into(),
                request_id.clone().into(),
                release_id.clone().into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(publisher))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": command.version.clone(),
                    "status": "yanked",
                    "reason_code": command.reason_code.clone(),
                    "reason": command.reason.clone(),
                })))),
            ],
        ))
        .await
        .map_err(store_error)?;
        record_release_yank_receipt(&self.infrastructure, &tx, backend, now, &receipt).await?;
        tx.commit().await.map_err(store_error)?;
        Ok(ModuleReleaseYankResult {
            request_id,
            status: "yanked".to_string(),
        })
    }

    /// Transfers a registry slug binding and records its immutable audit fact
    /// in the same transaction.
    pub async fn transfer_owner(
        &self,
        command: ModuleOwnerTransferCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let previous_owner = governance_owner_principal_for_slug(&tx, backend, &command.slug, true)
            .await?
            .ok_or(ModuleGovernanceError::OwnerBindingNotFound)?;
        let receipt = OwnerTransferReceipt {
            slug: &command.slug,
            context: &command.context,
            previous_owner_principal: &previous_owner,
            new_owner_principal: &command.new_owner_principal,
            actor_principal: &command.actor_principal,
            actor_can_manage_modules: command.actor_can_manage_modules,
            reason: &command.reason,
            reason_code: &command.reason_code,
        };
        if owner_transfer_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(());
        }
        if !governance_actor_can_transfer_owner(
            &previous_owner,
            &command.actor_principal,
            command.actor_can_manage_modules,
        ) {
            return Err(ModuleGovernanceError::OwnerTransferUnauthorized);
        }
        if previous_owner == command.new_owner_principal {
            return Err(ModuleGovernanceError::OwnerUnchanged);
        }

        let update = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_module_owners \
                     SET owner_principal = {}, bound_by_principal = {}, \
                         bound_at = {now}, updated_at = {now} \
                     WHERE slug = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                ),
                vec![
                    Value::Json(Some(Box::new(command.new_owner_principal.clone()))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.slug.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::OwnerBindingNotFound);
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, NULL, NULL, 'owner_transferred', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                self.infrastructure.prefixed_id("rge").into(),
                command.slug.clone().into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(command.new_owner_principal.clone()))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "owner_transition": {
                        "previous_owner": previous_owner.clone(),
                        "new_owner": command.new_owner_principal.clone(),
                        "bound_by": command.actor_principal.clone(),
                    },
                    "reason": command.reason.clone(),
                    "reason_code": command.reason_code.clone(),
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        record_owner_transfer_receipt(&self.infrastructure, &tx, backend, now, &receipt).await?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }
}
