use async_graphql::{Context, Error, FieldError, Result};
use rustok_api::graphql::GraphQLError;
use rustok_api::{Action, Resource};
use rustok_api::{AuthContext, PortActor, PortContext, RequestContext};
use rustok_core::{PermissionScope, SecurityContext, infer_user_role_from_permissions};
use uuid::Uuid;

const TRANSLATION_GRAPHQL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

pub(crate) fn read_port_context(ctx: &Context<'_>, operation: &str) -> Result<PortContext> {
    port_context(ctx, operation, None)
}

pub(crate) fn write_port_context(
    ctx: &Context<'_>,
    operation: &str,
    idempotency_key: String,
) -> Result<PortContext> {
    if idempotency_key.trim().is_empty() {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "idempotencyKey must not be empty",
        ));
    }
    port_context(ctx, operation, Some(idempotency_key))
}

fn port_context(
    ctx: &Context<'_>,
    operation: &str,
    idempotency_key: Option<String>,
) -> Result<PortContext> {
    let auth = ctx
        .data_opt::<AuthContext>()
        .ok_or_else(<FieldError as GraphQLError>::unauthenticated)?;
    let request = ctx.data_opt::<RequestContext>().ok_or_else(|| {
        <FieldError as GraphQLError>::internal_error("Request context is unavailable")
    })?;
    if auth.tenant_id != request.tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated tenant does not match request tenant",
        ));
    }

    let correlation_id = format!("translation-graphql-{operation}-{}", Uuid::new_v4());
    let mut context = PortContext::new(
        request.tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request.locale.as_str(),
        correlation_id,
    )
    .with_deadline(TRANSLATION_GRAPHQL_DEADLINE)
    .with_role(infer_user_role_from_permissions(&auth.permissions).to_string());
    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Some(channel) = request.channel_slug.as_deref() {
        context = context.with_channel(channel);
    }
    if let Some(idempotency_key) = idempotency_key {
        context = context.with_idempotency_key(idempotency_key);
    }
    Ok(context)
}

pub(crate) fn runtime<'a>(
    ctx: &'a Context<'_>,
) -> Result<&'a crate::graphql_runtime::TranslationGraphqlRuntimeData> {
    ctx.data::<crate::graphql_runtime::TranslationGraphqlRuntimeData>()
        .map_err(|_| {
            <FieldError as GraphQLError>::internal_error("Translation runtime is unavailable")
        })
}

pub(crate) fn require_translation_permission(context: &PortContext, action: Action) -> Result<()> {
    let security = SecurityContext::try_from_port_context(context)
        .map_err(|error| <FieldError as GraphQLError>::permission_denied(error.message.as_str()))?;
    if security.get_scope(Resource::Translations, action) == PermissionScope::None {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Translation permission denied",
        ));
    }
    Ok(())
}

pub(crate) fn translation_error(error: crate::TranslationError) -> Error {
    let public =
        crate::map_translation_public_error(&error, "graphql_operation", "translation_graphql");
    let message = public.to_string();
    match public.kind {
        crate::TranslationPublicErrorKind::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(&message)
        }
        crate::TranslationPublicErrorKind::NotFound => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        crate::TranslationPublicErrorKind::BadInput => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        crate::TranslationPublicErrorKind::Internal => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::Value;

    use super::translation_error;

    #[test]
    fn internal_failures_do_not_expose_database_details() {
        let error = translation_error(crate::TranslationError::Database(sea_orm::DbErr::Custom(
            "private database detail".to_string(),
        )));

        assert_eq!(
            error.message.split(" (code:").next(),
            Some("Translation service is temporarily unavailable")
        );
        assert_eq!(
            error
                .extensions
                .as_ref()
                .and_then(|extensions| extensions.get("code")),
            Some(&Value::from("INTERNAL_ERROR"))
        );
    }
}
