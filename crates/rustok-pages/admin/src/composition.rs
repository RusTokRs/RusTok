use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::core;
use crate::i18n::t;
use crate::model::PageDetail;
use crate::transport;
use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::use_route_query_value;
use rustok_page_builder_admin::{PageBuilderAdminFacade, PageBuilderAdminWithController};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};
use std::rc::Rc;

#[component]
pub fn PagesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let token = use_token();
    let tenant = use_tenant();
    let default_locale = route_context.locale.clone().unwrap_or_default();
    let loading_label = t(
        route_context.locale.as_deref(),
        "pages.builder.loading",
        "Loading visual editor...",
    );
    let missing_label = t(
        route_context.locale.as_deref(),
        "pages.builder.missing",
        "Select an existing page to open the Fly visual editor.",
    );
    let load_error_label = t(
        route_context.locale.as_deref(),
        "pages.builder.loadError",
        "Failed to load the selected page for visual editing",
    );

    let builder_resource = LocalResource::new(move || {
        let page_id = selected_page_query.get();
        let token = token.get();
        let tenant = tenant.get();
        async move {
            match page_id.filter(|page_id| core::optional_ui_text(page_id).is_some()) {
                Some(page_id) => transport::fetch_page(token, tenant, page_id).await,
                None => Ok(None),
            }
        }
    });

    view! {
        <div class="space-y-6">
            <section class="rounded-2xl border border-border bg-card p-4 shadow-sm">
                <Suspense fallback=move || view! {
                    <div class="h-48 animate-pulse rounded-xl bg-muted" aria-label=loading_label.clone()></div>
                }>
                    {move || {
                        builder_resource.get().map(|result| match result {
                            Ok(Some(page)) => view! {
                                <PagesFlyBuilder
                                    page
                                    token
                                    tenant
                                    default_locale=default_locale.clone()
                                />
                            }.into_any(),
                            Ok(None) => view! {
                                <div class="rounded-xl border border-dashed border-border px-5 py-8 text-sm text-muted-foreground" role="status">
                                    {missing_label.clone()}
                                </div>
                            }.into_any(),
                            Err(error) => view! {
                                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                                    {format!("{load_error_label}: {error}")}
                                </div>
                            }.into_any(),
                        })
                    }}
                </Suspense>
            </section>

            <crate::ui::leptos::PagesAdmin />
        </div>
    }
}

#[component]
fn PagesFlyBuilder(
    page: PageDetail,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    default_locale: String,
) -> impl IntoView {
    let seed = core::edit_form_seed_from_page(&page, &default_locale);
    let revision_id = builder::page_revision(&page);
    let controller = builder::controller_from_project(
        &page.id,
        &revision_id,
        &seed.project_data_text,
    );

    match controller {
        Ok(controller) => {
            let page_id = page.id.clone();
            let snapshot_default_locale = default_locale.clone();
            let facade: Rc<dyn PageBuilderAdminFacade> = Rc::new(PagesBuilderFacade::new(
                move || PagesBuilderSaveSnapshot {
                    token: token.get_untracked(),
                    tenant_slug: tenant.get_untracked(),
                    page_id: page_id.clone(),
                    default_locale: snapshot_default_locale.clone(),
                },
                |_page, _project_data| {},
            ));
            view! {
                <PageBuilderAdminWithController controller facade=Some(facade) />
            }
            .into_any()
        }
        Err(error) => view! {
            <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                {error.to_string()}
            </div>
        }
        .into_any(),
    }
}
