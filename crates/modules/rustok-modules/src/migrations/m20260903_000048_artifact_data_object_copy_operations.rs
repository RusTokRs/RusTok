use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable frozen operations, reservations, and namespace holds for actual
/// object migration. Pending publication cannot become age-authorized deletion.
#[derive(DeriveMigrationName)]
pub struct Migration;

const STORAGE_TABLES: [&str; 4] = [
    "module_artifact_data",
    "module_artifact_data_objects",
    "module_artifact_data_indexes",
    "module_artifact_data_index_contracts",
];

// Every guarded write locks roots before reading holds, regardless of the order
// in which PostgreSQL invokes other triggers on the child table.
const NAMESPACE_ROW_LOCKS: &str = "IF TG_OP='INSERT' THEN
    PERFORM 1 FROM module_artifact_data_namespaces WHERE tenant_id=NEW.tenant_id AND data_owner_id=NEW.data_owner_id AND namespace_instance_id=NEW.namespace_instance_id FOR UPDATE;
  ELSIF TG_OP='DELETE' THEN
    PERFORM 1 FROM module_artifact_data_namespaces WHERE tenant_id=OLD.tenant_id AND data_owner_id=OLD.data_owner_id AND namespace_instance_id=OLD.namespace_instance_id FOR UPDATE;
  ELSE
    PERFORM 1 FROM module_artifact_data_namespaces
     WHERE (tenant_id=OLD.tenant_id AND data_owner_id=OLD.data_owner_id AND namespace_instance_id=OLD.namespace_instance_id)
        OR (tenant_id=NEW.tenant_id AND data_owner_id=NEW.data_owner_id AND namespace_instance_id=NEW.namespace_instance_id)
     ORDER BY tenant_id,data_owner_id,namespace_instance_id FOR UPDATE;
  END IF;";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let (id, integer, timestamp) = match backend {
            DbBackend::Postgres => ("UUID", "BIGINT", "TIMESTAMPTZ"),
            DbBackend::Sqlite => ("TEXT", "INTEGER", "TEXT"),
            _ => {
                return Err(DbErr::Migration(
                    "Object migration requires PostgreSQL or SQLite".into(),
                ));
            }
        };
        let digest = |column: &str| match backend {
            DbBackend::Postgres => format!("{column} ~ '^sha256:[0-9a-f]{{64}}$'"),
            _ => format!(
                "length({column})=71 AND substr({column},1,7)='sha256:' AND substr({column},8) NOT GLOB '*[^0-9a-f]*'"
            ),
        };
        let mut statements = vec![
            format!(
                "CREATE TABLE module_artifact_data_object_migration_operations (
                 operation_id {id} PRIMARY KEY NOT NULL,
                 tenant_id {id} NOT NULL, data_owner_id {id} NOT NULL,
                 source_namespace_instance_id {id} NOT NULL, target_namespace_instance_id {id} NOT NULL,
                 idempotency_key {id} NOT NULL,
                 request_digest TEXT NOT NULL CHECK ({}),
                 request_json TEXT NOT NULL,
                 inventory_manifest_digest TEXT NOT NULL CHECK ({}),
                 inventory_json TEXT NOT NULL,
                 status TEXT NOT NULL CHECK (status IN ('preparing','committing','committed')),
                 created_at {timestamp} NOT NULL, committed_at {timestamp} NULL,
                 CHECK (source_namespace_instance_id <> target_namespace_instance_id),
                 CHECK ((status='committed' AND committed_at IS NOT NULL) OR (status<>'committed' AND committed_at IS NULL)),
                 UNIQUE (tenant_id,data_owner_id,idempotency_key),
                 UNIQUE (tenant_id,data_owner_id,target_namespace_instance_id),
                 UNIQUE (operation_id,tenant_id,data_owner_id,source_namespace_instance_id,target_namespace_instance_id,inventory_manifest_digest),
                 FOREIGN KEY (tenant_id,data_owner_id,source_namespace_instance_id)
                   REFERENCES module_artifact_data_namespaces(tenant_id,data_owner_id,namespace_instance_id),
                 FOREIGN KEY (tenant_id,data_owner_id,target_namespace_instance_id)
                   REFERENCES module_artifact_data_namespaces(tenant_id,data_owner_id,namespace_instance_id)
                 )", digest("request_digest"),digest("inventory_manifest_digest")),
            format!(
                "CREATE TABLE module_artifact_data_object_copy_operations (
                 operation_id {id} PRIMARY KEY NOT NULL, migration_operation_id {id} NOT NULL,
                 tenant_id {id} NOT NULL, data_owner_id {id} NOT NULL,
                 source_namespace_instance_id {id} NOT NULL, target_namespace_instance_id {id} NOT NULL,
                 inventory_manifest_digest TEXT NOT NULL CHECK ({}),
                 object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),
                 source_storage_key TEXT NOT NULL CHECK (length(source_storage_key)>0),
                 target_storage_key TEXT NOT NULL UNIQUE CHECK (length(target_storage_key)>0),
                 digest_sha256 TEXT NOT NULL CHECK ({}),
                 size_bytes {integer} NOT NULL CHECK (size_bytes >= 0),
                 status TEXT NOT NULL CHECK (status IN ('intent','checkpointed')),
                 actor_id {id} NOT NULL, trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),
                 correlation_id {id} NOT NULL, idempotency_key {id} NOT NULL,
                 reason TEXT NOT NULL CHECK (length(trim(reason)) BETWEEN 1 AND 2000),
                 created_at {timestamp} NOT NULL, committed_at {timestamp} NULL,
                 CHECK ((status='checkpointed' AND committed_at IS NOT NULL) OR (status='intent' AND committed_at IS NULL)),
                 UNIQUE (migration_operation_id,object_name),
                 FOREIGN KEY (migration_operation_id,tenant_id,data_owner_id,source_namespace_instance_id,target_namespace_instance_id,inventory_manifest_digest)
                   REFERENCES module_artifact_data_object_migration_operations
                   (operation_id,tenant_id,data_owner_id,source_namespace_instance_id,target_namespace_instance_id,inventory_manifest_digest)
                 )",digest("inventory_manifest_digest"),digest("digest_sha256")),
            "CREATE INDEX idx_artifact_data_object_migration_holds ON module_artifact_data_object_migration_operations (tenant_id,data_owner_id,status,source_namespace_instance_id,target_namespace_instance_id)".into(),
        ];
        if backend == DbBackend::Postgres {
            for table in [
                "module_artifact_data_object_migration_operations",
                "module_artifact_data_object_copy_operations",
            ] {
                statements.push(format!("ALTER TABLE {table} ENABLE ROW LEVEL SECURITY"));
                statements.push(format!(
                    "CREATE POLICY {table}_scope ON {table}
                     USING (tenant_id::text=current_setting('rustok.tenant_id',true))
                     WITH CHECK (tenant_id::text=current_setting('rustok.tenant_id',true))"
                ));
            }
        }
        for (table, initial, middle, terminal) in [
            (
                "module_artifact_data_object_migration_operations",
                "preparing",
                Some("committing"),
                "committed",
            ),
            (
                "module_artifact_data_object_copy_operations",
                "intent",
                None,
                "checkpointed",
            ),
        ] {
            let transitions = if let Some(middle) = middle {
                format!(
                    "(OLD.status='{initial}' AND NEW.status='{middle}') OR (OLD.status='{middle}' AND NEW.status='{terminal}')"
                )
            } else {
                format!("OLD.status='{initial}' AND NEW.status='{terminal}'")
            };
            if backend == DbBackend::Postgres {
                statements.push(format!(
                    "CREATE FUNCTION {table}_immutable_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN
                     IF TG_OP='DELETE' THEN RAISE EXCEPTION 'Object migration evidence cannot be deleted' USING ERRCODE='23514'; END IF;
                     IF (to_jsonb(NEW)-'status'-'committed_at') IS DISTINCT FROM (to_jsonb(OLD)-'status'-'committed_at')
                        OR NOT ({transitions}) THEN
                       RAISE EXCEPTION 'Object migration evidence is immutable' USING ERRCODE='23514';
                     END IF; RETURN NEW; END $$"));
                statements.push(format!("CREATE TRIGGER {table}_immutable_guard BEFORE UPDATE OR DELETE ON {table} FOR EACH ROW EXECUTE FUNCTION {table}_immutable_guard()"));
            } else {
                let columns = if middle.is_some() {
                    &[
                        "operation_id",
                        "tenant_id",
                        "data_owner_id",
                        "source_namespace_instance_id",
                        "target_namespace_instance_id",
                        "idempotency_key",
                        "request_digest",
                        "request_json",
                        "inventory_manifest_digest",
                        "inventory_json",
                        "created_at",
                    ][..]
                } else {
                    &[
                        "operation_id",
                        "migration_operation_id",
                        "tenant_id",
                        "data_owner_id",
                        "source_namespace_instance_id",
                        "target_namespace_instance_id",
                        "inventory_manifest_digest",
                        "object_name",
                        "source_storage_key",
                        "target_storage_key",
                        "digest_sha256",
                        "size_bytes",
                        "actor_id",
                        "trace_id",
                        "correlation_id",
                        "idempotency_key",
                        "reason",
                        "created_at",
                    ][..]
                };
                let changed = columns
                    .iter()
                    .map(|column| format!("OLD.{column} IS NOT NEW.{column}"))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                statements.push(format!(
                    "CREATE TRIGGER {table}_immutable_update BEFORE UPDATE ON {table}
                    WHEN ({changed}) OR NOT ({transitions})
                    BEGIN SELECT RAISE(ABORT,'Object migration evidence is immutable'); END"
                ));
                statements.push(format!(
                    "CREATE TRIGGER {table}_immutable_delete BEFORE DELETE ON {table}
                    BEGIN SELECT RAISE(ABORT,'Object migration evidence cannot be deleted'); END"
                ));
            }
        }
        if backend == DbBackend::Postgres {
            statements.push(format!(
                "CREATE FUNCTION artifact_data_migration_root_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN
                 IF (TG_OP='DELETE' OR NEW.state IS DISTINCT FROM OLD.state OR NEW.namespace_revision IS DISTINCT FROM OLD.namespace_revision)
                    AND {} THEN
                   RAISE EXCEPTION 'Object migration namespace is held' USING ERRCODE='23514';
                 END IF;
                 IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF; END $$", root_hold("OLD", backend, true)));
            statements.push("CREATE TRIGGER artifact_data_migration_root_guard BEFORE UPDATE OR DELETE ON module_artifact_data_namespaces FOR EACH ROW EXECUTE FUNCTION artifact_data_migration_root_guard()".into());
        } else {
            statements.push(format!("CREATE TRIGGER artifact_data_migration_root_update BEFORE UPDATE ON module_artifact_data_namespaces
                WHEN (NEW.state IS NOT OLD.state OR NEW.namespace_revision IS NOT OLD.namespace_revision) AND {}
                BEGIN SELECT RAISE(ABORT,'Object migration namespace is held'); END",root_hold("OLD", backend, true)));
            statements.push(format!("CREATE TRIGGER artifact_data_migration_root_delete BEFORE DELETE ON module_artifact_data_namespaces
                WHEN {} BEGIN SELECT RAISE(ABORT,'Object migration namespace is held'); END",root_hold("OLD", backend, true)));
        }
        for table in STORAGE_TABLES {
            if backend == DbBackend::Postgres {
                statements.push(format!(
                    "CREATE FUNCTION {table}_migration_write_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN
                     {NAMESPACE_ROW_LOCKS}
                     IF TG_OP<>'INSERT' AND {} THEN RAISE EXCEPTION 'Object migration namespace is held' USING ERRCODE='23514'; END IF;
                     IF TG_OP<>'DELETE' AND {} THEN RAISE EXCEPTION 'Object migration namespace is held' USING ERRCODE='23514'; END IF;
                     IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF; END $$",write_hold("OLD", backend),write_hold("NEW", backend)));
                statements.push(format!("CREATE TRIGGER {table}_migration_write_guard BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH ROW EXECUTE FUNCTION {table}_migration_write_guard()"));
            } else {
                for (action, predicate) in [
                    ("insert", write_hold("NEW", backend)),
                    (
                        "update",
                        format!(
                            "({}) OR ({})",
                            write_hold("OLD", backend),
                            write_hold("NEW", backend)
                        ),
                    ),
                    ("delete", write_hold("OLD", backend)),
                ] {
                    statements.push(format!("CREATE TRIGGER {table}_migration_{action} BEFORE {action} ON {table}
                        WHEN {predicate} BEGIN SELECT RAISE(ABORT,'Object migration namespace is held'); END"));
                }
            }
        }
        if backend == DbBackend::Postgres {
            statements.push(format!(
                "CREATE FUNCTION artifact_data_migration_reference_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN
                 {NAMESPACE_ROW_LOCKS}
                 IF TG_OP<>'INSERT' AND {} THEN RAISE EXCEPTION 'Object migration namespace is held' USING ERRCODE='23514'; END IF;
                 IF TG_OP<>'DELETE' AND {} THEN RAISE EXCEPTION 'Object migration namespace is held' USING ERRCODE='23514'; END IF;
                 IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF; END $$",root_hold("OLD", backend, false),root_hold("NEW", backend, false)));
            statements.push("CREATE TRIGGER artifact_data_migration_reference_guard BEFORE INSERT OR UPDATE OR DELETE ON module_artifact_data_owner_references FOR EACH ROW EXECUTE FUNCTION artifact_data_migration_reference_guard()".into());
        } else {
            for (action, predicate) in [
                ("insert", root_hold("NEW", backend, false)),
                (
                    "update",
                    format!(
                        "({}) OR ({})",
                        root_hold("OLD", backend, false),
                        root_hold("NEW", backend, false)
                    ),
                ),
                ("delete", root_hold("OLD", backend, false)),
            ] {
                statements.push(format!("CREATE TRIGGER artifact_data_migration_reference_{action} BEFORE {action} ON module_artifact_data_owner_references
                    WHEN {predicate} BEGIN SELECT RAISE(ABORT,'Object migration namespace is held'); END"));
            }
        }
        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(backend, sql))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let mut statements = Vec::new();
        if backend == DbBackend::Postgres {
            statements.push("DROP TRIGGER artifact_data_migration_reference_guard ON module_artifact_data_owner_references".into());
            statements.push("DROP FUNCTION artifact_data_migration_reference_guard()".into());
        } else {
            for action in ["insert", "update", "delete"] {
                statements.push(format!(
                    "DROP TRIGGER artifact_data_migration_reference_{action}"
                ));
            }
        }
        for table in STORAGE_TABLES {
            if backend == DbBackend::Postgres {
                statements.push(format!(
                    "DROP TRIGGER {table}_migration_write_guard ON {table}"
                ));
                statements.push(format!("DROP FUNCTION {table}_migration_write_guard()"));
            } else {
                for action in ["insert", "update", "delete"] {
                    statements.push(format!("DROP TRIGGER {table}_migration_{action}"));
                }
            }
        }
        if backend == DbBackend::Postgres {
            statements.push("DROP TRIGGER artifact_data_migration_root_guard ON module_artifact_data_namespaces".into());
            statements.push("DROP FUNCTION artifact_data_migration_root_guard()".into());
        } else {
            statements.push("DROP TRIGGER artifact_data_migration_root_update".into());
            statements.push("DROP TRIGGER artifact_data_migration_root_delete".into());
        }
        for table in [
            "module_artifact_data_object_copy_operations",
            "module_artifact_data_object_migration_operations",
        ] {
            if backend == DbBackend::Postgres {
                statements.push(format!("DROP TABLE {table}"));
                statements.push(format!("DROP FUNCTION {table}_immutable_guard()"));
            } else {
                statements.push(format!("DROP TABLE {table}"));
            }
        }
        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(backend, sql))
                .await?;
        }
        Ok(())
    }
}

fn root_hold(row: &str, backend: DbBackend, allow_committing_record_target: bool) -> String {
    let record = super::m20260903_000047_artifact_data_copy_operations::record_copy_hold(
        backend,
        row,
        allow_committing_record_target,
        None,
    );
    format!("(EXISTS (SELECT 1 FROM module_artifact_data_object_migration_operations operation
        WHERE operation.tenant_id={row}.tenant_id AND operation.data_owner_id={row}.data_owner_id
         AND operation.status IN ('preparing','committing')
         AND (operation.source_namespace_instance_id={row}.namespace_instance_id OR operation.target_namespace_instance_id={row}.namespace_instance_id)) OR {record})")
}

fn write_hold(row: &str, backend: DbBackend) -> String {
    let record = super::m20260903_000047_artifact_data_copy_operations::record_copy_hold(
        backend, row, true, None,
    );
    format!("(EXISTS (SELECT 1 FROM module_artifact_data_object_migration_operations operation
        WHERE operation.tenant_id={row}.tenant_id AND operation.data_owner_id={row}.data_owner_id
         AND ((operation.source_namespace_instance_id={row}.namespace_instance_id AND operation.status IN ('preparing','committing'))
          OR (operation.target_namespace_instance_id={row}.namespace_instance_id AND operation.status='preparing'))) OR {record})")
}
