use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

use crate::{
    AuthLifecycleMutationError, AuthUserBackfillReadPort, AuthUserBackfillReadRequest,
    AuthUserBackfillRecord,
};

/// Database-backed adapter for the auth-owned, bounded profile-provisioning
/// identity projection. It is usable by the standalone CLI and does not expose
/// user persistence models to consumer modules.
pub struct AuthUserBackfillDbReader {
    db: DatabaseConnection,
}

impl AuthUserBackfillDbReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AuthUserBackfillReadPort for AuthUserBackfillDbReader {
    async fn list_users_for_profile_backfill(
        &self,
        request: AuthUserBackfillReadRequest,
    ) -> Result<Vec<AuthUserBackfillRecord>, AuthLifecycleMutationError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => {
                "SELECT id, email, name FROM users WHERE tenant_id = ?1 ORDER BY created_at ASC LIMIT ?2"
            }
            _ => {
                "SELECT id, email, name FROM users WHERE tenant_id = $1 ORDER BY created_at ASC LIMIT $2"
            }
        };
        let statement = Statement::from_sql_and_values(
            backend,
            sql,
            vec![request.tenant_id.into(), (request.limit as i64).into()],
        );

        self.db
            .query_all(statement)
            .await
            .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?
            .into_iter()
            .map(|row| {
                Ok(AuthUserBackfillRecord {
                    id: row
                        .try_get("", "id")
                        .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?,
                    email: row
                        .try_get("", "email")
                        .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?,
                    name: row
                        .try_get("", "name")
                        .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?,
                })
            })
            .collect()
    }
}
