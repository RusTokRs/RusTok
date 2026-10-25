use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use crate::{
    AuthError, AuthLifecycleMutationError, DEFAULT_MIN_PASSWORD_LENGTH_CHARS, hash_password,
    normalize_display_name, normalize_email, validate_password_policy,
};

fn bootstrap_validation_error(error: AuthError) -> AuthLifecycleMutationError {
    match error {
        AuthError::Validation(message) => AuthLifecycleMutationError::Validation(message),
        other => internal_bootstrap_error(other),
    }
}

fn internal_bootstrap_error<E>(error: E) -> AuthLifecycleMutationError
where
    E: std::fmt::Display,
{
    tracing::error!(
        error = %error,
        "Auth user bootstrap operation failed"
    );
    AuthLifecycleMutationError::Internal("Auth user bootstrap operation failed".to_string())
}

fn normalize_bootstrap_email(email: &str) -> String {
    email.trim().to_lowercase()
}

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
        if request.tenant_id.is_nil() {
            return Err(AuthLifecycleMutationError::Validation(
                "tenant_id must not be nil for user bootstrap".to_string(),
            ));
        }
        let email = normalize_email(&request.email).map_err(bootstrap_validation_error)?;
        let name = normalize_display_name(Some(&request.name))
            .map_err(bootstrap_validation_error)?
            .ok_or_else(|| {
                AuthLifecycleMutationError::Validation(
                    "name must not be empty for user bootstrap".to_string(),
                )
            })?;
        // Bootstrap identities (installer/superadmin seeds) are the most
        // privileged accounts in the system; they must satisfy the same
        // password policy as any other new credential.
        validate_password_policy(&request.password, DEFAULT_MIN_PASSWORD_LENGTH_CHARS)
            .map_err(bootstrap_validation_error)?;
        tracing::debug!(tenant_id = %request.tenant_id, "Ensuring auth user bootstrap identity");
        if let Some(existing) = Self::find_user_on(db, request.tenant_id, &email).await? {
            return Ok(existing);
        }

        let password_hash = hash_password(&request.password).map_err(internal_bootstrap_error)?;
        let user_id = rustok_core::generate_id();
        let sql = match backend {
            DbBackend::Sqlite => {
                "INSERT INTO users (id, tenant_id, email, password_hash, name) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (tenant_id, email) DO NOTHING"
            }
            DbBackend::Postgres => {
                "INSERT INTO users (id, tenant_id, email, password_hash, name) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (tenant_id, email) DO NOTHING"
            }
            DbBackend::MySql => {
                return Err(AuthLifecycleMutationError::Internal(
                    "auth user bootstrap does not support mysql".to_string(),
                ));
            }
            _ => {
                return Err(AuthLifecycleMutationError::Internal(
                    "auth user bootstrap does not support this database backend".to_string(),
                ));
            }
        };
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    user_id.into(),
                    request.tenant_id.into(),
                    email.clone().into(),
                    password_hash.into(),
                    name.to_string().into(),
                ],
            ))
            .await
            .map_err(internal_bootstrap_error)?;

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
        if tenant_id.is_nil() || email.trim().is_empty() {
            return Ok(None);
        }
        tracing::debug!(tenant_id = %tenant_id, "Looking up bootstrap auth user");
        let email = normalize_bootstrap_email(email);
        let sql = match backend {
            DbBackend::Sqlite => {
                "SELECT id, email FROM users WHERE tenant_id = ?1 AND email = ?2 LIMIT 1"
            }
            DbBackend::Postgres => {
                "SELECT id, email FROM users WHERE tenant_id = $1 AND email = $2 LIMIT 1"
            }
            DbBackend::MySql => {
                return Err(AuthLifecycleMutationError::Internal(
                    "auth user bootstrap does not support mysql".to_string(),
                ));
            }
            _ => {
                return Err(AuthLifecycleMutationError::Internal(
                    "auth user bootstrap does not support this database backend".to_string(),
                ));
            }
        };
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into(), email.into()],
            ))
            .await
            .map_err(internal_bootstrap_error)?;

        row.map(|row| {
            Ok(AuthUserBootstrapRecord {
                id: row.try_get("", "id").map_err(internal_bootstrap_error)?,
                email: row.try_get("", "email").map_err(internal_bootstrap_error)?,
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
        _ => Err(AuthLifecycleMutationError::Internal(
            "auth user bootstrap does not support this database backend".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_supported_backend;
    use crate::AuthLifecycleMutationError;
    use sea_orm::DbBackend;

    #[test]
    fn bootstrap_email_lookup_is_case_normalized() {
        assert_eq!(
            super::normalize_bootstrap_email("Admin@Example.COM"),
            "admin@example.com"
        );
        assert_eq!(
            super::normalize_bootstrap_email("   Admin@Example.COM  \n"),
            "admin@example.com"
        );
    }

    #[test]
    fn internal_bootstrap_errors_are_redacted() {
        let error = super::internal_bootstrap_error("database password leaked");

        assert!(matches!(
            error,
            AuthLifecycleMutationError::Internal(message)
                if message == "Auth user bootstrap operation failed"
        ));
    }

    #[test]
    fn unsupported_backend_is_rejected_without_panicking() {
        assert!(ensure_supported_backend(DbBackend::MySql).is_err());
    }

    #[test]
    fn unsupported_mysql_backend_is_rejected_before_bootstrap_queries() {
        assert!(ensure_supported_backend(DbBackend::Postgres).is_ok());
        assert!(ensure_supported_backend(DbBackend::Sqlite).is_ok());
        assert!(ensure_supported_backend(DbBackend::MySql).is_err());
    }

    #[tokio::test]
    async fn bootstrap_user_on_rejects_nil_tenant_and_empty_fields() {
        use super::{AuthUserBootstrapDbWriter, AuthUserBootstrapRequest};
        use sea_orm::Database;
        use uuid::Uuid;

        let db = Database::connect("sqlite::memory:").await.unwrap();

        let req_nil_tenant = AuthUserBootstrapRequest {
            tenant_id: Uuid::nil(),
            email: "admin@example.com".to_string(),
            name: "Admin".to_string(),
            password: "password123".to_string(),
        };
        assert!(AuthUserBootstrapDbWriter::ensure_user_on(&db, req_nil_tenant).await.is_err());

        let req_empty_email = AuthUserBootstrapRequest {
            tenant_id: Uuid::new_v4(),
            email: "   ".to_string(),
            name: "Admin".to_string(),
            password: "password123".to_string(),
        };
        assert!(AuthUserBootstrapDbWriter::ensure_user_on(&db, req_empty_email).await.is_err());

        let req_empty_password = AuthUserBootstrapRequest {
            tenant_id: Uuid::new_v4(),
            email: "admin@example.com".to_string(),
            name: "Admin".to_string(),
            password: "".to_string(),
        };
        assert!(AuthUserBootstrapDbWriter::ensure_user_on(&db, req_empty_password).await.is_err());

        let req_empty_name = AuthUserBootstrapRequest {
            tenant_id: Uuid::new_v4(),
            email: "admin@example.com".to_string(),
            name: "   ".to_string(),
            password: "password123".to_string(),
        };
        assert!(AuthUserBootstrapDbWriter::ensure_user_on(&db, req_empty_name).await.is_err());

        let req_invalid_email = AuthUserBootstrapRequest {
            tenant_id: Uuid::new_v4(),
            email: "not-an-email".to_string(),
            name: "Admin".to_string(),
            password: "long-enough-password".to_string(),
        };
        assert!(matches!(
            AuthUserBootstrapDbWriter::ensure_user_on(&db, req_invalid_email).await,
            Err(AuthLifecycleMutationError::Validation(_))
        ));

        let req_weak_password = AuthUserBootstrapRequest {
            tenant_id: Uuid::new_v4(),
            email: "admin@example.com".to_string(),
            name: "Admin".to_string(),
            password: "short".to_string(),
        };
        assert!(matches!(
            AuthUserBootstrapDbWriter::ensure_user_on(&db, req_weak_password).await,
            Err(AuthLifecycleMutationError::Validation(_))
        ));

        // find_user_on with nil tenant or empty email returns Ok(None)
        assert_eq!(
            AuthUserBootstrapDbWriter::find_user_on(&db, Uuid::nil(), "admin@example.com").await.unwrap(),
            None
        );
        assert_eq!(
            AuthUserBootstrapDbWriter::find_user_on(&db, Uuid::new_v4(), "   ").await.unwrap(),
            None
        );
    }
}
