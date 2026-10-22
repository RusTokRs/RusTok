//! Artifact data export service.

use rustok_events::DomainEvent;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait,
    Value as SqlValue,
};
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::traits::*;
use super::types::*;
use super::*;

/// Owner service for a bounded, audited structured-data export page. It is not
/// registered as a sandbox capability and it holds the namespace lifecycle
/// lock only for the page query plus the matching audit/outbox transaction.
#[derive(Clone)]
pub struct SeaOrmArtifactDataExportService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmArtifactDataExportService<A>
where
    A: ArtifactDataExportAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, authorizer, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            infrastructure,
        }
    }

    pub async fn export(
        &self,
        request: ArtifactDataExportRequest,
    ) -> Result<ArtifactDataExportResult, ArtifactDataError> {
        validate_export_request(&request)?;
        self.authorizer.authorize_export(&request).await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.tenant_id).await?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let namespace_revision = require_active_namespace(
            &transaction,
            &request.scope,
            request.expected_namespace_revision,
            backend,
        )
        .await?;
        let query_limit = i64::from(request.page.limit) + 1;
        let prefix_pattern = format!("{}%", escape_like_prefix(&request.page.prefix));
        let (query, values) = match request.page.after_key.as_deref() {
            Some(after_key) => (
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND data_key LIKE {} ESCAPE '\\' AND data_key > {}
                     ORDER BY data_key ASC LIMIT {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                ),
                vec![
                    backend.uuid_value(request.scope.tenant_id),
                    backend.uuid_value(request.scope.data_owner_id),
                    backend.uuid_value(request.scope.namespace_instance_id),
                    prefix_pattern.clone().into(),
                    after_key.to_owned().into(),
                    query_limit.into(),
                ],
            ),
            None => (
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND data_key LIKE {} ESCAPE '\\'
                     ORDER BY data_key ASC LIMIT {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                ),
                vec![
                    backend.uuid_value(request.scope.tenant_id),
                    backend.uuid_value(request.scope.data_owner_id),
                    backend.uuid_value(request.scope.namespace_instance_id),
                    prefix_pattern.into(),
                    query_limit.into(),
                ],
            ),
        };
        let mut records = transaction
            .query_all_raw(Statement::from_sql_and_values(backend.db_backend(), query, values))
            .await
            .map_err(storage_error)?
            .into_iter()
            .map(record_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        let next_after_key = if records.len() > request.page.limit as usize {
            records.truncate(request.page.limit as usize);
            records.last().map(|record| record.key.clone())
        } else {
            None
        };
        let page = ArtifactDataPage {
            records,
            next_after_key,
        };
        let export_id = self.infrastructure.new_id();
        let exported_records =
            i64::try_from(page.records.len()).map_err(|_| ArtifactDataError::ExportPrecondition)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_data_exports
                     (export_id, tenant_id, data_owner_id, namespace_instance_id, policy_revision, namespace_revision,
                      actor_id, trace_id, correlation_id, idempotency_key, prefix, after_key, page_limit, reason,
                      exported_records, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                    backend.placeholder(7),
                    backend.placeholder(8),
                    backend.placeholder(9),
                    backend.placeholder(10),
                    backend.placeholder(11),
                    backend.placeholder(12),
                    backend.placeholder(13),
                    backend.placeholder(14),
                    backend.placeholder(15),
                    backend.now_sql(),
                ),
                vec![
                    backend.uuid_value(export_id),
                    backend.uuid_value(request.scope.tenant_id),
                    backend.uuid_value(request.scope.data_owner_id),
                    backend.uuid_value(request.scope.namespace_instance_id),
                    revision_value(request.scope.policy_revision)?,
                    revision_value(namespace_revision)?,
                    backend.uuid_value(request.context.actor_id),
                    request.context.trace_id.clone().into(),
                    backend.uuid_value(request.context.correlation_id),
                    backend.uuid_value(request.context.idempotency_key),
                    request.page.prefix.clone().into(),
                    request
                        .page
                        .after_key
                        .clone()
                        .map_or(SqlValue::String(None), Into::into),
                    i64::from(request.page.limit).into(),
                    request.reason.clone().into(),
                    exported_records.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactDataExported {
                        export_id,
                        tenant_id: request.scope.tenant_id,
                        module_slug: request.scope.module_slug.clone(),
                        data_contract_revision: request.scope.data_contract_revision,
                        namespace_revision,
                        exported_records: u64::try_from(exported_records)
                            .map_err(|_| ArtifactDataError::ExportPrecondition)?,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataExportResult {
            export_id,
            namespace_revision,
            page,
        })
    }
}
