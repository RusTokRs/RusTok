use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui::{Button, ButtonVariant, Size};
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::model::{ReactionAction, ReactionSnapshotView, ReactionSubjectUiRef};
use crate::transport;

#[component]
pub fn ReactionBar(subject: ReactionSubjectUiRef) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let label = t(locale.as_deref(), "reactions.label", "Reactions");
    let loading_label = t(
        locale.as_deref(),
        "reactions.loading",
        "Loading reactions\u{2026}",
    );
    let load_error_label = t(
        locale.as_deref(),
        "reactions.error.load",
        "Reactions are temporarily unavailable.",
    );
    let update_error_label = t(
        locale.as_deref(),
        "reactions.error.update",
        "Could not update this reaction.",
    );
    let sign_in_label = t(locale.as_deref(), "reactions.signIn", "Sign in to react.");
    let empty_label = t(
        locale.as_deref(),
        "reactions.empty",
        "No reactions are available for this item.",
    );
    let like_label = t(locale.as_deref(), "reactions.like", "Like");
    let selected_label = t(locale.as_deref(), "reactions.selected", "Selected");

    let transport_context = transport::current_reaction_storefront_transport_context();
    let resource_context = transport_context.clone();
    let mutation_context = transport_context;
    let resource_subject = subject.clone();
    let mutation_subject = subject;

    let (settled_snapshot, set_settled_snapshot) =
        signal(None::<Result<ReactionSnapshotView, String>>);
    let (mutation_busy, set_mutation_busy) = signal(false);
    let (mutation_error, set_mutation_error) = signal(false);

    let snapshot_resource = Resource::new_blocking(
        move || resource_subject.clone(),
        move |subject| {
            let context = resource_context.clone();
            async move { transport::load_reaction_snapshot(context, subject).await }
        },
    );

    let on_toggle = Callback::new(move |(reaction, action): (String, ReactionAction)| {
        if mutation_busy.get_untracked() {
            return;
        }
        let context = mutation_context.clone();
        let subject = mutation_subject.clone();
        set_mutation_busy.set(true);
        set_mutation_error.set(false);
        spawn_local(async move {
            let write_context = context.clone();
            let write_subject = subject.clone();
            match crate::core::settle_reaction_mutation(
                || transport::apply_reaction(write_context, write_subject, reaction, action),
                || transport::load_reaction_snapshot(context, subject),
            )
            .await
            {
                Ok(settled) => {
                    set_mutation_error.set(settled.write_failed);
                    set_settled_snapshot.set(Some(Ok(settled.snapshot)));
                }
                Err(_) => {
                    // A failed canonical reload removes stale interactive controls.
                    set_settled_snapshot.set(Some(Err(String::new())));
                }
            }
            set_mutation_busy.set(false);
        });
    });

    view! {
        <section class="space-y-2" aria-label=label.clone()>
            <div class="flex flex-wrap items-center gap-2">
                <span class="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                    {label.clone()}
                </span>
                {move || mutation_error.get().then(|| view! {
                    <span class="text-xs text-destructive" role="status" aria-live="polite">
                        {update_error_label.clone()}
                    </span>
                })}
            </div>

            <Suspense fallback=move || view! {
                <div class="inline-flex items-center rounded-full border border-border bg-muted/30 px-3 py-1.5 text-xs text-muted-foreground" role="status" aria-live="polite">
                    {loading_label.clone()}
                </div>
            }>
                {move || {
                    let load_error_label = load_error_label.clone();
                    let sign_in_label = sign_in_label.clone();
                    let empty_label = empty_label.clone();
                    let like_label = like_label.clone();
                    let selected_label = selected_label.clone();
                    let on_toggle = on_toggle;
                    Suspend::new(async move {
                        let initial = snapshot_resource.await.map_err(|_| String::new());
                        move || match settled_snapshot.get().unwrap_or_else(|| initial.clone()) {
                            Ok(snapshot) => view! {
                                <ReactionButtons
                                    snapshot
                                    mutation_busy
                                    on_toggle
                                    sign_in_label=sign_in_label.clone()
                                    empty_label=empty_label.clone()
                                    like_label=like_label.clone()
                                    selected_label=selected_label.clone()
                                />
                            }
                            .into_any(),
                            Err(_) => view! {
                                <p class="text-xs text-muted-foreground" role="status" aria-live="polite">
                                    {load_error_label.clone()}
                                </p>
                            }
                            .into_any(),
                        }
                    })
                }}
            </Suspense>
        </section>
    }
}

#[component]
fn ReactionButtons(
    snapshot: ReactionSnapshotView,
    mutation_busy: ReadSignal<bool>,
    on_toggle: Callback<(String, ReactionAction)>,
    sign_in_label: String,
    empty_label: String,
    like_label: String,
    selected_label: String,
) -> impl IntoView {
    if snapshot.catalog.keys.is_empty() {
        return view! {
            <p class="text-xs text-muted-foreground">{empty_label}</p>
        }
        .into_any();
    }

    let can_mutate = snapshot.can_mutate();
    let controls = snapshot
        .controls(&like_label, &selected_label)
        .into_iter()
        .map(|control| {
            let reaction_for_click = control.reaction;
            let action = control.action;
            let selected = control.selected;
            view! {
                <Button size=Size::Sm variant=ButtonVariant::Outline
                    attr:aria-pressed=selected.to_string() attr:aria-label=control.aria_label
                    attr:aria-busy=move || mutation_busy.get().to_string()
                    disabled=Signal::derive(move || !can_mutate || mutation_busy.get())
                    on_click=Box::new(move || on_toggle.run((reaction_for_click.clone(), action)))>
                    <span>{control.label}</span>
                    <span class="tabular-nums text-xs text-muted-foreground">{control.count}</span>
                </Button>
            }
        })
        .collect_view();

    view! {
        <div class="space-y-2">
            <div class="flex flex-wrap gap-2">{controls}</div>
            {(!can_mutate).then(|| view! {
                <p class="text-xs text-muted-foreground">{sign_in_label}</p>
            })}
        </div>
    }
    .into_any()
}
