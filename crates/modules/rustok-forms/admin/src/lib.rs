//! Leptos admin surface for the forms module: submission triage inbox.

mod model;
mod transport;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use uuid::Uuid;

pub use model::FormSubmission;

/// Inbox listing form submissions with triage actions.
#[component]
pub fn FormsAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
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
                <h1 class="text-xl font-semibold text-card-foreground">"Form submissions"</h1>
                <span class="text-xs text-muted-foreground">
                    "newest first; honeypot captures are stored as spam"
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
                {move || {
                    submissions.get().map(|result| match result {
                        Ok(rows) if rows.is_empty() => view! {
                            <p class="text-sm text-muted-foreground">
                                "No form submissions yet."
                            </p>
                        }.into_any(),
                        Ok(rows) => view! {
                            <ul class="space-y-2">
                                {rows
                                    .into_iter()
                                    .map(|row| {
                                        let id = row.id;
                                        let summary = summarize(&row);
                                        let pending = row.state == "new" || row.state == "read";
                                        view! {
                                            <li class="rounded-lg bg-muted/40 px-3 py-2 text-sm">
                                                <div class="flex flex-wrap items-center justify-between gap-3">
                                                    <div class="min-w-0">
                                                        <div class="truncate font-medium text-foreground">
                                                            {format!("{} · {} · {}", row.form_id, row.state, row.created_at)}
                                                        </div>
                                                        <div class="truncate text-xs text-muted-foreground">{summary}</div>
                                                    </div>
                                                    <Show when=move || pending>
                                                        <div class="flex flex-wrap gap-2">
                                                            <ActionButton label="Mark read" state="read" id transition busy />
                                                            <ActionButton label="Mark handled" state="handled" id transition busy />
                                                            <ActionButton label="Mark spam" state="spam" id transition busy />
                                                        </div>
                                                    </Show>
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
                }}
            </Suspense>
        </section>
    }
}

#[component]
fn ActionButton(
    label: &'static str,
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

fn summarize(row: &FormSubmission) -> String {
    let mut parts = Vec::new();
    if let Some(page_id) = row.page_id {
        parts.push(format!("page {page_id}"));
    }
    if let Some(value) = row.payload.as_object() {
        for (name, field) in value.iter().take(6) {
            let text = match field {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            parts.push(format!(
                "{name}={}",
                text.chars().take(40).collect::<String>()
            ));
        }
    }
    if parts.is_empty() {
        "(no fields)".to_string()
    } else {
        parts.join(" · ")
    }
}
