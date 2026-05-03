use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::app::modules::page_for_route_segment;
use crate::app::providers::enabled_modules::use_enabled_modules_context;
use crate::shared::context::module_request::ModuleRequestProvider;
use crate::{t_string, use_i18n};

#[derive(Params, PartialEq)]
struct ModuleAdminParams {
    module_slug: Option<String>,
    module_path: Option<String>,
}

#[component]
pub fn ModuleAdminPage() -> impl IntoView {
    let i18n = use_i18n();
    let params = use_params::<ModuleAdminParams>();
    let enabled_modules = use_enabled_modules_context();

    let route_segment = Signal::derive(move || {
        params.with(|params| {
            params
                .as_ref()
                .ok()
                .and_then(|params| params.module_slug.clone())
                .unwrap_or_default()
        })
    });
    let module_subpath = Signal::derive(move || {
        params.with(|params| {
            params
                .as_ref()
                .ok()
                .and_then(|params| params.module_path.clone())
                .map(|subpath| subpath.trim_matches('/').to_string())
                .filter(|subpath| !subpath.is_empty())
        })
    });

    let any_page = Signal::derive(move || page_for_route_segment(&route_segment.get(), None));
    let enabled_page = Signal::derive(move || {
        let enabled = enabled_modules.modules.get();
        page_for_route_segment(&route_segment.get(), Some(&enabled))
    });
    let is_loading = Signal::derive(move || enabled_modules.is_loading.get());

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            {move || {
                if is_loading.get() {
                    return view! {
                        <div class="space-y-4">
                            <div class="h-10 w-64 animate-pulse rounded-xl bg-muted"></div>
                            <div class="h-64 animate-pulse rounded-xl bg-muted"></div>
                        </div>
                    }
                    .into_any();
                }

                match (any_page.get(), enabled_page.get()) {
                    (_, Some(page)) => {
                        let route_segment_value = route_segment.get();
                        let subpath_value = module_subpath.get();
                        view! {
                            <div class="space-y-6">
                                <ModulePageSecondaryNav
                                    page=page.clone()
                                    active_subpath=subpath_value.clone()
                                />
                                <ModuleRequestProvider
                                    route_segment=Some(route_segment_value)
                                    subpath=subpath_value
                                >
                                    {(page.render)()}
                                </ModuleRequestProvider>
                            </div>
                        }
                        .into_any()
                    }
                    (Some(page), None) => view! {
                        <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                            <div class="max-w-2xl space-y-3">
                                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                                    {page.module_slug.to_string()}
                                </span>
                                <h1 class="text-2xl font-semibold text-card-foreground">{page.title.to_string()}</h1>
                                <p class="text-sm text-muted-foreground">
                                    {t_string!(i18n, modules.moduleDisabled)}
                                </p>
                            </div>
                        </div>
                    }
                    .into_any(),
                    (None, None) => view! {
                        <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                            <div class="max-w-2xl space-y-3">
                                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                                    "module route"
                                </span>
                                <h1 class="text-2xl font-semibold text-card-foreground">
                                    {t_string!(i18n, modules.moduleNotFoundTitle)}
                                </h1>
                                <p class="text-sm text-muted-foreground">
                                    {t_string!(i18n, modules.moduleNotFound)}
                                </p>
                            </div>
                        </div>
                    }
                    .into_any(),
                }
            }}
        </section>
    }
}

#[component]
fn ModulePageSecondaryNav(
    page: crate::app::modules::AdminPageRegistration,
    active_subpath: Option<String>,
) -> impl IntoView {
    if page.child_pages.is_empty() {
        return ().into_any();
    }

    let route_segment = page.route_segment;
    let root_href = format!("/modules/{route_segment}");
    let root_is_active = active_subpath.is_none();

    view! {
        <nav class="rounded-xl border border-border bg-card p-2 shadow-sm">
            <div class="flex flex-wrap gap-2">
                <A
                    href=root_href
                    attr:class=move || secondary_nav_class(root_is_active)
                >
                    {page.title}
                </A>
                {page
                    .child_pages
                    .iter()
                    .map(|child| {
                        let href = format!("/modules/{route_segment}/{}", child.subpath);
                        let child_subpath = child.subpath.to_string();
                        let active_subpath = active_subpath.clone();
                        let is_active = active_subpath
                            .as_deref()
                            .map(|active| {
                                active == child_subpath
                                    || active.starts_with(&format!("{child_subpath}/"))
                            })
                            .unwrap_or(false);
                        view! {
                            <A
                                href=href
                                attr:class=move || secondary_nav_class(is_active)
                            >
                                {child.nav_label}
                            </A>
                        }
                    })
                    .collect_view()}
            </div>
        </nav>
    }
    .into_any()
}

fn secondary_nav_class(is_active: bool) -> String {
    format!(
        "inline-flex items-center rounded-lg px-3 py-2 text-sm font-medium transition {}",
        if is_active {
            "bg-primary text-primary-foreground"
        } else {
            "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        }
    )
}
