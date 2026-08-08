use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_FORUM_PAGE_BUILDER_ATTESTATION_CHALLENGE_BYTES: usize = 128;
const FORUM_PAGE_BUILDER_ATTESTATION_CONTRACT: &str =
    "forum_page_builder_server_fn_attestation_v1";
const FORUM_PAGE_BUILDER_PREVIEW_ENDPOINT: &str = "/api/fn/forum/page-builder-widget-preview";
const FORUM_PAGE_BUILDER_PROPERTY_SCHEMA_ENDPOINT: &str =
    "/api/fn/forum/page-builder-widget-property-schema";
const FORUM_PAGE_BUILDER_PROPERTY_VALIDATE_ENDPOINT: &str =
    "/api/fn/forum/page-builder-widget-property-validate";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumWidgetPreviewTransportRequest {
    pub widget_type: String,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumPageBuilderTransportAttestationResponse {
    pub challenge: String,
    pub contract: String,
    pub source_commit: Option<String>,
    pub module_id: String,
    pub owner_provider: String,
    pub owner_version: String,
    pub catalog_version: String,
    pub builder_contract_version: String,
    pub widget_types: Vec<String>,
    pub preview_endpoint: String,
    pub property_schema_endpoint: String,
    pub property_validate_endpoint: String,
}

#[cfg(feature = "ssr")]
pub(crate) fn require_tenant_scope(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    if auth.tenant_id == tenant.id {
        Ok(())
    } else {
        Err(ServerFnError::new("Forum Page Builder tenant scope mismatch"))
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn require_forum_transport_authorization(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    require_tenant_scope(auth, tenant)?;
    if rustok_api::has_any_effective_permission(
        &auth.permissions,
        &[rustok_api::Permission::FORUM_TOPICS_READ],
    ) {
        Ok(())
    } else {
        Err(ServerFnError::new("forum_topics:read required"))
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn require_forum_module_state(
    state: Result<bool, impl std::fmt::Display>,
) -> Result<(), ServerFnError> {
    match state {
        Ok(true) => Ok(()),
        Ok(false) => Err(ServerFnError::new("Forum module is not enabled")),
        Err(error) => Err(ServerFnError::new(format!(
            "Forum module state is unavailable: {error}"
        ))),
    }
}

#[cfg(feature = "ssr")]
pub(crate) async fn require_forum_module_enabled(
    host: &rustok_api::HostRuntimeContext,
    tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    require_forum_module_state(
        rustok_api::is_tenant_module_enabled(host.db(), tenant_id, "forum").await,
    )
}

#[cfg(feature = "ssr")]
fn runtime() -> Result<
    (
        rustok_api::HostRuntimeContext,
        rustok_outbox::TransactionalEventBus,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;

    let host = expect_context::<rustok_api::HostRuntimeContext>();
    let event_bus = host
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            ServerFnError::new(
                "Forum widget preview requires TransactionalEventBus in host runtime context",
            )
        })?;
    Ok((host, event_bus))
}

#[cfg(feature = "ssr")]
fn validate_attestation_challenge(challenge: &str) -> Result<(), ServerFnError> {
    let bytes = challenge.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_FORUM_PAGE_BUILDER_ATTESTATION_CHALLENGE_BYTES {
        return Err(ServerFnError::new(
            "Forum Page Builder attestation challenge is outside the bounded size",
        ));
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ServerFnError::new(
            "Forum Page Builder attestation challenge contains unsupported characters",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn deployed_source_commit() -> Option<String> {
    let value = std::env::var("RUSTOK_SOURCE_COMMIT").ok()?;
    let value = value.trim();
    (value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

#[cfg(feature = "ssr")]
fn build_transport_attestation(
    challenge: String,
) -> ForumPageBuilderTransportAttestationResponse {
    let manifest = crate::forum_contribution_manifest();
    let catalog = rustok_forum::ForumWidgetContractService::catalog();
    let mut widget_types = catalog
        .items
        .iter()
        .map(|item| item.widget_type.clone())
        .collect::<Vec<_>>();
    widget_types.sort();

    ForumPageBuilderTransportAttestationResponse {
        challenge,
        contract: FORUM_PAGE_BUILDER_ATTESTATION_CONTRACT.to_string(),
        source_commit: deployed_source_commit(),
        module_id: manifest.module_id,
        owner_provider: manifest.owner_provider,
        owner_version: manifest.owner_version,
        catalog_version: catalog.catalog_version,
        builder_contract_version: catalog.builder_contract_version,
        widget_types,
        preview_endpoint: FORUM_PAGE_BUILDER_PREVIEW_ENDPOINT.to_string(),
        property_schema_endpoint: FORUM_PAGE_BUILDER_PROPERTY_SCHEMA_ENDPOINT.to_string(),
        property_validate_endpoint: FORUM_PAGE_BUILDER_PROPERTY_VALIDATE_ENDPOINT.to_string(),
    }
}

/// Read-only deployed-transport probe for Page Builder evidence.
///
/// A successful response proves that the request crossed the real Leptos `/api/fn` dispatcher,
/// tenant/auth middleware, the shared effective `forum_topics:read` gate, exact tenant-module
/// enablement, the Forum admin host runtime (including its transactional event bus), and the
/// Forum-owned widget contract catalog. It returns only stable contract identity plus the caller's
/// bounded challenge and canonical runtime source revision when the production image supplied one;
/// it never reads or returns Forum topic/reply data.
#[server(prefix = "/api/fn", endpoint = "forum/page-builder-transport-attestation")]
pub async fn attest_forum_page_builder_transport(
    challenge: String,
) -> Result<ForumPageBuilderTransportAttestationResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        validate_attestation_challenge(&challenge)?;
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        require_forum_transport_authorization(&auth, &tenant)?;

        let (host, _event_bus) = runtime()?;
        require_forum_module_enabled(&host, tenant.id).await?;
        Ok(build_transport_attestation(challenge))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = challenge;
        Err(ServerFnError::new(
            "forum/page-builder-transport-attestation requires the `ssr` feature",
        ))
    }
}

/// Native Leptos transport used by the admin composition root. The transport returns the owner
/// DTO as JSON so `rustok-forum-admin` does not need a runtime dependency from browser builds to
/// the Forum backend crate and the Page Builder host remains provider-neutral.
#[server(prefix = "/api/fn", endpoint = "forum/page-builder-widget-preview")]
pub async fn preview_forum_page_builder_widget(
    request: ForumWidgetPreviewTransportRequest,
) -> Result<Value, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_forum_transport_authorization(&auth, &tenant)?;

        let (host, event_bus) = runtime()?;
        require_forum_module_enabled(&host, tenant.id).await?;
        let response = rustok_forum::ForumWidgetPreviewService::new(host.db_clone(), event_bus)
            .preview(
                tenant.id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                &request_context.locale,
                Some(tenant.default_locale.as_str()),
                rustok_forum::PreviewForumWidgetInput {
                    widget_type: request.widget_type,
                    props: request.props,
                },
            )
            .await
            .map_err(|error| ServerFnError::new(error.to_string()))?;

        serde_json::to_value(response).map_err(|error| {
            ServerFnError::new(format!(
                "Forum widget preview serialization failed: {error}"
            ))
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "forum/page-builder-widget-preview requires the `ssr` feature",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use rustok_api::{Action, Permission, Resource};
    use uuid::Uuid;

    fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> rustok_api::AuthContext {
        rustok_api::AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    fn tenant(id: Uuid) -> rustok_api::TenantContext {
        rustok_api::TenantContext {
            id,
            name: "Forum evidence tenant".to_string(),
            slug: "forum-evidence".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    #[test]
    fn transport_authorization_accepts_exact_read_and_effective_manage() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        assert!(require_forum_transport_authorization(
            &auth(tenant_id, vec![Permission::FORUM_TOPICS_READ]),
            &tenant,
        )
        .is_ok());
        assert!(require_forum_transport_authorization(
            &auth(
                tenant_id,
                vec![Permission::new(Resource::ForumTopics, Action::Manage)],
            ),
            &tenant,
        )
        .is_ok());
    }

    #[test]
    fn transport_authorization_rejects_missing_read_and_cross_tenant_context() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        let missing = require_forum_transport_authorization(&auth(tenant_id, Vec::new()), &tenant)
            .expect_err("missing forum_topics:read should be rejected")
            .to_string();
        assert!(missing.contains("forum_topics:read required"));

        let mismatch = require_forum_transport_authorization(
            &auth(Uuid::new_v4(), vec![Permission::FORUM_TOPICS_READ]),
            &tenant,
        )
        .expect_err("cross-tenant auth context should be rejected")
        .to_string();
        assert!(mismatch.contains("tenant scope mismatch"));
    }

    #[test]
    fn module_state_fails_closed_for_disabled_and_unavailable_states() {
        assert!(require_forum_module_state(Ok::<_, &str>(true)).is_ok());
        assert!(
            require_forum_module_state(Ok::<_, &str>(false))
                .expect_err("disabled Forum must be rejected")
                .to_string()
                .contains("Forum module is not enabled")
        );
        assert!(
            require_forum_module_state(Err::<bool, _>("database unavailable"))
                .expect_err("module lookup failure must fail closed")
                .to_string()
                .contains("Forum module state is unavailable")
        );
    }

    #[test]
    fn attestation_challenge_is_bounded_and_transport_identity_is_owner_derived() {
        assert!(validate_attestation_challenge("forum-attest_01.alpha:beta").is_ok());
        assert!(validate_attestation_challenge("").is_err());
        assert!(validate_attestation_challenge("contains space").is_err());
        assert!(
            validate_attestation_challenge(
                &"x".repeat(MAX_FORUM_PAGE_BUILDER_ATTESTATION_CHALLENGE_BYTES + 1)
            )
            .is_err()
        );

        let response = build_transport_attestation("forum-attest_01".to_string());
        assert_eq!(response.contract, FORUM_PAGE_BUILDER_ATTESTATION_CONTRACT);
        assert_eq!(response.module_id, "forum");
        assert_eq!(response.owner_provider, "rustok.forum");
        assert_eq!(
            response.widget_types,
            vec![
                "forum.reply_stream".to_string(),
                "forum.topic_detail".to_string(),
                "forum.topic_list".to_string(),
            ]
        );
        assert_eq!(response.preview_endpoint, FORUM_PAGE_BUILDER_PREVIEW_ENDPOINT);
        assert_eq!(
            response.property_schema_endpoint,
            FORUM_PAGE_BUILDER_PROPERTY_SCHEMA_ENDPOINT
        );
        assert_eq!(
            response.property_validate_endpoint,
            FORUM_PAGE_BUILDER_PROPERTY_VALIDATE_ENDPOINT
        );
    }
}
