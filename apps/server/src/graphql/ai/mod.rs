pub mod mutation;
pub mod query;
pub mod types;

use async_graphql::{FieldError, Result};

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use rustok_core::Permission;
use rustok_rbac::has_effective_permission_in_set;

fn ensure_permission(auth: &AuthContext, permission: &Permission, message: &str) -> Result<()> {
    if has_effective_permission_in_set(&auth.permissions, permission) {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(message))
    }
}

pub(super) fn ensure_ai_provider_read(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_PROVIDERS_READ,
        "Permission denied: ai:providers:read required",
    )
}

pub(super) fn ensure_ai_provider_manage(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_PROVIDERS_MANAGE,
        "Permission denied: ai:providers:manage required",
    )
}

pub(super) fn ensure_ai_task_profile_read(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_TASK_PROFILES_READ,
        "Permission denied: ai:task_profiles:read required",
    )
}

pub(super) fn ensure_ai_task_profile_manage(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_TASK_PROFILES_MANAGE,
        "Permission denied: ai:task_profiles:manage required",
    )
}

pub(super) fn ensure_ai_session_read(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_SESSIONS_READ,
        "Permission denied: ai:sessions:read required",
    )
}

pub(super) fn ensure_ai_session_run(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_SESSIONS_RUN,
        "Permission denied: ai:sessions:run required",
    )
}

pub(super) fn ensure_ai_run_cancel(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_RUNS_CANCEL,
        "Permission denied: ai:runs:cancel required",
    )
}

pub(super) fn ensure_ai_approval_resolve(auth: &AuthContext) -> Result<()> {
    ensure_permission(
        auth,
        &Permission::AI_APPROVALS_RESOLVE,
        "Permission denied: ai:approvals:resolve required",
    )
}

pub(super) fn ensure_ai_overview_read(auth: &AuthContext) -> Result<()> {
    if has_effective_permission_in_set(&auth.permissions, &Permission::AI_SESSIONS_READ)
        || has_effective_permission_in_set(&auth.permissions, &Permission::AI_PROVIDERS_READ)
        || has_effective_permission_in_set(&auth.permissions, &Permission::AI_TASK_PROFILES_READ)
    {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: AI read permissions required",
        ))
    }
}

pub use mutation::AiMutation;
pub use query::AiQuery;
