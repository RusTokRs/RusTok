//! Deployment-bound service credentials, canonical authentication and fresh effective claims.
//! This adapter knows no Inventory tables, source revisions, job ownership or event actors.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use async_trait::async_trait;
use rustok_api::{AuthContext, AuthPrincipalKind, PortContext, PortError};
use rustok_auth::worker_context::{
    WorkerServiceContextPort, WorkerServiceContextRequest, WorkerServiceContextRuntime,
};
use rustok_secrets::{
    EnvResolver, ExposeSecret, MountedFileResolver, SecretAccessPolicy, SecretRef,
    SecretResolverRegistry,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthConfig,
    error::{Error, Result},
    extractors::auth::{CurrentUser, resolve_current_user_from_access_token},
    services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext},
};

const BINDINGS_ENV: &str = "RUSTOK_WORKER_SERVICE_TOKENS_JSON";
const MOUNT_ROOT_ENV: &str = "RUSTOK_WORKER_SERVICE_SECRET_MOUNT_ROOT";
const MAX_BINDINGS: usize = 256;
const MAX_CONFIG_BYTES: usize = 256 * 1024;
const MAX_TOKEN_BYTES: usize = 16 * 1024;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialBinding {
    tenant_id: Uuid,
    consumer: String,
    #[serde(deserialize_with = "deserialize_secret_reference")]
    access_token: SecretRef,
}

// Configuration is an exact reference DTO, never an inline credential container. Canonical
// SecretRef intentionally accepts broader serde input; this deployment boundary must not.
fn deserialize_secret_reference<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<SecretRef, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Reference {
        resolver: String,
        key: String,
    }
    let Reference { resolver, key } = Reference::deserialize(deserializer)?;
    Ok(SecretRef { resolver, key })
}

#[derive(Clone)]
struct ServerWorkerServiceContext {
    auth: ServerAuthRuntime,
    bindings: Arc<BTreeMap<(Uuid, String), SecretRef>>,
    secrets: SecretResolverRegistry,
}

pub(super) fn materialize_worker_service_context(
    server: &ServerRuntimeContext,
    auth_config: &AuthConfig,
) -> Result<WorkerServiceContextRuntime> {
    let bindings = match std::env::var_os(BINDINGS_ENV) {
        Some(serialized) => {
            let serialized = serialized
                .into_string()
                .map_err(|_| configuration_error())?;
            parse_bindings(&serialized)?
        }
        None => Vec::new(),
    };
    let secrets = match server.shared_get::<SecretResolverRegistry>() {
        Some(secrets) => secrets,
        None => local_secret_registry(&bindings)?,
    };
    for binding in &bindings {
        secrets
            .validate_reference_for_tenant(binding.tenant_id, &binding.access_token)
            .map_err(|_| configuration_error())?;
    }
    Ok(WorkerServiceContextRuntime::new(Arc::new(
        ServerWorkerServiceContext {
            auth: ServerAuthRuntime::new(server.clone(), auth_config.clone()),
            bindings: Arc::new(
                bindings
                    .into_iter()
                    .map(|binding| ((binding.tenant_id, binding.consumer), binding.access_token))
                    .collect(),
            ),
            secrets,
        },
    )))
}

fn parse_bindings(raw: &str) -> Result<Vec<CredentialBinding>> {
    if raw.is_empty() || raw.len() > MAX_CONFIG_BYTES {
        return Err(configuration_error());
    }
    let bindings: Vec<CredentialBinding> =
        serde_json::from_str(raw).map_err(|_| configuration_error())?;
    if bindings.is_empty() || bindings.len() > MAX_BINDINGS {
        return Err(configuration_error());
    }
    let mut consumers = BTreeSet::new();
    let mut owners = BTreeMap::new();
    for binding in &bindings {
        WorkerServiceContextRequest::new(
            binding.tenant_id,
            &binding.consumer,
            binding.tenant_id,
            std::time::Duration::from_secs(1),
        )
        .map_err(|_| configuration_error())?;
        let reference = &binding.access_token;
        if !consumers.insert((binding.tenant_id, binding.consumer.clone()))
            || reference.resolver.is_empty()
            || reference.resolver.len() > 128
            || reference.key.is_empty()
            || reference.key.len() > 256
            || reference.key != reference.key.trim()
            || reference.key.chars().any(char::is_control)
        {
            return Err(configuration_error());
        }
        if owners
            .insert(
                (reference.resolver.clone(), reference.key.clone()),
                binding.tenant_id,
            )
            .is_some_and(|owner| owner != binding.tenant_id)
        {
            return Err(configuration_error());
        }
    }
    Ok(bindings)
}

fn local_secret_registry(bindings: &[CredentialBinding]) -> Result<SecretResolverRegistry> {
    let mut keys = BTreeMap::<String, BTreeSet<String>>::new();
    for binding in bindings {
        keys.entry(binding.access_token.resolver.clone())
            .or_default()
            .insert(binding.access_token.key.clone());
    }
    let mut builder = SecretResolverRegistry::builder();
    for (resolver, keys) in keys {
        let policy = SecretAccessPolicy::Exact(keys.into_iter().collect());
        builder = match resolver.as_str() {
            "env" => builder.resolver(resolver, EnvResolver, policy),
            "mounted_file" => {
                let root = std::env::var_os(MOUNT_ROOT_ENV)
                    .map(PathBuf::from)
                    .ok_or_else(configuration_error)?;
                builder.resolver(resolver, MountedFileResolver::new(root), policy)
            }
            _ => return Err(configuration_error()),
        };
    }
    Ok(builder.build())
}

#[async_trait]
impl WorkerServiceContextPort for ServerWorkerServiceContext {
    async fn current_context(
        &self,
        request: WorkerServiceContextRequest,
    ) -> std::result::Result<PortContext, PortError> {
        let started = Instant::now();
        rustok_core::resilience::with_timeout(request.budget(), || async {
            let reference = self.bindings.get(&(request.tenant_id(), request.consumer().to_owned()))
                .ok_or_else(|| PortError::unavailable("auth.worker_service_unconfigured", "Bind a genuine tenant service token through the deployment secret registry"))?;
            // Bypass the shared cache: invalidate-then-resolve can race an older cache fill.
            // Effective grants are also resolved from current persisted authority below.
            let token = self.secrets.resolve_fresh_for_tenant(request.tenant_id(), reference).await
                .map_err(|_| unavailable())?;
            if token.expose_secret().is_empty() || token.expose_secret().len() > MAX_TOKEN_BYTES {
                return Err(unavailable());
            }
            let current = resolve_current_user_from_access_token(&self.auth, request.tenant_id(), token.expose_secret()).await
                .map_err(|(status, _)| {
                    if status.is_server_error() { unavailable() } else { forbidden() }
                })?;
            let availability = crate::services::rbac_module_availability::permission_availability_for(self.auth.runtime_ctx(), request.tenant_id())
                .await.map_err(|_| unavailable())?;
            let permissions = match availability {
                Some(availability) => availability.active_permissions(&current.permissions),
                None => current.permissions.clone(),
            };
            let remaining = request.budget().saturating_sub(started.elapsed());
            current_port_context(&request, &current, permissions, remaining)
        }).await.map_err(|_| PortError::timeout("auth.worker_service_timeout", "Current worker service authentication exceeded its read budget"))?
    }
}

fn current_port_context(
    request: &WorkerServiceContextRequest,
    current: &CurrentUser,
    permissions: Vec<rustok_api::Permission>,
    remaining: std::time::Duration,
) -> std::result::Result<PortContext, PortError> {
    let client_id = current.client_id.ok_or_else(forbidden)?;
    if current.principal_kind != AuthPrincipalKind::Service
        || current.actor_kind != rustok_core::SecurityActorKind::Service
        || current.grant_type != "client_credentials"
        || current.user.tenant_id != request.tenant_id()
        || current.user.id.is_nil()
        || client_id.is_nil()
    {
        return Err(forbidden());
    }
    if remaining.as_millis() == 0 {
        return Err(PortError::timeout(
            "auth.worker_service_timeout",
            "Worker read authentication exhausted its current budget",
        ));
    }
    // Preserve the canonical transport/native principal mapping, not a worker-specific
    // interpretation of the distinct OAuth application subject and public client UUID.
    let auth = AuthContext {
        user_id: current.user.id,
        session_id: current.session_id,
        tenant_id: current.user.tenant_id,
        permissions,
        client_id: Some(client_id),
        scopes: current.scopes.clone(),
        grant_type: current.grant_type.clone(),
    };
    // The consumer is nonlocalized. `und` conveys no locale selection or public-read authority.
    let mut context = PortContext::new(
        request.tenant_id().to_string(),
        auth.port_actor(),
        "und",
        request.correlation_id().to_string(),
    )
    .with_deadline(remaining);
    context.claims = auth
        .permissions
        .into_iter()
        .map(|permission| permission.to_string())
        .collect();
    Ok(context)
}

fn configuration_error() -> Error {
    Error::BadRequest("Worker service credential bindings are invalid; configure exact tenant/consumer secret references".into())
}

fn unavailable() -> PortError {
    PortError::unavailable(
        "auth.worker_service_unavailable",
        "Current service credential or authorization policy is unavailable",
    )
}

fn forbidden() -> PortError {
    PortError::forbidden(
        "auth.worker_service_forbidden",
        "A current authenticated tenant OAuth service principal is required",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(tenant: Uuid, consumer: &str, key: &str) -> serde_json::Value {
        serde_json::json!({"tenant_id": tenant, "consumer": consumer, "access_token": {"resolver": "env", "key": key}})
    }

    #[test]
    fn configuration_rejects_inline_tokens_unknown_fields_and_duplicate_scope() {
        let good = binding(
            Uuid::from_u128(1),
            "inventory-stock-primary",
            "TENANT_STOCK_TOKEN",
        );
        assert!(parse_bindings(&serde_json::json!([good.clone()]).to_string()).is_ok());
        assert!(
            parse_bindings(&serde_json::json!([good.clone(), good.clone()]).to_string()).is_err()
        );
        let mut nested_inline = good.clone();
        nested_inline["access_token"]["token"] = "forbidden-inline-credential".into();
        assert!(parse_bindings(&serde_json::json!([nested_inline]).to_string()).is_err());
        let mut unsafe_binding = good;
        unsafe_binding["token"] = "not-a-secret-reference".into();
        assert!(parse_bindings(&serde_json::json!([unsafe_binding]).to_string()).is_err());
    }

    #[test]
    fn configuration_refuses_cross_tenant_secret_aliasing_and_unbounded_input() {
        assert!(
            parse_bindings(
                &serde_json::json!([
                    binding(
                        Uuid::from_u128(1),
                        "inventory-stock-primary",
                        "SHARED_TOKEN"
                    ),
                    binding(
                        Uuid::from_u128(2),
                        "inventory-stock-primary",
                        "SHARED_TOKEN"
                    ),
                ])
                .to_string()
            )
            .is_err()
        );
        assert!(parse_bindings("[]").is_err());
        assert!(parse_bindings(&" ".repeat(MAX_CONFIG_BYTES + 1)).is_err());
    }

    fn current(principal_kind: AuthPrincipalKind) -> CurrentUser {
        CurrentUser {
            user: crate::models::users::Model::default_service_user(
                Uuid::from_u128(3),
                Uuid::from_u128(1),
            ),
            session_id: Uuid::nil(),
            permissions: vec![rustok_api::Permission::INVENTORY_READ],
            inferred_role: rustok_core::UserRole::Customer,
            actor_kind: rustok_core::SecurityActorKind::Service,
            principal_kind,
            client_id: Some(Uuid::from_u128(4)),
            scopes: vec!["inventory:read".into()],
            grant_type: "client_credentials".into(),
        }
    }

    #[test]
    fn context_conversion_rejects_human_delegated_nil_and_cross_tenant_principals() {
        let request = WorkerServiceContextRequest::new(
            Uuid::from_u128(1),
            "inventory-stock-primary",
            Uuid::from_u128(2),
            std::time::Duration::from_secs(10),
        )
        .unwrap();
        for principal in [
            AuthPrincipalKind::DirectUser,
            AuthPrincipalKind::DelegatedUser,
        ] {
            assert!(
                current_port_context(
                    &request,
                    &current(principal),
                    Vec::new(),
                    std::time::Duration::from_secs(9)
                )
                .is_err()
            );
        }
        let mut wrong = current(AuthPrincipalKind::Service);
        wrong.user.tenant_id = Uuid::from_u128(5);
        assert!(
            current_port_context(
                &request,
                &wrong,
                Vec::new(),
                std::time::Duration::from_secs(9)
            )
            .is_err()
        );
        wrong = current(AuthPrincipalKind::Service);
        wrong.client_id = Some(Uuid::nil());
        assert!(
            current_port_context(
                &request,
                &wrong,
                Vec::new(),
                std::time::Duration::from_secs(9)
            )
            .is_err()
        );
        wrong = current(AuthPrincipalKind::Service);
        wrong.user.id = Uuid::nil();
        assert!(
            current_port_context(
                &request,
                &wrong,
                Vec::new(),
                std::time::Duration::from_secs(9)
            )
            .is_err()
        );
    }

    #[test]
    fn context_contains_only_current_effective_permissions_and_actual_client_identity() {
        let request = WorkerServiceContextRequest::new(
            Uuid::from_u128(1),
            "inventory-stock-primary",
            Uuid::from_u128(2),
            std::time::Duration::from_secs(10),
        )
        .unwrap();
        let principal = current(AuthPrincipalKind::Service);
        let context = current_port_context(
            &request,
            &principal,
            Vec::new(),
            std::time::Duration::from_secs(9),
        )
        .unwrap();
        assert!(
            context.claims.is_empty(),
            "A saved principal permission must not reappear after current filtering"
        );
        assert_ne!(principal.user.id, principal.client_id.unwrap());
        assert_eq!(context.actor.id, principal.client_id.unwrap().to_string());
        assert_ne!(context.actor.id, principal.user.id.to_string());
        assert_eq!(context.deadline_ms, Some(9_000));
        assert!(context.idempotency_key.is_none());
    }

    #[test]
    fn service_actor_matches_canonical_principal_mapping_with_distinct_subject_and_client_id() {
        let request = WorkerServiceContextRequest::new(
            Uuid::from_u128(1),
            "inventory-stock-primary",
            Uuid::from_u128(2),
            std::time::Duration::from_secs(10),
        )
        .unwrap();
        for client_id in [Uuid::from_u128(4), Uuid::from_u128(5)] {
            let mut principal = current(AuthPrincipalKind::Service);
            principal.client_id = Some(client_id);
            let canonical = AuthContext {
                user_id: principal.user.id,
                session_id: principal.session_id,
                tenant_id: principal.user.tenant_id,
                permissions: principal.permissions.clone(),
                client_id: principal.client_id,
                scopes: principal.scopes.clone(),
                grant_type: principal.grant_type.clone(),
            };
            let context = current_port_context(
                &request,
                &principal,
                principal.permissions.clone(),
                std::time::Duration::from_secs(9),
            )
            .unwrap();
            assert_eq!(context.actor, canonical.port_actor());
            assert_eq!(context.actor.id, client_id.to_string());
            assert_ne!(context.actor.id, principal.user.id.to_string());
            assert_eq!(context.tenant_id, request.tenant_id().to_string());
            assert_eq!(context.correlation_id, request.correlation_id().to_string());
            assert_eq!(context.claims, vec!["inventory:read".to_owned()]);
        }
    }
}
