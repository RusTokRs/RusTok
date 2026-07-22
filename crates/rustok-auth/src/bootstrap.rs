use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use crate::{AuthLifecycleMutationError, hash_password};

/// Narrow identity input for bootstrap and installer workflows.
///
/// Role assignment is deliberately excluded: it belongs to the RBAC owner and
/// is composed separately by the installer seed workflow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthUserBootstrapRequest {
    pub tenant_id: Uuid,
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthUserBootstrapRecord {
    pub id: Uuid,
    pub email: String,
    pub created: bool,
}

/// Database-backed adapter for idempotent auth-user provisioning.
///
/// It keeps user persistence and credential hashing inside the auth owner while
/// allowing installer and standalone CLI composition without server models.
pub struct AuthUserBootstrapDbWriter {
    db: DatabaseConnection,
}

impl AuthUserBootstrapDbWriter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn ensure_user(
        &self,
        request: AuthUserBootstrapRequest,
    ) -> Result<AuthUserBootstrapRecord, AuthLifecycleMutationError> {
        Self::ensure_user_on(&self.db, request).await
    }

    /// Ensure a bootstrap identity on a caller-owned connection or transaction.
    ///
    /// Installer composition uses this API so identity creation and RBAC role
    /// assignment can share one atomic commit boundary.
    pub async fn ensure_user_on<C>(
        db: &C,
        request: AuthUserBootstrapRequest,
    ) -> Result<AuthUserBootstrapRecord, AuthLifecycleMutationError>
    where
        C: ConnectionTrait,
    {
        let backend = db.get_database_backend();
        ensure_supported_backend(backend)?;
        let email = request.email.to_lowercase();
        if let Some(existing) = Self::find_user_on(db, request.tenant_id, &email).await? {
            return Ok(existing);
        }

        let password_hash = hash_password(&request.password)
            .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?;
        let user_id = rustok_core::generate_id();
        let sql = match backend {
            DbBackend::Sqlite => {
                "INSERT INTO users (id, tenant_id, email, password_hash, name) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (tenant_id, email) DO NOTHING"
            }
            DbBackend::Postgres => {
                "INSERT INTO users (id, tenant_id, email, password_hash, name) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (tenant_id, email) DO NOTHING"
            }
            DbBackend::MySql => unreachable!("unsupported backend rejected before SQL rendering"),
        };
        let result = db
            .execute(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    user_id.into(),
                    request.tenant_id.into(),
                    email.clone().into(),
                    password_hash.into(),
                    request.name.into(),
                ],
            ))
            .await
            .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?;

        if result.rows_affected() == 1 {
            return Ok(AuthUserBootstrapRecord {
                id: user_id,
                email,
                created: true,
            });
        }

        Self::find_user_on(db, request.tenant_id, &email)
            .await?
            .ok_or_else(|| {
                AuthLifecycleMutationError::Internal(
                    "user bootstrap insert completed without a persisted identity".to_string(),
                )
            })
    }

    /// Reads an existing bootstrap identity without creating or updating it.
    ///
    /// Installer verification uses this owner-owned lookup instead of reaching
    /// into host user entities.
    pub async fn find_user(
        &self,
        tenant_id: Uuid,
        email: &str,
    ) -> Result<Option<AuthUserBootstrapRecord>, AuthLifecycleMutationError> {
        Self::find_user_on(&self.db, tenant_id, email).await
    }

    /// Read a bootstrap identity on a caller-owned connection or transaction.
    pub async fn find_user_on<C>(
        db: &C,
        tenant_id: Uuid,
        email: &str,
    ) -> Result<Option<AuthUserBootstrapRecord>, AuthLifecycleMutationError>
    where
        C: ConnectionTrait,
    {
        let backend = db.get_database_backend();
        ensure_supported_backend(backend)?;
        let sql = match backend {
            DbBackend::Sqlite => {
                "SELECT id, email FROM users WHERE tenant_id = ?1 AND email = ?2 LIMIT 1"
            }
            DbBackend::Postgres => {
                "SELECT id, email FROM users WHERE tenant_id = $1 AND email = $2 LIMIT 1"
            }
            DbBackend::MySql => unreachable!("unsupported backend rejected before SQL rendering"),
        };
        let row = db
            .query_one(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into(), email.into()],
            ))
            .await
            .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?;

        row.map(|row| {
            Ok(AuthUserBootstrapRecord {
                id: row
                    .try_get("", "id")
                    .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?,
                email: row
                    .try_get("", "email")
                    .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?,
                created: false,
            })
        })
        .transpose()
    }
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), AuthLifecycleMutationError> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        DbBackend::MySql => Err(AuthLifecycleMutationError::Internal(
            "auth user bootstrap does not support mysql".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_supported_backend;
    use sea_orm::DbBackend;

    #[test]
    fn unsupported_mysql_backend_is_rejected_before_bootstrap_queries() {
        assert!(ensure_supported_backend(DbBackend::Postgres).is_ok());
        assert!(ensure_supported_backend(DbBackend::Sqlite).is_ok());
        assert!(ensure_supported_backend(DbBackend::MySql).is_err());
    }
}
