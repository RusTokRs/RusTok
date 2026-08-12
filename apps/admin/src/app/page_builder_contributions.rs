use leptos::prelude::*;
use rustok_page_builder_admin::{
    PageBuilderContributionHostContext, PageBuilderContributionHostExtension,
    PageBuilderContributionPreviewError, PageBuilderContributionPreviewFuture,
    PageBuilderContributionPreviewPort, PageBuilderContributionPreviewRequest,
    PageBuilderContributionPropertyError, PageBuilderContributionPropertyIssue,
    PageBuilderContributionPropertyPort, PageBuilderContributionPropertySchema,
    PageBuilderContributionPropertySchemaFuture, PageBuilderContributionPropertySchemaRequest,
    PageBuilderContributionPropertyValidation, PageBuilderContributionPropertyValidationFuture,
    PageBuilderContributionPropertyValidationRequest,
};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default)]
struct ForumPageBuilderPreviewPort;

impl PageBuilderContributionPreviewPort for ForumPageBuilderPreviewPort {
    fn preview(
        &self,
        request: PageBuilderContributionPreviewRequest,
    ) -> PageBuilderContributionPreviewFuture {
        Box::pin(async move {
            if request.provider.trim() != "rustok.forum" {
                return Err(PageBuilderContributionPreviewError::with_stable_code(
                    format!(
                        "Forum preview port received provider `{}`",
                        request.provider.trim()
                    ),
                    "FORUM_PREVIEW_PROVIDER_MISMATCH",
                ));
            }
            rustok_forum_admin::preview_forum_page_builder_widget(
                rustok_forum_admin::ForumWidgetPreviewTransportRequest {
                    widget_type: request.component_type,
                    props: request.props,
                },
            )
            .await
            .map_err(|error| {
                PageBuilderContributionPreviewError::with_stable_code(
                    error.to_string(),
                    "FORUM_WIDGET_PREVIEW_TRANSPORT_FAILED",
                )
            })
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ForumPageBuilderPropertyPort;

impl PageBuilderContributionPropertyPort for ForumPageBuilderPropertyPort {
    fn schema(
        &self,
        request: PageBuilderContributionPropertySchemaRequest,
    ) -> PageBuilderContributionPropertySchemaFuture {
        Box::pin(async move {
            if request.provider.trim() != "rustok.forum" {
                return Err(PageBuilderContributionPropertyError::with_stable_code(
                    format!(
                        "Forum property port received provider `{}`",
                        request.provider.trim()
                    ),
                    "FORUM_PROPERTY_PROVIDER_MISMATCH",
                ));
            }
            let response = rustok_forum_admin::load_forum_page_builder_widget_property_schema(
                rustok_forum_admin::ForumWidgetPropertySchemaTransportRequest {
                    widget_type: request.component_type,
                    property_schema: request.property_schema,
                },
            )
            .await
            .map_err(|error| {
                PageBuilderContributionPropertyError::with_stable_code(
                    error.to_string(),
                    "FORUM_WIDGET_PROPERTY_SCHEMA_TRANSPORT_FAILED",
                )
            })?;
            Ok(PageBuilderContributionPropertySchema {
                schema_id: response.schema_id,
                schema: response.schema,
            })
        })
    }

    fn validate(
        &self,
        request: PageBuilderContributionPropertyValidationRequest,
    ) -> PageBuilderContributionPropertyValidationFuture {
        Box::pin(async move {
            if request.provider.trim() != "rustok.forum" {
                return Err(PageBuilderContributionPropertyError::with_stable_code(
                    format!(
                        "Forum property port received provider `{}`",
                        request.provider.trim()
                    ),
                    "FORUM_PROPERTY_PROVIDER_MISMATCH",
                ));
            }
            let response = rustok_forum_admin::validate_forum_page_builder_widget_properties(
                rustok_forum_admin::ForumWidgetPropertyValidationTransportRequest {
                    widget_type: request.component_type,
                    property_schema: request.property_schema,
                    props: request.props,
                },
            )
            .await
            .map_err(|error| {
                PageBuilderContributionPropertyError::with_stable_code(
                    error.to_string(),
                    "FORUM_WIDGET_PROPERTY_VALIDATE_TRANSPORT_FAILED",
                )
            })?;
            Ok(PageBuilderContributionPropertyValidation {
                valid: response.valid,
                normalized_props: response.normalized_props,
                issues: response
                    .issues
                    .into_iter()
                    .map(|issue| PageBuilderContributionPropertyIssue {
                        class: issue.class,
                        code: issue.code,
                        message: issue.message,
                        path: issue.path,
                    })
                    .collect(),
            })
        })
    }
}

#[server(prefix = "/api/fn", endpoint = "page-builder/contribution-permissions")]
async fn page_builder_contribution_permissions(
    required_permissions: Vec<String>,
) -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let mut granted = Vec::new();
        for required in required_permissions {
            let permission =
                rustok_api::Permission::from_str(required.trim()).map_err(|error| {
                    ServerFnError::new(format!(
                        "Invalid Page Builder contribution permission `{required}`: {error}"
                    ))
                })?;
            if rustok_api::has_effective_permission(&auth.permissions, &permission) {
                granted.push(permission.to_string());
            }
        }
        granted.sort();
        granted.dedup();
        Ok(granted)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = required_permissions;
        Err(ServerFnError::new(
            "page-builder/contribution-permissions requires the `ssr` feature",
        ))
    }
}

/// App composition scope for optional Page Builder provider extensions.
///
/// The enabled-module set has already crossed tenant control-plane loading before this component
/// is mounted. The browser sends only manifest-declared permissions; the server resolves each one
/// through `has_effective_permission`, so a resource `manage` grant correctly satisfies an exact
/// `read` contribution requirement without exposing the caller's complete permission snapshot.
#[component]
pub fn PageBuilderContributionScope(
    enabled_modules: HashSet<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let required_permissions = required_contribution_permissions(&enabled_modules);
    let permissions = LocalResource::new(move || {
        page_builder_contribution_permissions(required_permissions.clone())
    });
    let children_for_result = children.clone();

    view! {
        <Suspense fallback=|| view! {
            <div class="h-24 animate-pulse rounded-xl border border-border bg-muted" aria-label="Loading Page Builder contribution permissions"></div>
        }>
            {move || {
                let enabled_modules = enabled_modules.clone();
                let children = children_for_result.clone();
                permissions.get().map(|result| match result {
                    Ok(permissions) => view! {
                        <ResolvedPageBuilderContributionScope
                            enabled_modules
                            permissions
                            children
                        />
                    }.into_any(),
                    Err(error) => view! {
                        <div
                            class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
                            role="alert"
                            data-page-builder-contribution-host="permission-error"
                        >
                            {format!("Page Builder contribution permissions are unavailable: {error}")}
                        </div>
                    }.into_any(),
                })
            }}
        </Suspense>
    }
}

fn required_contribution_permissions(enabled_modules: &HashSet<String>) -> Vec<String> {
    let mut permissions = Vec::new();
    if enabled_modules.contains("forum") {
        permissions.extend(
            rustok_forum_admin::forum_contribution_manifest()
                .required_permissions
                .into_iter(),
        );
    }
    permissions.sort();
    permissions.dedup();
    permissions
}

#[component]
fn ResolvedPageBuilderContributionScope(
    enabled_modules: HashSet<String>,
    permissions: Vec<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let mut extensions = Vec::new();
    if enabled_modules.contains("forum") {
        extensions.push(
            PageBuilderContributionHostExtension::new(
                rustok_forum_admin::forum_contribution_manifest(),
                |registries| {
                    rustok_forum_admin::register_forum_fly_widgets(registries)
                        .map_err(|error| error.to_string())
                },
            )
            .with_preview_port(Arc::new(ForumPageBuilderPreviewPort))
            .with_property_port(Arc::new(ForumPageBuilderPropertyPort)),
        );
    }

    match PageBuilderContributionHostContext::new(extensions) {
        Ok(context) => {
            provide_context(context.with_granted_permissions(permissions));
            children().into_any()
        }
        Err(error) => view! {
            <div
                class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
                role="alert"
                data-page-builder-contribution-host="configuration-error"
            >
                {format!("Page Builder contribution host configuration failed: {error}")}
            </div>
        }
        .into_any(),
    }
}
