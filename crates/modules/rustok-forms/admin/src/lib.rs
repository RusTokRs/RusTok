//! Leptos admin surface for the forms module: submission triage inbox.

pub mod core;
mod i18n;
mod model;
mod transport;

pub use core::*;
pub use i18n::*;
pub use model::FormSubmission;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use uuid::Uuid;

/// Inbox listing form submissions with triage actions.
#[component]
pub fn FormsAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let labels = FormsAdminLabels::from_locale(None);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let fetch_token = token;
    let fetch_tenant = tenant;
    let submissions = LocalResource::new(move || {
        let token = fetch_token.get();
        let tenant = fetch_tenant.get();
        async move {
            transport::fetch_form_submissions(token, tenant)
                .await
                .map_err(|failure| failure.to_string())
        }
    });

    let action_token = token;
    let action_tenant = tenant;
    let transition = Callback::new(move |(id, state): (Uuid, String)| {
        let token = action_token.get_untracked();
        let tenant = action_tenant.get_untracked();
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match transport::set_form_submission_state(token, tenant, id, &state).await {
                Ok(_) => submissions.refetch(),
                Err(failure) => error.set(Some(failure.to_string())),
            }
            busy.set(false);
        });
    });

    view! {
        <section class="space-y-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <h1 class="text-xl font-semibold text-card-foreground">{labels.title.clone()}</h1>
                <span class="text-xs text-muted-foreground">
                    {labels.subtitle.clone()}
                </span>
            </div>
            {move || error.get().map(|message| view! {
                <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                    {message}
                </div>
            })}
            <Suspense fallback=|| view! {
                <div class="space-y-2" aria-label="Loading form submissions">
                    <div class="h-12 animate-pulse rounded-lg bg-muted"></div>
                    <div class="h-12 animate-pulse rounded-lg bg-muted"></div>
                </div>
            }>
                {
                    let empty_text = labels.empty.clone();
                    move || {
                        let empty_text = empty_text.clone();
                        submissions.get().map(|result| match result {
                            Ok(rows) if rows.is_empty() => view! {
                                <p class="text-sm text-muted-foreground">
                                    {empty_text}
                                </p>
                            }.into_any(),
                            Ok(rows) => view! {
                                <ul class="space-y-2">
                                    {rows
                                        .into_iter()
                                        .map(|row| {
                                            let id = row.id;
                                            let summary = summarize_submission(&row);
                                            let status = state_label(&row.state, None);
                                            let actions = allowed_transitions(&row.state, None);
                                            view! {
                                                <li class="rounded-lg bg-muted/40 px-3 py-2 text-sm">
                                                    <div class="flex flex-wrap items-center justify-between gap-3">
                                                        <div class="min-w-0">
                                                            <div class="truncate font-medium text-foreground">
                                                                {format!("{} · {} · {}", row.form_id, status, row.created_at)}
                                                            </div>
                                                            <div class="truncate text-xs text-muted-foreground">{summary}</div>
                                                        </div>
                                                        <div class="flex flex-wrap gap-2">
                                                            {actions
                                                                .into_iter()
                                                                .map(|action| {
                                                                    view! {
                                                                        <ActionButton
                                                                            label=action.label
                                                                            state=action.target_state
                                                                            id
                                                                            transition
                                                                            busy
                                                                        />
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </div>
                                                    </div>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            }.into_any(),
                            Err(message) => view! {
                                <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                                    {message}
                                </div>
                            }.into_any(),
                        })
                    }
                }
            </Suspense>
        </section>
    }
}

#[component]
fn ActionButton(
    label: String,
    state: &'static str,
    id: Uuid,
    transition: Callback<(Uuid, String), ()>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class="rounded-lg border border-border px-2.5 py-1.5 text-xs font-medium hover:bg-muted disabled:opacity-50"
            disabled=move || busy.get()
            on:click=move |_| transition.run((id, state.to_string()))
        >
            {label}
        </button>
    }
}
