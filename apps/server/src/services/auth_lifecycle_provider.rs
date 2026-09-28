use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use rustok_auth::{
    AcceptInviteRecord, AuthLifecycleContext, AuthLifecycleMutationError, AuthLifecyclePort,
    AuthSessionRecord, AuthTokenRecord, AuthUserBackfillDbReader, AuthUserBackfillReadPort,
    AuthUserBackfillReadRequest, AuthUserBackfillRecord, AuthUserRecord,
};

use crate::auth::{AuthConfig, encode_password_reset_token};
use crate::common::RustokSettings;
use crate::models::users;
use crate::services::auth_invite::InviteAcceptanceError;
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService, AuthTokens};
use crate::services::email::{PasswordResetEmail, email_service_from_ctx, password_reset_url};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

const DEFAULT_RESET_TOKEN_TTL_SECS: u64 = 15 * 60;

pub struct ServerAuthLifecycleProvider {
    runtime_ctx: ServerRuntimeContext,
    auth_config: AuthConfig,
}

impl ServerAuthLifecycleProvider {
    pub fn new(runtime_ctx: ServerRuntimeContext, auth_config: AuthConfig) -> Self {
        Self {
            runtime_ctx,
            auth_config,
        }
    }

    fn ensure_registration_enabled(
        settings: &RustokSettings,
    ) -> Result<(), AuthLifecycleMutationError> {
        if settings.features.registration_enabled {
            Ok(())
        } else {
            Err(AuthLifecycleMutationError::Validation(
                "Registration is disabled".to_string(),
            ))
        }
    }

    async fn permission_strings(
        &self,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<Vec<String>, AuthLifecycleMutationError> {
        let permissions =
            RbacService::get_user_permissions(self.runtime_ctx.db(), &tenant_id, &user_id)
                .await
                .map_err(|err| AuthLifecycleMutationError::Internal(err.to_string()))?;
        let mut values = permissions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        Ok(values)
    }

    async fn token_record(
        &self,
        tenant_id: uuid::Uuid,