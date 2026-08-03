use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use crate::transport;
use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::use_route_query_value;
use rustok_page_builder_admin::{ConsumerPropertiesPanel, ConsumerPropertyEditorRuntime};
use rustok_ui_core::AdminQueryKey;
use std::sync::Arc;

#[component]
pub(crate) fn PagesPublishedMetadataSurface(
    refresh_generation: RwSignal<u64>,
) -> impl IntoView {
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
                    Ok(Some(page)) if page.status.eq_ignore_ascii_case("published") => view! {
                        <div
                            class="space-y-2"
                            data-pages-published-metadata-surface="registered"
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
                    Ok(_) => ().into_any(),
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
