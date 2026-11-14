use crate::access::effective_permissions_grant;
use crate::i18n::t;
use crate::model::{PageStarterMetadataUpdate, PageStarterSummary};
use crate::transport;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_auth_ui::ui::leptos::hooks::{use_current_user, use_tenant, use_token};

// The selection keeps the observed revision and request context, not merely the key.
type Selection = (String, String, Option<String>, Option<String>);

#[component]
pub(crate) fn StarterCatalogManager(
    starter_generation: RwSignal<u64>,
    default_locale: String,
) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let user = use_current_user();
    let allowed = Signal::derive(move || user.get().is_some_and(|user|
        effective_permissions_grant(&user.permissions, "pages:manage")));
    let text = |key, fallback| t(Some(&default_locale), key, fallback);
    let title = text("pages.starter.manageTitle", "Manage starter catalog");
    let hint = text("pages.starter.manageHint", "Only tenant starters can be changed. The document and thumbnail are preserved; suggested layouts are not applied automatically.");
    let built_in = text("pages.starter.builtin", "Built-in (read only)");
    let tenant_label = text("pages.starter.tenant", "Tenant starter");
    let edit_label = text("pages.starter.edit", "Edit");
    let name_label = text("pages.starter.name", "Starter name");
    let category_label = text("pages.starter.category", "Category");
    let description_label = text("pages.starter.description", "Description");
    let layout_label = text("pages.starter.layoutKey", "Suggested layout key");
    let save_label = text("pages.starter.saveChanges", "Save changes");
    let delete_label = text("pages.starter.delete", "Delete starter");
    let delete_warning = text("pages.starter.deleteWarning", "Delete this tenant starter? If it overrides a built-in, the built-in will become visible again.");
    let confirm_label = text("pages.starter.confirmDelete", "Confirm deletion");
    let cancel_label = text("pages.starter.cancel", "Cancel");
    let reload_label = text("pages.starter.reload", "Reload catalog and discard edits");
    let context_error = text("pages.starter.contextError", "Session or tenant changed. Reload and try again.");
    let saved_message = text("pages.starter.updated", "Starter updated");
    let deleted_message = text("pages.starter.deleted", "Starter deleted");
    let not_deleted = text("pages.starter.notDeleted", "Starter was not deleted");
    let operation_error = text("pages.starter.operationError", "Changes were not applied. Edits are retained; reload the catalog to resolve a revision conflict.");
    let loading = text("pages.starter.loading", "Loading starters...");
    let unavailable = text("pages.starter.unavailable", "Starter catalog unavailable");

    let selection = RwSignal::new(None::<Selection>);
    let name = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let layout = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let confirming = RwSignal::new(false);
    let feedback = RwSignal::new(None::<(bool, String)>);

    let catalog = Resource::new(
        move || (token.get(), tenant.get(), starter_generation.get(), allowed.get()),
        move |(token, tenant, generation, allowed)| async move {
            let result: Result<Vec<PageStarterSummary>, String> = if allowed {
                transport::fetch_page_starters(token, tenant.clone()).await.map_err(|err| err.to_string())
            } else {
                Ok(Vec::new())
            };
            (tenant, generation, result)
        },
    );

    let save = Callback::new({
        let context_error = context_error.clone();
        let operation_error = operation_error.clone();
        move |event: SubmitEvent| {
            event.prevent_default();
            if busy.get_untracked() || !allowed.get_untracked() { return; }
            let Some((key, revision, chosen_token, chosen_tenant)) = selection.get_untracked() else { return; };
            if token.get_untracked() != chosen_token || tenant.get_untracked() != chosen_tenant {
                feedback.set(Some((false, context_error.clone())));
                return;
            }
            let description_value = description.get_untracked();
            let input = PageStarterMetadataUpdate {
                key, expected_revision: revision, name: name.get_untracked(),
                category: category.get_untracked(),
                description: (!description_value.trim().is_empty()).then_some(description_value),
                default_template_key: layout.get_untracked(),
            };
            let context_error = context_error.clone();
            let operation_error = operation_error.clone();
            let saved_message = saved_message.clone();
            busy.set(true);
            feedback.set(None);
            spawn_local(async move {
                let result = transport::update_starter_metadata(chosen_token.clone(), chosen_tenant.clone(), input).await;
                if token.get_untracked() != chosen_token || tenant.get_untracked() != chosen_tenant {
                    feedback.set(Some((false, context_error)));
                } else {
                    match result {
                        Ok(_) => {
                            selection.set(None);
                            confirming.set(false);
                            feedback.set(Some((true, saved_message)));
                            starter_generation.update(|generation| *generation = generation.wrapping_add(1));
                        }
                        Err(error) => feedback.set(Some((false, format!("{operation_error}: {error}")))),
                    }
                }
                busy.set(false);
            });
        }
    });

    let delete = Callback::new({
        let context_error = context_error.clone();
        let operation_error = operation_error.clone();
        move |()| {
            if busy.get_untracked() || !allowed.get_untracked() || !confirming.get_untracked() { return; }
            let Some((key, revision, chosen_token, chosen_tenant)) = selection.get_untracked() else { return; };
            if token.get_untracked() != chosen_token || tenant.get_untracked() != chosen_tenant {
                feedback.set(Some((false, context_error.clone())));
                return;
            }
            let context_error = context_error.clone();
            let operation_error = operation_error.clone();
            let deleted_message = deleted_message.clone();
            let not_deleted = not_deleted.clone();
            busy.set(true);
            feedback.set(None);
            spawn_local(async move {
                let result = transport::delete_starter(chosen_token.clone(), chosen_tenant.clone(), key, revision).await;
                if token.get_untracked() != chosen_token || tenant.get_untracked() != chosen_tenant {
                    feedback.set(Some((false, context_error)));
                } else {
                    match result {
                        Ok(true) => {
                            selection.set(None);
                            confirming.set(false);
                            feedback.set(Some((true, deleted_message)));
                            starter_generation.update(|generation| *generation = generation.wrapping_add(1));
                        }
                        Ok(false) => feedback.set(Some((false, not_deleted))),
                        Err(error) => feedback.set(Some((false, format!("{operation_error}: {error}")))),
                    }
                }
                busy.set(false);
            });
        }
    });

    view! {
        <section hidden=move || !allowed.get() class="rounded-2xl border border-border bg-card p-4 shadow-sm">
            <h2 class="font-semibold">{title}</h2>
            <p class="mt-1 text-xs text-muted-foreground">{hint}</p>
            <button type="button" class="mt-2 text-xs underline" disabled=move || busy.get() on:click=move |_| {
                selection.set(None); confirming.set(false); feedback.set(None);
                starter_generation.update(|generation| *generation = generation.wrapping_add(1));
            }>{reload_label}</button>
            <Suspense fallback=move || view! { <p>{loading.clone()}</p> }>
                {move || catalog.get().map(|(loaded_tenant, generation, result)| {
                    if loaded_tenant != tenant.get() || generation != starter_generation.get() {
                        return view! { <p role="alert">{unavailable.clone()}</p> }.into_any();
                    }
                    match result {
                        Err(error) => view! { <p role="alert">{format!("{}: {}", unavailable, error)}</p> }.into_any(),
                        Ok(items) => view! {
                            <ul class="mt-3 space-y-2">
                                {items.into_iter().map(|item| {
                                    let display = format!("{} ({})", item.name, item.key);
                                    let revision = item.revision.clone();
                                    let select_item = item.clone();
                                    let label = if revision.is_some() { tenant_label.clone() } else { built_in.clone() };
                                    let edit = edit_label.clone();
                                    view! {
                                        <li class="rounded-lg border border-border p-2 text-sm">
                                            <div>{display}</div><div class="text-xs text-muted-foreground">{label}</div>
                                            {revision.map(|revision| view! {
                                                <button type="button" class="text-xs underline" disabled=move || busy.get() on:click=move |_| {
                                                    let item = select_item.clone();
                                                    name.set(item.name); category.set(item.category);
                                                    description.set(item.description.unwrap_or_default());
                                                    layout.set(item.default_template_key);
                                                    selection.set(Some((item.key, revision.clone(), token.get_untracked(), tenant.get_untracked())));
                                                    confirming.set(false); feedback.set(None);
                                                }>{edit}</button>
                                            })}
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any(),
                    }
                })}
            </Suspense>
            {move || selection.get().map(|(key, _, _, _)| {
                let name_label = name_label.clone(); let category_label = category_label.clone();
                let description_label = description_label.clone(); let layout_label = layout_label.clone();
                let save_label = save_label.clone(); let delete_label = delete_label.clone();
                let delete_warning = delete_warning.clone(); let confirm_label = confirm_label.clone();
                let cancel_label = cancel_label.clone();
                view! {
                    <form class="mt-3 space-y-2" on:submit=move |event| save.run(event)>
                        <p class="text-xs font-semibold">{key}</p>
                        <label class="block text-xs">{name_label}<input required disabled=move || busy.get() class="w-full rounded border border-input bg-background p-2" prop:value=move || name.get() on:input=move |event| name.set(event_target_value(&event)) /></label>
                        <label class="block text-xs">{category_label}<input required disabled=move || busy.get() class="w-full rounded border border-input bg-background p-2" prop:value=move || category.get() on:input=move |event| category.set(event_target_value(&event)) /></label>
                        <label class="block text-xs">{description_label}<textarea disabled=move || busy.get() class="w-full rounded border border-input bg-background p-2" prop:value=move || description.get() on:input=move |event| description.set(event_target_value(&event)) /></label>
                        <label class="block text-xs">{layout_label}<input required disabled=move || busy.get() class="w-full rounded border border-input bg-background p-2" prop:value=move || layout.get() on:input=move |event| layout.set(event_target_value(&event)) /></label>
                        <button type="submit" disabled=move || busy.get() class="rounded bg-primary px-3 py-2 text-xs text-primary-foreground disabled:opacity-50">{save_label}</button>
                        <button type="button" disabled=move || busy.get() class="ml-2 text-xs text-destructive underline" on:click=move |_| confirming.set(true)>{delete_label}</button>
                        {move || confirming.get().then(|| view! {
                            <div class="rounded border border-destructive/40 p-2 text-xs" role="alert">
                                <p>{delete_warning.clone()}</p>
                                <button type="button" disabled=move || busy.get() class="mt-2 text-destructive underline" on:click=move |_| delete.run(())>{confirm_label.clone()}</button>
                                <button type="button" disabled=move || busy.get() class="ml-2 underline" on:click=move |_| confirming.set(false)>{cancel_label.clone()}</button>
                            </div>
                        })}
                    </form>
                }
            })}
            {move || feedback.get().map(|(ok, message)| view! { <p class="mt-2 text-xs" role=if ok { "status" } else { "alert" }>{message}</p> })}
        </section>
    }
}
