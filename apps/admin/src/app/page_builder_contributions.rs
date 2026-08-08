use leptos::prelude::*;
use rustok_page_builder_admin::{
    PageBuilderContributionHostContext, PageBuilderContributionHostExtension,
    PageBuilderContributionPreviewError, PageBuilderContributionPreviewFuture,
    PageBuilderContributionPreviewPort, PageBuilderContributionPreviewRequest,
};
use std::collections::HashSet;
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

#[server(prefix = "/api/fn", endpoint = "page-builder/contribution-permissions")]
async fn page_builder_contribution_permissions() -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let mut permissions = auth
            .permissions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        permissions.sort();
        permissions.dedup();
        Ok(permissions)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "page-builder/contribution-permissions requires the `ssr` feature",
        ))
    }
}

/// App composition scope for optional Page Builder provider extensions.
///
/// The enabled-module set has already crossed tenant control-plane loading before this component
/// is mounted. Effective permissions are loaded from the authenticated server snapshot rather than
/// inferred from the client-visible role string.
#[component]
pub fn PageBuilderContributionScope(
    enabled_modules: HashSet<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let permissions = LocalResource::new(page_builder_contribution_permissions);
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
            .with_preview_port(Arc::new(ForumPageBuilderPreviewPort)),
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
