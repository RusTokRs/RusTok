use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Records whether the RBAC permission vocabulary declared by an admitted
/// artifact has been projected for its installation scope.
///
/// Admission and permission projection cross different owner boundaries, so
/// the projection cannot be hidden behind the admission result. A fresh
/// admission with declared permissions starts as `pending`; an admission
/// without declared permissions is `not_required`; only a successful RBAC
/// projection advances the state to `registered`. Existing admissions with
/// permissions are conservatively backfilled as `pending` because the prior
/// schema did not record whether the trailing projection completed.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "ALTER TABLE module_artifact_admissions \
                 ADD COLUMN permission_registration_status TEXT NOT NULL DEFAULT 'pending' \
                 CHECK (permission_registration_status IN ('not_required', 'pending', 'registered'))",
                "UPDATE module_artifact_admissions admission \
                 SET permission_registration_status = 'not_required' \
                 FROM module_artifact_installations installation \
                 WHERE installation.installation_id = admission.installation_id \
                   AND COALESCE(jsonb_typeof(installation.descriptor->'permissions'), 'null') <> 'array' \
                    OR (installation.installation_id = admission.installation_id \
                        AND jsonb_typeof(installation.descriptor->'permissions') = 'array' \
                        AND jsonb_array_length(installation.descriptor->'permissions') = 0)",
            ],
            SupportedBackend::Sqlite => &[
                "ALTER TABLE module_artifact_admissions \
                 ADD COLUMN permission_registration_status TEXT NOT NULL DEFAULT 'pending' \
                 CHECK (permission_registration_status IN ('not_required', 'pending', 'registered'))",
                "UPDATE module_artifact_admissions \
                 SET permission_registration_status = 'not_required' \
                 WHERE EXISTS (\
                    SELECT 1 FROM module_artifact_installations installation \
                    WHERE installation.installation_id = module_artifact_admissions.installation_id \
                      AND COALESCE(json_array_length(json_extract(installation.descriptor, '$.permissions')), 0) = 0\
                 )",
            ],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                "ALTER TABLE module_artifact_admissions DROP COLUMN permission_registration_status"
                    .to_string(),
            ))
            .await?;
        Ok(())
    }
}
