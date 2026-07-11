use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TryGetable};
use uuid::Uuid;

use crate::{hash_password, AuthLifecycleMutationError};

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
        let email = request.email.to_lowercase();
        if let Some(existing) = self.find_user(request.tenant_id, &email).await? {
            return Ok(existing);
        }

        let password_hash = hash_password(&request.password)
            .map_err(|error| AuthLifecycleMutationError::Internal(error.to_string()))?;
        let user_id = rustok_core::generate_id();
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => {
                "INSERT INTO users (id, tenant_id, email, password_hash, name) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (tenant_id, email) DO NOTHING"
            }
            _ => {
                "INSERT INTO users (id, tenant_id, email, password_hash, name) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (tenant_id, email) DO NOTHING"
            }
        };
        let result = self
            .db
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

        self.find_user(request.tenant_id, &email)
            .await?
            .ok_or_else(|| {
                AuthLifecycleMutationError::Internal(
                    "user bootstrap insert completed without a persisted identity".to_string(),
                )
            })
    }

    async fn find_user(
        &self,
        tenant_id: Uuid,
        email: &str,
    ) -> Result<Option<AuthUserBootstrapRecord>, AuthLifecycleMutationError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => {
                "SELECT id, email FROM users WHERE tenant_id = ?1 AND email = ?2 LIMIT 1"
            }
            _ => "SELECT id, email FROM users WHERE tenant_id = $1 AND email = $2 LIMIT 1",
        };
        let row = self
            .db
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
