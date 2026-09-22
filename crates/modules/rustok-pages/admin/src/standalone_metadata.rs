use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use crate::model::PageDetail;
use crate::transport;
use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::use_route_query_value;
use rustok_page_builder_admin::{ConsumerPropertiesPanel, ConsumerPropertyEditorRuntime};
use rustok_ui_core::AdminQueryKey;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublishedMetadataSurfaceAdmission {
    Hidden,
    Registered,
}

fn published_metadata_surface_admission(
    page: Option<&PageDetail>,
) -> PublishedMetadataSurfaceAdmission {
    match page {
        Some(page) if page.status.eq_ignore_ascii_case("published") => {
            PublishedMetadataSurfaceAdmission::Registered
        }
        _ => PublishedMetadataSurfaceAdmission::Hidden,
    }
}

#[component]
pub(crate) fn PagesPublishedMetadataSurface(refresh_generation: RwSignal<u64>) -> impl IntoView {
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let token = use_token();
    let tenant = use_tenant();
    let runtime = use_context::<Arc<ConsumerPropertyEditorRuntime>>();
    let contribution_assembly = Some(Arc::new(build_pages_admin_contribution_registry(
        &pages_admin_contribution_policy(),
    )));

    let page_token = token;
    let page_tenant = tenant;
    let page_resource = LocalResource::new(move || {
        let page_id = selected_page_query.get();
        let token = page_token.get();
        let tenant = page_tenant.get();
        let _generation = refresh_generation.get();
        async move {
            let Some(page_id) = page_id.filter(|value| !value.trim().is_empty()) else {
                return Ok::<_, transport::TransportError>(None);
            };
            transport::fetch_page(token, tenant, page_id).await
        }
    });

    view! {
        <Suspense fallback=|| ()>
            {move || {
                let runtime = runtime.clone();
                let contribution_assembly = contribution_assembly.clone();
                page_resource.get().map(|result| match result {
                    Ok(page) => match published_metadata_surface_admission(page.as_ref()) {
                        PublishedMetadataSurfaceAdmission::Registered => view! {
                            <div
                                class="space-y-2"
                                data-pages-published-metadata-surface="registered"
                                data-pages-published-metadata-admission="published-only"
                                data-pages-fly-canvas-mounted="false"
                                data-pages-document-authoring="false"
                                data-pages-metadata-runtime="registered"
                                data-pages-metadata-persistence="owner-port"
                            >
                                <div class="rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-muted-foreground">
                                    "Published metadata uses the registered Pages property contract. The immutable Fly document remains unmounted."
                                </div>
                                <ConsumerPropertiesPanel
                                    runtime=runtime.clone()
                                    contribution_assembly=contribution_assembly.clone()
                                />
                            </div>
                        }
                        .into_any(),
                        PublishedMetadataSurfaceAdmission::Hidden => ().into_any(),
                    },
                    Err(load_error) => view! {
                        <div
                            class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
                            data-pages-published-metadata-surface="error"
                            role="alert"
                        >
                            {format!("Unable to load the published metadata surface: {load_error}")}
                        </div>
                    }
                    .into_any(),
                })
            }}
        </Suspense>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(status: &str) -> PageDetail {
        PageDetail {
            id: "page-1".to_string(),
            version: 7,
            status: status.to_string(),
            template: "default".to_string(),
            updated_at: "2026-08-03T00:00:00Z".to_string(),
            available_locales: vec!["en".to_string()],
            channel_slugs: vec!["web".to_string()],
            translation: None,
            body: None,
        }
    }

    #[test]
    fn published_page_admits_registered_metadata_surface() {
        assert_eq!(
            published_metadata_surface_admission(Some(&page("published"))),
            PublishedMetadataSurfaceAdmission::Registered
        );
        assert_eq!(
            published_metadata_surface_admission(Some(&page("PUBLISHED"))),
            PublishedMetadataSurfaceAdmission::Registered
        );
    }

    #[test]
    fn non_published_or_missing_page_hides_registered_metadata_surface() {
        for status in ["draft", "archived", ""] {
            assert_eq!(
                published_metadata_surface_admission(Some(&page(status))),
                PublishedMetadataSurfaceAdmission::Hidden
            );
        }
        assert_eq!(
            published_metadata_surface_admission(None),
            PublishedMetadataSurfaceAdmission::Hidden
        );
    }
}
