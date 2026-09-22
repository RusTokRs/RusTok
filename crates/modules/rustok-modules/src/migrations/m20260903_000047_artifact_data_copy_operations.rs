use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

/// Frozen page requests commit before record writes; exact receipts commit with them.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let (uuid, json, timestamp, digest_check) = match backend {
            DbBackend::Postgres => (
                "UUID",
                "JSONB",
                "TIMESTAMPTZ",
                "VALUE ~ '^sha256:[0-9a-f]{64}$'",
            ),
            DbBackend::Sqlite => (
                "TEXT",
                "JSON",
                "TEXT",
                "length(VALUE)=71 AND substr(VALUE,1,7)='sha256:' AND substr(VALUE,8) NOT GLOB '*[^0-9a-f]*'",
            ),
            backend => {
                return Err(DbErr::Migration(format!(
                    "Artifact record copy does not support {backend:?}"
                )));
            }
        };
        manager.get_connection().execute_unprepared(&format!(
            "CREATE TABLE module_artifact_data_copy_operations (
                operation_id {uuid} PRIMARY KEY NOT NULL,tenant_id {uuid} NOT NULL,data_owner_id {uuid} NOT NULL,
                source_namespace_instance_id {uuid} NOT NULL,target_namespace_instance_id {uuid} NOT NULL,
                expected_source_namespace_revision BIGINT NOT NULL CHECK(expected_source_namespace_revision>0),
                expected_target_namespace_revision BIGINT NOT NULL CHECK(expected_target_namespace_revision>0),
                idempotency_key {uuid} NOT NULL,request_digest TEXT NOT NULL CHECK({}),
                request_json {json} NOT NULL,frozen_page_json {json} NOT NULL,
                frozen_page_digest TEXT NOT NULL CHECK({}),
                status TEXT NOT NULL CHECK(status IN ('preparing','committing','committed')),
                receipt_json {json} NULL,receipt_digest TEXT NULL CHECK(receipt_digest IS NULL OR ({})),
                actor_id {uuid} NOT NULL,trace_id TEXT NOT NULL CHECK(length(trim(trace_id)) BETWEEN 1 AND 512),
                correlation_id {uuid} NOT NULL,reason TEXT NOT NULL CHECK(length(trim(reason)) BETWEEN 1 AND 2000),
                created_at {timestamp} NOT NULL,committed_at {timestamp} NULL,
                CHECK(source_namespace_instance_id<>target_namespace_instance_id),
                CHECK((status='committed' AND receipt_json IS NOT NULL AND receipt_digest IS NOT NULL AND committed_at IS NOT NULL)
                   OR(status<>'committed' AND receipt_json IS NULL AND receipt_digest IS NULL AND committed_at IS NULL)),
                UNIQUE(tenant_id,data_owner_id,idempotency_key),
                UNIQUE(tenant_id,data_owner_id,target_namespace_instance_id,expected_target_namespace_revision),
                FOREIGN KEY(tenant_id,data_owner_id,source_namespace_instance_id)
                    REFERENCES module_artifact_data_namespaces(tenant_id,data_owner_id,namespace_instance_id) ON UPDATE RESTRICT ON DELETE RESTRICT,
                FOREIGN KEY(tenant_id,data_owner_id,target_namespace_instance_id)
                    REFERENCES module_artifact_data_namespaces(tenant_id,data_owner_id,namespace_instance_id) ON UPDATE RESTRICT ON DELETE RESTRICT
            )",digest_check.replace("VALUE","request_digest"),digest_check.replace("VALUE","frozen_page_digest"),
            digest_check.replace("VALUE","receipt_digest"))).await?;
        manager.get_connection().execute_unprepared(
            "CREATE INDEX idx_artifact_data_copy_ops_scope ON module_artifact_data_copy_operations
             (tenant_id,data_owner_id,target_namespace_instance_id,expected_target_namespace_revision,status)").await?;
        let transitions = "(OLD.status='preparing' AND NEW.status='committing') OR (OLD.status='committing' AND NEW.status='committed')";
        if backend == DbBackend::Postgres {
            for sql in [
                "ALTER TABLE module_artifact_data_copy_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_copy_operations_scope ON module_artifact_data_copy_operations
                 USING(tenant_id::text=current_setting('rustok.tenant_id',true))
                 WITH CHECK(tenant_id::text=current_setting('rustok.tenant_id',true))",
                "CREATE FUNCTION reject_artifact_data_copy_receipt_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
                 BEGIN
                   IF TG_OP='DELETE' THEN RAISE EXCEPTION 'Artifact record-copy evidence cannot be deleted'; END IF;
                   IF OLD.status='committed' OR
                     (to_jsonb(NEW)-'status'-'receipt_json'-'receipt_digest'-'committed_at') IS DISTINCT FROM
                     (to_jsonb(OLD)-'status'-'receipt_json'-'receipt_digest'-'committed_at') OR
                     NOT((OLD.status='preparing' AND NEW.status='committing') OR (OLD.status='committing' AND NEW.status='committed')) THEN
                     RAISE EXCEPTION 'Artifact record-copy receipts are immutable';
                   END IF; RETURN NEW; END $$",
                "CREATE TRIGGER artifact_data_copy_receipt_immutable BEFORE UPDATE OR DELETE
                 ON module_artifact_data_copy_operations FOR EACH ROW EXECUTE FUNCTION reject_artifact_data_copy_receipt_mutation()",
            ] {manager.get_connection().execute_unprepared(sql).await?;}
        } else {
            let changed = [
                "operation_id",
                "tenant_id",
                "data_owner_id",
                "source_namespace_instance_id",
                "target_namespace_instance_id",
                "expected_source_namespace_revision",
                "expected_target_namespace_revision",
                "idempotency_key",
                "request_digest",
                "request_json",
                "frozen_page_json",
                "frozen_page_digest",
                "actor_id",
                "trace_id",
                "correlation_id",
                "reason",
                "created_at",
            ]
            .iter()
            .map(|column| format!("OLD.{column} IS NOT NEW.{column}"))
            .collect::<Vec<_>>()
            .join(" OR ");
            for sql in [
                format!("CREATE TRIGGER artifact_data_copy_receipt_immutable_update BEFORE UPDATE ON module_artifact_data_copy_operations
                    WHEN OLD.status='committed' OR ({changed}) OR NOT({transitions})
                    BEGIN SELECT RAISE(ABORT,'Artifact record-copy receipts are immutable'); END"),
                "CREATE TRIGGER artifact_data_copy_receipt_immutable_delete BEFORE DELETE ON module_artifact_data_copy_operations
                 BEGIN SELECT RAISE(ABORT,'Artifact record-copy evidence cannot be deleted'); END".into(),
                "CREATE TRIGGER artifact_data_copy_receipt_json_insert BEFORE INSERT ON module_artifact_data_copy_operations
                 WHEN NOT json_valid(NEW.request_json) OR NOT json_valid(NEW.frozen_page_json)
                   OR(NEW.receipt_json IS NOT NULL AND NOT json_valid(NEW.receipt_json))
                 BEGIN SELECT RAISE(ABORT,'Artifact record-copy evidence requires valid JSON'); END".into(),
                "CREATE TRIGGER artifact_data_copy_receipt_json_update BEFORE UPDATE ON module_artifact_data_copy_operations
                 WHEN NEW.receipt_json IS NOT NULL AND NOT json_valid(NEW.receipt_json)
                 BEGIN SELECT RAISE(ABORT,'Artifact record-copy evidence requires valid JSON'); END".into(),
            ] {manager.get_connection().execute_unprepared(&sql).await?;}
        }
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS module_artifact_data_copy_operations")
            .await?;
        if manager.get_database_backend() == DbBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "DROP FUNCTION IF EXISTS reject_artifact_data_copy_receipt_mutation()",
                )
                .await?;
        }
        Ok(())
    }
}

/// Latest-page ownership retains both namespaces between non-terminal pages.
/// Only the owner commit transaction sees a committing target; other writers
/// still see preparing and serialize on the namespace roots.
pub(crate) fn record_copy_hold(
    backend: DbBackend,
    row: &str,
    allow_committing_target: bool,
    authorized_target: Option<&str>,
) -> String {
    let nonterminal = match backend {
        DbBackend::Postgres => {
            "(copy.receipt_json->'is_terminal_page') IS DISTINCT FROM 'true'::jsonb"
        }
        _ => "json_type(copy.receipt_json,'$.is_terminal_page') IS NOT 'true'",
    };
    let target_states = if allow_committing_target {
        "copy.status='preparing'"
    } else {
        "copy.status IN ('preparing','committing')"
    };
    let exclude = authorized_target
        .map(|target| format!(" AND copy.target_namespace_instance_id<>{target}"))
        .unwrap_or_default();
    format!("EXISTS(SELECT 1 FROM module_artifact_data_copy_operations copy
        WHERE copy.tenant_id={row}.tenant_id AND copy.data_owner_id={row}.data_owner_id
         {exclude}
         AND NOT EXISTS(SELECT 1 FROM module_artifact_data_copy_operations newer
             WHERE newer.tenant_id=copy.tenant_id AND newer.data_owner_id=copy.data_owner_id
              AND newer.target_namespace_instance_id=copy.target_namespace_instance_id
              AND newer.expected_target_namespace_revision>copy.expected_target_namespace_revision)
         AND ((copy.source_namespace_instance_id={row}.namespace_instance_id
                AND(copy.status IN ('preparing','committing') OR(copy.status='committed' AND {nonterminal})))
          OR(copy.target_namespace_instance_id={row}.namespace_instance_id
                AND({target_states} OR(copy.status='committed' AND {nonterminal})))))")
}
