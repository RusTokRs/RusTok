use crate::transport;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::use_route_query_value;
use rustok_ui_core::AdminQueryKey;

#[component]
pub(crate) fn PagesRollbackControl(on_rolled_back: Callback<()>) -> impl IntoView {
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let token = use_token();
    let tenant = use_tenant();
    let refresh_generation = RwSignal::new(0_u64);
    let busy = RwSignal::new(false);
    let armed_page_id = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let success = RwSignal::new(None::<String>);

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
                page_resource.get().map(|result| match result {
                    Ok(Some(page)) if page.status.eq_ignore_ascii_case("published") => {
                        let action_page_id = page.id.clone();
                        let label_page_id = page.id.clone();
                        let cancel_page_id = page.id.clone();
                        view! {
                            <section class="rounded-xl border border-amber-300/50 bg-amber-50 px-4 py-3 text-amber-950 shadow-sm">
                                <div class="flex flex-wrap items-center justify-between gap-3">
                                    <div>
                                        <div class="text-sm font-semibold">"Immutable artifact rollback"</div>
                                        <p class="mt-1 text-xs leading-5 text-amber-900/80">
                                            "Restore the previous distinct verified publish artifact set without recompiling the current Fly document."
                                        </p>
                                    </div>
                                    <div class="flex items-center gap-2">
                                        {move || {
                                            let armed = armed_page_id
                                                .get()
                                                .as_deref()
                                                .is_some_and(|page_id| page_id == cancel_page_id);
                                            armed.then(|| view! {
                                                <button
                                                    type="button"
                                                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                                                    disabled=move || busy.get()
                                                    on:click=move |_| {
                                                        armed_page_id.set(None);
                                                        error.set(None);
                                                    }
                                                >
                                                    "Cancel"
                                                </button>
                                            })
                                        }}
                                        <button
                                            type="button"
                                            class="rounded-lg border border-amber-500/50 bg-background px-3 py-2 text-sm font-medium hover:bg-amber-100 disabled:opacity-50"
                                            disabled=move || busy.get()
                                            title="Restore the previous immutable publish artifact set"
                                            on:click=move |_| {
                                                if busy.get_untracked() {
                                                    return;
                                                }
                                                let confirmed = armed_page_id
                                                    .get_untracked()
                                                    .as_deref()
                                                    .is_some_and(|page_id| page_id == action_page_id);
                                                if !confirmed {
                                                    armed_page_id.set(Some(action_page_id.clone()));
                                                    error.set(None);
                                                    success.set(None);
                                                    return;
                                                }

                                                let page_id = action_page_id.clone();
                                                let token = token.get_untracked();
                                                let tenant = tenant.get_untracked();
                                                busy.set(true);
                                                armed_page_id.set(None);
                                                error.set(None);
                                                success.set(None);
                                                spawn_local(async move {
                                                    match transport::rollback_page(token, tenant, page_id).await {
                                                        Ok(result) => {
                                                            success.set(Some(format!(
                                                                "Rollback committed as page version {}.",
                                                                result.version()
                                                            )));
                                                            refresh_generation.update(|generation| {
                                                                *generation = generation.wrapping_add(1)
                                                            });
                                                            on_rolled_back.run(());
                                                        }
                                                        Err(rollback_error) => {
                                                            error.set(Some(rollback_error.to_string()));
                                                        }
                                                    }
                                                    busy.set(false);
                                                });
                                            }
                                        >
                                            {move || {
                                                if busy.get() {
                                                    "Rolling back..."
                                                } else if armed_page_id
                                                    .get()
                                                    .as_deref()
                                                    .is_some_and(|page_id| page_id == label_page_id)
                                                {
                                                    "Confirm rollback"
                                                } else {
                                                    "Prepare rollback"
                                                }
                                            }}
                                        </button>
                                    </div>
                                </div>
                                {move || error.get().map(|message| view! {
                                    <div class="mt-3 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                                        {message}
                                    </div>
                                })}
                                {move || success.get().map(|message| view! {
                                    <div class="mt-3 rounded-lg border border-emerald-300/40 bg-emerald-50 px-3 py-2 text-sm text-emerald-800" role="status">
                                        {message}
                                    </div>
                                })}
                            </section>
                        }
                        .into_any()
                    }
                    Ok(_) => ().into_any(),
                    Err(load_error) => view! {
                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                            {format!("Unable to evaluate rollback availability: {load_error}")}
                        </div>
                    }
                    .into_any(),
                })
            }}
        </Suspense>
    }
}
