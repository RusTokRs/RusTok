use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::features::workflow::api::{self, WorkflowVersionSummaryDto};

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn VersionHistory(
    workflow_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
    /// Called after a version is restored
    on_restored: Callback<()>,
) -> impl IntoView {
    let (restoring, set_restoring) = signal(Option::<i32>::None);

    let wf_id = workflow_id.clone();
    let tok = token.clone();
    let ts = tenant_slug.clone();
    let versions_resource = local_resource(
        move || (tok.clone(), ts.clone(), wf_id.clone()),
        move |(tok, ts, id)| async move { api::fetch_versions(tok, ts, id).await },
    );

    view! {
        <div class="space-y-2">
            <h3 class="text-sm font-semibold text-foreground">"Version History"</h3>

            <Suspense fallback=move || view! {
                <div class="h-24 animate-pulse rounded-xl bg-muted"></div>
            }>
                {move || {
                    let restoring_ver = restoring.get();

                    versions_resource.get().map(|result| match result {
                        Err(e) => view! {
                            <div class="text-xs text-destructive">{e.to_string()}</div>
                        }.into_any(),
                        Ok(versions) if versions.is_empty() => view! {
                            <div class="rounded-lg border border-dashed border-border px-4 py-6 text-center text-xs text-muted-foreground">
                                "No saved versions yet. Versions are created automatically when you edit a workflow."
                            </div>
                        }.into_any(),
                        Ok(versions) => view! {
                            <div class="divide-y divide-border rounded-xl border border-border overflow-hidden">
                                {versions.iter().map(|v| {
                                    let ver = v.version;
                                    let wf_id2 = workflow_id.clone();
                                    let tok2 = token.clone();
                                    let ts2 = tenant_slug.clone();
                                    let is_restoring = restoring_ver == Some(ver);

                                    view! {
                                        <VersionRow
                                            version=v.clone()
                                            is_restoring=is_restoring
                                            on_restore=Callback::new(move |_| {
                                                let wf_id3 = wf_id2.clone();
                                                let tok3 = tok2.clone();
                                                let ts3 = ts2.clone();
                                                let on_restored = on_restored;
                                                set_restoring.set(Some(ver));

                                                spawn_local(async move {
                                                    match api::restore_version(tok3, ts3, wf_id3, ver).await {
                                                        Ok(_) => {
                                                            set_restoring.set(None);
                                                            on_restored.run(());
                                                        }
                                                        Err(_) => {
                                                            set_restoring.set(None);
                                                        }
                                                    }
                                                });
                                            })
                                        />
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn VersionRow(
    version: WorkflowVersionSummaryDto,
    is_restoring: bool,
    on_restore: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between px-4 py-2 text-sm bg-card hover:bg-muted/40">
            <div class="flex items-center gap-3">
                <span class="inline-flex h-6 w-6 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary">
                    {version.version}
                </span>
                <span class="text-xs text-muted-foreground">{version.created_at.clone()}</span>
            </div>
            <button
                on:click=move |_| on_restore.run(())
                disabled=is_restoring
                class="rounded-md border border-border px-2 py-0.5 text-xs hover:bg-muted disabled:opacity-50"
            >
                {if is_restoring { "Restoring…" } else { "Restore" }}
            </button>
        </div>
    }
}
