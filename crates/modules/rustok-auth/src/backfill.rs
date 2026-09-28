use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

use crate::{
    AuthLifecycleMutationError, AuthUserBackfillReadPort, AuthUserBackfillReadRequest,
    AuthUserBackfillRecord,
};

const MIN_BACKFILL_USER_READ_LIMIT: u64 = 1;
const MAX_BACKFILL_USER_READ_LIMIT: u64 = 500;

fn internal_backfill_error<E>(error: E) -> AuthLifecycleMutationError
where
    E: std::fmt::Display,
{
    tracing::error!(
        error = %error,
        "Auth user backfill read failed"
    );
    AuthLifecycleMutationError::Internal("Auth user backfill read failed".to_string())
}


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

fn validate_backfill_user_read_limit(
    limit: u64,
) -> Result<u64, AuthLifecycleMutationError> {
    if !(MIN_BACKFILL_USER_READ_LIMIT..=MAX_BACKFILL_USER_READ_LIMIT).contains(&limit) {
        return Err(AuthLifecycleMutationError::Validation(format!(
            "profile backfill user read limit must be between {MIN_BACKFILL_USER_READ_LIMIT} and {MAX_BACKFILL_USER_READ_LIMIT}"
        )));
    }

    Ok(limit)
}

#[async_trait]
impl AuthUserBackfillReadPort for AuthUserBackfillDbReader {
    async fn list_users_for_profile_backfill(
        &self,
        request: AuthUserBackfillReadRequest,
    ) -> Result<Vec<AuthUserBackfillRecord>, AuthLifecycleMutationError> {
        let limit = validate_backfill_user_read_limit(request.limit)?;
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => {
                "SELECT id, email, name FROM users WHERE tenant_id = ?1 ORDER BY created_at ASC, id ASC LIMIT ?2"
            }
            _ => {
                "SELECT id, email, name FROM users WHERE tenant_id = $1 ORDER BY created_at ASC, id ASC LIMIT $2"
            }
        };
        let statement = Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                request.tenant_id.into(),
                i64::try_from(limit)
                    .map_err(|_| AuthLifecycleMutationError::Validation(
                        "profile backfill user read limit is out of range".to_string(),
                    ))?
                    .into(),
            ],
        );

        self.db
            .query_all_raw(statement)
            .await
            .map_err(internal_backfill_error)?
            .into_iter()
            .map(|row| {
                Ok(AuthUserBackfillRecord {
                    id: row
                        .try_get("", "id")
                        .map_err(internal_backfill_error)?,
                    email: row
                        .try_get("", "email")
                        .map_err(internal_backfill_error)?,
                    name: row
                        .try_get("", "name")
                        .map_err(internal_backfill_error)?,
                })
            })
            .collect()
    }
}


#[cfg(test)]
mod tests {
    use super::validate_backfill_user_read_limit;

    #[test]
    fn internal_backfill_errors_are_redacted() {
        let error = super::internal_backfill_error("database password leaked");

        assert!(matches!(
            error,
            AuthLifecycleMutationError::Internal(message)
                if message == "Auth user backfill read failed"
        ));
    }

    #[test]
    fn backfill_user_read_limit_is_bounded() {
        assert!(validate_backfill_user_read_limit(0).is_err());
        assert_eq!(validate_backfill_user_read_limit(1).unwrap(), 1);
        assert_eq!(validate_backfill_user_read_limit(500).unwrap(), 500);
        assert!(validate_backfill_user_read_limit(501).is_err());
        assert!(validate_backfill_user_read_limit(u64::MAX).is_err());
    }
}
