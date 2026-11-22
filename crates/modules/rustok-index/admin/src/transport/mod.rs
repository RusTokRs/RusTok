mod graphql_adapter;
#[cfg(feature = "ssr")]
mod native_http;
mod native_server_adapter;

use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};

use crate::model::{
    CancelActionResult, CancelJobInput, IndexAdminBootstrap, ReplayActionResult, RetryActionResult,
    RetryJobInput, TriggerReplayInput,
};

pub use native_server_adapter::{
    cancel_job_native, fetch_bootstrap_native, retry_job_native, trigger_replay_native,
};

#[cfg(feature = "ssr")]
pub use native_http::IndexAdminUpstream;
#[cfg(feature = "ssr")]
pub use native_server_adapter::IndexAdminReplayStop;

pub type IndexAdminTransportError = UiTransportError;

const fn command_path() -> UiTransportPath {
    if cfg!(any(feature = "ssr", feature = "hydrate")) {
        UiTransportPath::NativeServer
    } else {
        UiTransportPath::Graphql
    }
}

/// Native browser calls read the current headers from canonical auth storage. Compare them to
/// the credentials that authorized this intent, including the public tenant changed in another tab.
pub(crate) fn is_current_scope(token: &str, tenant: &str) -> bool {
    if token.is_empty() || tenant.is_empty() {
        return false;
    }
    #[cfg(target_arch = "wasm32")]
    {
        use rustok_auth_ui::ui::leptos::storage;
        storage::get_token().as_deref() == Some(token) && storage::matches_request_tenant(tenant)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true // Native endpoint authority is derived from the validated request, not browser storage.
    }
}

fn require_scope(
    path: UiTransportPath,
    token: Option<&str>,
    tenant: Option<&str>,
) -> Result<(), IndexAdminTransportError> {
    let error = |message| match path {
        UiTransportPath::NativeServer => UiTransportError::native("index", message),
        UiTransportPath::Graphql => UiTransportError::graphql("index", message),
    };
    match (token, tenant) {
        (Some(token), Some(tenant)) if is_current_scope(token, tenant) => Ok(()),
        (Some(token), Some(tenant)) if !token.is_empty() && !tenant.is_empty() => {
            Err(error("Index request scope changed"))
        }
        _ => Err(error("Index authentication required")),
    }
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<IndexAdminBootstrap, IndexAdminTransportError> {
    // Only the internal combined diagnostics snapshot has no GraphQL equivalent.
    require_scope(
        UiTransportPath::NativeServer,
        token.as_deref(),
        tenant.as_deref(),
    )?;
    let snapshot = fetch_scoped_bootstrap(token.clone(), tenant.clone())
        .await
        .map_err(|error| UiTransportError::native("index", error))?;
    require_scope(
        UiTransportPath::NativeServer,
        token.as_deref(),
        tenant.as_deref(),
    )?;
    if tenant.as_deref() != Some(snapshot.tenant.slug.as_str()) {
        return Err(UiTransportError::native(
            "index",
            "Index snapshot scope changed",
        ));
    }
    Ok(snapshot)
}

async fn fetch_scoped_bootstrap(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<IndexAdminBootstrap, leptos::prelude::ServerFnError> {
    #[cfg(feature = "ssr")]
    if let Some(upstream) = leptos::prelude::use_context::<IndexAdminUpstream>() {
        // SSR rendering has a validated session snapshot, not a browser AJAX Authorization header.
        return upstream
            .call(
                native_http::IndexEndpoint::Bootstrap,
                token,
                tenant,
                serde_json::json!({}),
            )
            .await;
    }
    #[cfg(not(feature = "ssr"))]
    let _ = (token, tenant);
    fetch_bootstrap_native().await
}

pub async fn trigger_replay(
    token: Option<String>,
    tenant: Option<String>,
    input: TriggerReplayInput,
) -> Result<ReplayActionResult, IndexAdminTransportError> {
    require_scope(command_path(), token.as_deref(), tenant.as_deref())?;
    let native = input.clone();
    // Do not fall back to the other transport after an uncertain write result.
    execute_selected_transport(
        "index",
        command_path(),
        move || trigger_replay_native(native),
        move || graphql_adapter::trigger_replay(token, tenant, input),
    )
    .await
}

pub async fn cancel_job(
    token: Option<String>,
    tenant: Option<String>,
    input: CancelJobInput,
) -> Result<CancelActionResult, IndexAdminTransportError> {
    require_scope(command_path(), token.as_deref(), tenant.as_deref())?;
    let native = input.clone();
    execute_selected_transport(
        "index",
        command_path(),
        move || cancel_job_native(native),
        move || graphql_adapter::cancel_job(token, tenant, input),
    )
    .await
}

pub async fn retry_job(
    token: Option<String>,
    tenant: Option<String>,
    input: RetryJobInput,
) -> Result<RetryActionResult, IndexAdminTransportError> {
    require_scope(command_path(), token.as_deref(), tenant.as_deref())?;
    let native = input.clone();
    execute_selected_transport(
        "index",
        command_path(),
        move || retry_job_native(native),
        move || graphql_adapter::retry_job(token, tenant, input),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_path_follows_the_compiled_profile() {
        #[cfg(any(feature = "ssr", feature = "hydrate"))]
        assert_eq!(command_path(), UiTransportPath::NativeServer);
        #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
        assert_eq!(command_path(), UiTransportPath::Graphql);
        let denied = require_scope(command_path(), Some("fixture"), None).unwrap_err();
        assert_eq!(denied.failed_path, command_path());
        assert!(!denied.fallback_attempted);
    }
}
