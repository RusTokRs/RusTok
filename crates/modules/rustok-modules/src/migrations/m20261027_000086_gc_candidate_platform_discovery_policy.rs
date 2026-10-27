use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Lets the platform maintenance worker discover which tenants hold
/// queued data-object GC candidates. The original policy
/// (`000027`) is tenant-only, so a platform-operated inventory scan
/// would see zero rows on PostgreSQL and tenant sweeps would never
/// start. The recreated policy keeps the tenant predicate for
/// tenant-scoped sessions and adds the same
/// `rustok.module_control_plane_owner = 'platform'` bypass the
/// installations policy has carried since `000001`; the bypass is set
/// transaction-locally by the control plane only.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        if backend == SupportedBackend::Postgres {
            for statement in [
                "DROP POLICY IF EXISTS module_artifact_data_object_gc_candidates_scope \
                 ON module_artifact_data_object_gc_candidates",
                "CREATE POLICY module_artifact_data_object_gc_candidates_scope \
                 ON module_artifact_data_object_gc_candidates \
                 USING (current_setting('rustok.module_control_plane_owner', true) = 'platform' \
                     OR tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (current_setting('rustok.module_control_plane_owner', true) = 'platform' \
                     OR tenant_id::text = current_setting('rustok.tenant_id', true))",
            ] {
                manager
                    .get_connection()
                    .execute_raw(Statement::from_string(
                        backend.db_backend(),
                        statement.to_string(),
                    ))
                    .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        if backend == SupportedBackend::Postgres {
            for statement in [
                "DROP POLICY IF EXISTS module_artifact_data_object_gc_candidates_scope \
                 ON module_artifact_data_object_gc_candidates",
                "CREATE POLICY module_artifact_data_object_gc_candidates_scope \
                 ON module_artifact_data_object_gc_candidates \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ] {
                manager
                    .get_connection()
                    .execute_raw(Statement::from_string(
                        backend.db_backend(),
                        statement.to_string(),
                    ))
                    .await?;
            }
        }
        Ok(())
    }
}
