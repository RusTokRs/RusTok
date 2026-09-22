use std::fmt::{Debug, Formatter};

use fly::{GrapesJsCodec, ProjectHash};
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_page_builder_storefront::{
    AuthenticatedInlineEditSession, InlineEditGrant, InlineEditRequest,
    PageBuilderAuthenticatedInlineStorefront, PageSelection,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct PagesInlineEditBootstrap {
    pub pages_page_id: String,
    pub locale: String,
    pub project_data: Value,
    pub fly_page_id: String,
    pub revision_id: String,
    pub project_hash: u64,
    pub session_id: String,
    authorization_proof: String,
    pub expires_at_unix_ms: u64,
    pub channel_id: Option<String>,
    pub channel_slug: Option<String>,
}

impl PagesInlineEditBootstrap {
    pub fn authorization_proof(&self) -> &str {
        &self.authorization_proof
    }

    pub fn adapter_grant(&self) -> Result<InlineEditGrant, String> {
        InlineEditGrant::new(
            self.session_id.clone(),
            self.fly_page_id.clone(),
            self.revision_id.clone(),
            ProjectHash(self.project_hash),
            self.authorization_proof.clone(),
            self.expires_at_unix_ms,
        )
        .map_err(|error| error.to_string())
    }
}

impl Debug for PagesInlineEditBootstrap {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PagesInlineEditBootstrap")
            .field("pages_page_id", &self.pages_page_id)
            .field("locale", &self.locale)
            .field("project_data", &self.project_data)
            .field("fly_page_id", &self.fly_page_id)
            .field("revision_id", &self.revision_id)
            .field("project_hash", &self.project_hash)
            .field("session_id", &self.session_id)
            .field("authorization_proof", &"[REDACTED]")
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .field("channel_id", &self.channel_id)
            .field("channel_slug", &self.channel_slug)
            .finish()
    }
}

pub async fn fetch_pages_inline_edit_bootstrap(
    pages_page_id: String,
    locale: String,
) -> Result<PagesInlineEditBootstrap, ServerFnError> {
    pages_inline_edit_bootstrap(pages_page_id, locale).await
}

pub async fn commit_pages_inline_edit(
    request: InlineEditRequest,
) -> Result<PagesInlineEditBootstrap, ServerFnError> {
    pages_inline_edit_commit(request).await
}

#[server(
    PagesInlineEditBootstrapServerFn,
    prefix = "/api/fn",
    endpoint = "pages/inline-edit/bootstrap"
)]
async fn pages_inline_edit_bootstrap(
    pages_page_id: String,
    locale: String,
) -> Result<PagesInlineEditBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let context = InlineEditServerContext::extract().await?;
        let page_id = uuid::Uuid::parse_str(&pages_page_id).map_err(ServerFnError::new)?;
        let document = context
            .service
            .load_inline_edit_document(
                context.tenant_id,
                context.security.clone(),
                page_id,
                &locale,
            )
            .await
            .map_err(pages_server_error)?;
        issue_bootstrap(&context, document, current_unix_ms()?).map_err(pages_server_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (pages_page_id, locale);
        Err(ServerFnError::new(
            "pages/inline-edit/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(
    PagesInlineEditCommitServerFn,
    prefix = "/api/fn",
    endpoint = "pages/inline-edit/commit"
)]
async fn pages_inline_edit_commit(
    request: InlineEditRequest,
) -> Result<PagesInlineEditBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_pages::{PageBodyInput, SavePageDocumentInput};

        let context = InlineEditServerContext::extract().await?;
        let received_at_unix_ms = current_unix_ms()?;
        let claims = context
            .keyring
            .verify(request.authorization_proof(), received_at_unix_ms)
            .map_err(pages_server_error)?;
        ensure_claims_match_request(&context, &claims, &request).map_err(pages_server_error)?;

        let document = context
            .service
            .load_inline_edit_document(
                context.tenant_id,
                context.security.clone(),
                claims.pages_page_id,
                &claims.locale,
            )
            .await
            .map_err(pages_server_error)?;
        if document.revision_id != claims.revision_id {
            return Err(pages_server_error(
                rustok_pages::inline_edit_context_mismatch(
                    "Pages inline edit body revision changed after grant issuance",
                ),
            ));
        }
        let canonical =
            decode_canonical_document(&document.project_data).map_err(pages_server_error)?;
        if canonical.fly_page_id != claims.fly_page_id
            || canonical.project_hash.0 != claims.project_hash
        {
            return Err(pages_server_error(
                rustok_pages::inline_edit_context_mismatch(
                    "Pages inline edit Fly document identity changed after grant issuance",
                ),
            ));
        }

        let authorization_time_unix_ms = current_unix_ms()?;
        let proof = request.authorization_proof().to_string();
        let authorization_claims = context
            .keyring
            .verify(&proof, authorization_time_unix_ms)
            .map_err(pages_server_error)?;
        if authorization_claims != claims {
            return Err(pages_server_error(
                rustok_pages::inline_edit_context_mismatch(
                    "Pages inline edit grant identity changed during commit authorization",
                ),
            ));
        }
        ensure_claims_match_request(&context, &authorization_claims, &request)
            .map_err(pages_server_error)?;

        let adapter_grant = InlineEditGrant::new(
            claims.session_id.to_string(),
            claims.fly_page_id.clone(),
            claims.revision_id.clone(),
            ProjectHash(claims.project_hash),
            proof.clone(),
            claims.expires_at_unix_ms,
        )
        .map_err(ServerFnError::new)?;
        let mut session = AuthenticatedInlineEditSession::new(
            document.project_data,
            PageSelection::First,
            adapter_grant,
            authorization_time_unix_ms,
        )
        .map_err(ServerFnError::new)?;
        let claims_for_authorization = claims.clone();
        let result = session
            .apply_authorized(
                request,
                authorization_time_unix_ms,
                &move |candidate: &InlineEditRequest| {
                    request_matches_claims(candidate, &claims_for_authorization, &proof)
                },
            )
            .map_err(ServerFnError::new)?;

        context
            .service
            .ensure_builder_inline_edit_enabled_for_tenant(context.tenant_id)
            .await
            .map_err(pages_server_error)?;
        let saved = context
            .service
            .save_document(
                context.tenant_id,
                context.security.clone(),
                claims.pages_page_id,
                SavePageDocumentInput {
                    expected_revision: claims.revision_id.clone(),
                    body: PageBodyInput {
                        locale: claims.locale.clone(),
                        document: result.project_data.clone(),
                    },
                },
            )
            .await
            .map_err(pages_server_error)?;
        let revision_id = saved
            .body
            .filter(|body| body.locale == claims.locale)
            .map(|body| body.updated_at)
            .ok_or_else(|| {
                pages_server_error(rustok_pages::inline_edit_context_mismatch(
                    "Pages document save did not return the committed inline edit locale",
                ))
            })?;
        let next_document = rustok_pages::PageInlineEditDocument {
            pages_page_id: claims.pages_page_id,
            locale: claims.locale,
            revision_id,
            project_data: result.project_data,
        };
        issue_bootstrap_with_identity(
            &context,
            next_document,
            claims.fly_page_id,
            result.project_hash,
            current_unix_ms()?,
        )
        .map_err(pages_server_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "pages/inline-edit/commit requires the `ssr` feature",
        ))
    }
}

#[component]
pub fn PagesAuthenticatedInlineEditSurface(
    pages_page_id: String,
    locale: String,
    #[prop(optional)] class: Option<String>,
) -> impl IntoView {
    let class = class.unwrap_or_else(|| "rustok-pages-authenticated-inline-edit".to_string());
    let bootstrap = LocalResource::new(move || {
        let pages_page_id = pages_page_id.clone();
        let locale = locale.clone();
        async move { fetch_pages_inline_edit_bootstrap(pages_page_id, locale).await }
    });

    view! {
        <Suspense fallback=|| view! {
            <div class="rounded-2xl border border-border bg-muted/30 p-6" aria-busy="true">
                "Loading inline editor..."
            </div>
        }>
            {move || {
                bootstrap.get().map(|result| match result {
                    Ok(initial) => view! {
                        <LoadedPagesInlineEditSurface initial class=class.clone() />
                    }
                    .into_any(),
                    Err(error) => view! {
                        <div class="rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive" role="alert">
                            {error.to_string()}
                        </div>
                    }
                    .into_any(),
                })
            }}
        </Suspense>
    }
}

#[component]
fn LoadedPagesInlineEditSurface(initial: PagesInlineEditBootstrap, class: String) -> impl IntoView {
    let current = RwSignal::new(initial);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let status = RwSignal::new(None::<String>);

    move || {
        let bootstrap = current.get();
        let grant = match bootstrap.adapter_grant() {
            Ok(grant) => grant,
            Err(message) => {
                return view! {
                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive" role="alert">
                        {message}
                    </div>
                }
                .into_any();
            }
        };
        let on_request = Callback::new(move |request: InlineEditRequest| {
            if busy.get_untracked() {
                return;
            }
            busy.set(true);
            error.set(None);
            status.set(Some("Saving inline edit...".to_string()));
            spawn_local(async move {
                match commit_pages_inline_edit(request).await {
                    Ok(next) => {
                        current.set(next);
                        status.set(Some("Inline edit saved.".to_string()));
                    }
                    Err(commit_error) => {
                        error.set(Some(commit_error.to_string()));
                        status.set(None);
                    }
                }
                busy.set(false);
            });
        });
        let on_error = Callback::new(move |message: String| {
            error.set(Some(message));
            status.set(None);
        });
        view! {
            <section class=class.clone() data-pages-authenticated-inline-edit="true">
                <PageBuilderAuthenticatedInlineStorefront
                    project_data=bootstrap.project_data
                    grant
                    on_request
                    on_error
                    selection=PageSelection::First
                />
                {move || status.get().map(|message| view! {
                    <p class="mt-2 text-sm text-muted-foreground" role="status">{message}</p>
                })}
                {move || error.get().map(|message| view! {
                    <p class="mt-2 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">{message}</p>
                })}
            </section>
        }
        .into_any()
    }
}

#[cfg(feature = "ssr")]
#[derive(Clone)]
struct InlineEditServerContext {
    tenant_id: uuid::Uuid,
    auth: rustok_api::AuthContext,
    security: rustok_core::SecurityContext,
    channel_id: Option<uuid::Uuid>,
    channel_slug: Option<String>,
    keyring: rustok_pages::PageInlineEditKeyring,
    service: rustok_pages::PageService,
}

#[cfg(feature = "ssr")]
impl InlineEditServerContext {
    async fn extract() -> Result<Self, ServerFnError> {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthPrincipalContext, HostRuntimeContext};
        use rustok_outbox::TransactionalEventBus;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let principal = leptos_axum::extract::<AuthPrincipalContext>()
            .await
            .map_err(ServerFnError::new)?;
        if !principal.kind.is_direct_user() || auth.session_id.is_nil() {
            return Err(pages_server_error(
                rustok_pages::inline_edit_context_mismatch(
                    "Pages inline editing requires a direct authenticated user session",
                ),
            ));
        }
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(pages_server_error(
                rustok_pages::inline_edit_context_mismatch(
                    "Pages inline edit authenticated tenant does not match request tenant",
                ),
            ));
        }
        let request = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        if request.as_ref().is_some_and(|request| {
            request.tenant_id != tenant.id || request.user_id != Some(auth.user_id)
        }) {
            return Err(pages_server_error(
                rustok_pages::inline_edit_context_mismatch(
                    "Pages inline edit request context identity does not match authentication",
                ),
            ));
        }
        let keyring = runtime
            .shared_get::<rustok_pages::PageInlineEditKeyring>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "PAGE_INLINE_EDIT_SIGNING_UNAVAILABLE: Inline editing is not configured on this host.",
                )
            })?;
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "PAGE_INLINE_EDIT_RUNTIME_UNAVAILABLE: Inline editing persistence is unavailable.",
                )
            })?;
        let security = rustok_core::security_context_from_access_token(
            auth.user_id,
            &auth.grant_type,
            &auth.permissions,
        );
        let service = rustok_pages::PageService::new(runtime.db_clone(), event_bus);
        service
            .ensure_builder_inline_edit_enabled_for_tenant(tenant.id)
            .await
            .map_err(pages_server_error)?;
        Ok(Self {
            tenant_id: tenant.id,
            auth,
            security,
            channel_id: request.as_ref().and_then(|request| request.channel_id),
            channel_slug: request
                .as_ref()
                .and_then(|request| normalize_channel_slug(request.channel_slug.as_deref())),
            keyring,
            service,
        })
    }
}

#[cfg(feature = "ssr")]
struct CanonicalInlineDocument {
    fly_page_id: String,
    project_hash: ProjectHash,
}

#[cfg(feature = "ssr")]
fn decode_canonical_document(
    project_data: &Value,
) -> Result<CanonicalInlineDocument, rustok_pages::PagesError> {
    let document = GrapesJsCodec::decode_value(project_data.clone()).map_err(|_| {
        rustok_pages::inline_edit_context_mismatch(
            "Pages inline edit document could not be decoded by the canonical Fly codec",
        )
    })?;
    if document.project.pages.len() != 1 {
        return Err(rustok_pages::inline_edit_context_mismatch(
            "Pages inline edit requires exactly one Fly page in the localized body",
        ));
    }
    let fly_page_id = document.project.pages[0]
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            rustok_pages::inline_edit_context_mismatch(
                "Pages inline edit requires a stable Fly page id",
            )
        })?;
    let mut unstable_component_path = None;
    document.project.visit_components(|component, _, path| {
        if unstable_component_path.is_none()
            && component
                .id()
                .map(str::trim)
                .is_none_or(|component_id| component_id.is_empty())
        {
            unstable_component_path = Some(path.to_string());
        }
    });
    if let Some(path) = unstable_component_path {
        return Err(rustok_pages::inline_edit_context_mismatch(format!(
            "Pages inline edit requires stable component ids before hashing; missing at {path}"
        )));
    }
    Ok(CanonicalInlineDocument {
        fly_page_id,
        project_hash: document.hash(),
    })
}

#[cfg(feature = "ssr")]
fn issue_bootstrap(
    context: &InlineEditServerContext,
    document: rustok_pages::PageInlineEditDocument,
    now_unix_ms: u64,
) -> Result<PagesInlineEditBootstrap, rustok_pages::PagesError> {
    let canonical = decode_canonical_document(&document.project_data)?;
    issue_bootstrap_with_identity(
        context,
        document,
        canonical.fly_page_id,
        canonical.project_hash,
        now_unix_ms,
    )
}

#[cfg(feature = "ssr")]
fn issue_bootstrap_with_identity(
    context: &InlineEditServerContext,
    document: rustok_pages::PageInlineEditDocument,
    fly_page_id: String,
    project_hash: ProjectHash,
    now_unix_ms: u64,
) -> Result<PagesInlineEditBootstrap, rustok_pages::PagesError> {
    let issued = context.keyring.issue(
        rustok_pages::PageInlineEditGrantContext {
            tenant_id: context.tenant_id,
            actor_id: context.auth.user_id,
            auth_session_id: context.auth.session_id,
            session_id: uuid::Uuid::new_v4(),
            pages_page_id: document.pages_page_id,
            fly_page_id: fly_page_id.clone(),
            locale: document.locale.clone(),
            revision_id: document.revision_id.clone(),
            project_hash: project_hash.0,
            channel_id: context.channel_id,
            channel_slug: context.channel_slug.clone(),
        },
        now_unix_ms,
    )?;
    Ok(PagesInlineEditBootstrap {
        pages_page_id: document.pages_page_id.to_string(),
        locale: document.locale,
        project_data: document.project_data,
        fly_page_id,
        revision_id: issued.claims.revision_id.clone(),
        project_hash: issued.claims.project_hash,
        session_id: issued.claims.session_id.to_string(),
        authorization_proof: issued.authorization_proof().to_string(),
        expires_at_unix_ms: issued.claims.expires_at_unix_ms,
        channel_id: issued.claims.channel_id.map(|value| value.to_string()),
        channel_slug: issued.claims.channel_slug,
    })
}

#[cfg(feature = "ssr")]
fn ensure_claims_match_request(
    context: &InlineEditServerContext,
    claims: &rustok_pages::PageInlineEditGrantClaims,
    request: &InlineEditRequest,
) -> Result<(), rustok_pages::PagesError> {
    if claims.tenant_id != context.tenant_id
        || claims.actor_id != context.auth.user_id
        || claims.auth_session_id != context.auth.session_id
        || claims.channel_id != context.channel_id
        || claims.channel_slug != context.channel_slug
        || request.session_id != claims.session_id.to_string()
        || request.page_id != claims.fly_page_id
        || request.revision_id != claims.revision_id
        || request.expected_project_hash.0 != claims.project_hash
        || request.expires_at_unix_ms != claims.expires_at_unix_ms
    {
        return Err(rustok_pages::inline_edit_context_mismatch(
            "Pages inline edit grant, request, authentication, or channel context does not match",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn request_matches_claims(
    request: &InlineEditRequest,
    claims: &rustok_pages::PageInlineEditGrantClaims,
    authorization_proof: &str,
) -> Result<(), String> {
    if request.session_id == claims.session_id.to_string()
        && request.page_id == claims.fly_page_id
        && request.revision_id == claims.revision_id
        && request.expected_project_hash.0 == claims.project_hash
        && request.expires_at_unix_ms == claims.expires_at_unix_ms
        && request.authorization_proof() == authorization_proof
    {
        Ok(())
    } else {
        Err("inline edit request no longer matches its trusted Pages grant".to_string())
    }
}

#[cfg(feature = "ssr")]
fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

#[cfg(feature = "ssr")]
fn current_unix_ms() -> Result<u64, ServerFnError> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(ServerFnError::new)?;
    u64::try_from(elapsed.as_millis()).map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
fn pages_server_error(error: rustok_pages::PagesError) -> ServerFnError {
    let rich: rustok_core::error::RichError = error.into();
    let code = rich
        .error_code
        .as_deref()
        .unwrap_or("PAGE_INLINE_EDIT_FAILED");
    let message = rich
        .user_message
        .as_deref()
        .unwrap_or("The inline edit request could not be completed.");
    ServerFnError::new(format!("{code}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bootstrap_debug_redacts_authorization_proof() {
        let bootstrap = PagesInlineEditBootstrap {
            pages_page_id: uuid::Uuid::new_v4().to_string(),
            locale: "en".to_string(),
            project_data: json!({"pages": []}),
            fly_page_id: "home".to_string(),
            revision_id: "revision".to_string(),
            project_hash: 7,
            session_id: uuid::Uuid::new_v4().to_string(),
            authorization_proof: "signed-proof".to_string(),
            expires_at_unix_ms: 10_000,
            channel_id: None,
            channel_slug: None,
        };
        assert!(!format!("{bootstrap:?}").contains("signed-proof"));
        assert_eq!(bootstrap.authorization_proof(), "signed-proof");
    }

    #[test]
    fn bootstrap_reconstructs_exact_adapter_grant() {
        let bootstrap = PagesInlineEditBootstrap {
            pages_page_id: uuid::Uuid::new_v4().to_string(),
            locale: "en".to_string(),
            project_data: json!({"pages": []}),
            fly_page_id: "home".to_string(),
            revision_id: "revision".to_string(),
            project_hash: 7,
            session_id: uuid::Uuid::new_v4().to_string(),
            authorization_proof: "signed-proof".to_string(),
            expires_at_unix_ms: 10_000,
            channel_id: None,
            channel_slug: None,
        };
        let grant = bootstrap.adapter_grant().expect("grant");
        assert_eq!(grant.page_id(), "home");
        assert_eq!(grant.revision_id(), "revision");
        assert_eq!(grant.expected_project_hash(), ProjectHash(7));
    }
}
