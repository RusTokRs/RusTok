/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

use crate::catalog_transport;
use crate::core::attribute_lifecycle::{
    self as core, AttributeWriteScope, DefinitionPhase, DefinitionRecord,
};
use crate::core::write_journal::{JournalScope, JournalState};
use crate::model::{
    AttributeDefinitionAction, AttributeLifecycleRequest, AttributeLifecycleWorkspace,
};
use crate::transport::write_journal;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui::{Alert, AlertVariant, Button, Label, Select, SelectOption, Textarea};
use leptos_ui_routing::use_route_query_value;
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

#[derive(Clone)]
struct LoadedScope {
    tenant_id: String,
    actor_id: String,
    token: Option<String>,
    tenant: Option<String>,
    record: Option<DefinitionRecord>,
}
#[derive(Clone)]
struct LoadedWorkspace {
    target: String,
    tenant_id: String,
    actor_id: String,
    token: Option<String>,
    tenant: Option<String>,
    value: AttributeLifecycleWorkspace,
}
impl LoadedWorkspace {
    fn matches(&self, scope: &LoadedScope) -> bool {
        self.tenant_id == scope.tenant_id
            && self.actor_id == scope.actor_id
            && self.token == scope.token
            && self.tenant == scope.tenant
    }
}

/// Recovery is mounted independently of both directory and selected-definition reads.
#[component]
pub(super) fn AttributeLifecyclePanel(
    parent_busy: Signal<bool>,
    on_lock_change: Callback<bool>,
) -> impl IntoView {
    let context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = StoredValue::new(context.locale.clone());
    let text = move |key: &'static str| crate::i18n::t(locale.get_value().as_deref(), key, key);
    let selected_attribute = StoredValue::new(
        use_route_query_value(AdminQueryKey::AttributeId.as_str()).get_untracked(),
    );
    let route_option =
        StoredValue::new(use_route_query_value(AdminQueryKey::OptionId.as_str()).get_untracked());
    let initial_target = StoredValue::new(
        route_option
            .get_value()
            .unwrap_or_else(|| "attribute".into()),
    );
    let base = StoredValue::new(context.admin_module_route_base("product"));
    let token = use_token();
    let tenant = use_tenant();
    let record = RwSignal::new(None::<DefinitionRecord>);
    let storage_error = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let auth_nonce = RwSignal::new(0_u64);
    let read_nonce = RwSignal::new(0_u64);
    let error = RwSignal::new(None::<String>);
    let (directory_selection, set_directory_selection) =
        signal(selected_attribute.get_value().unwrap_or_default());
    let (selected_target, set_selected_target) = signal(initial_target.get_value());
    let (patch, set_patch) = signal(String::new());
    Effect::new(move |_| {
        token.track();
        tenant.track();
        busy.set(false);
    });

    let recovery = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let generation = auth_nonce.get();
        let attr = selected_attribute.get_value();
        let loc = locale.get_value();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|_| ())?;
            if token.try_get_untracked() != Some(tok.clone())
                || tenant.try_get_untracked() != Some(ten.clone())
                || auth_nonce.try_get_untracked() != Some(generation)
            {
                return Err(());
            }
            if record.try_get_untracked().flatten().is_some_and(|saved| {
                saved.scope.tenant_id != bootstrap.current_tenant.id
                    || saved.scope.actor_id != bootstrap.me.id
            }) {
                return Err(());
            }
            core::selected_option(&initial_target.get_value())?;
            write_journal::validate_definition_document_selection(
                attr.as_deref(),
                route_option.get_value().as_deref(),
            )
            .map_err(|_| ())?;
            write_journal::validate_recovery_selection::<DefinitionPhase>(attr.is_some())
                .map_err(|_| ())?;
            let saved = match attr {
                Some(attribute_id) => Some(
                    write_journal::load::<DefinitionPhase>(&AttributeWriteScope {
                        tenant_id: bootstrap.current_tenant.id.clone(),
                        actor_id: bootstrap.me.id.clone(),
                        attribute_id,
                        locale: loc.ok_or(())?,
                    })
                    .map_err(|_| ())?,
                ),
                None => None,
            };
            Ok::<_, ()>(LoadedScope {
                tenant_id: bootstrap.current_tenant.id,
                actor_id: bootstrap.me.id,
                token: tok,
                tenant: ten,
                record: saved,
            })
        }
    });
    Effect::new(move |_| {
        let Some(result) = recovery.get() else {
            return;
        };
        match result {
            Ok(loaded) if loaded.token == token.get() && loaded.tenant == tenant.get() => {
                if let Some(live) = loaded.record {
                    if record
                        .get_untracked()
                        .as_ref()
                        .is_some_and(|old| !old.can_restore(&live))
                    {
                        storage_error.set(true);
                        return;
                    }
                    set_selected_target.set(
                        live.state
                            .request()
                            .map(|(_, input)| {
                                input
                                    .option_id
                                    .clone()
                                    .unwrap_or_else(|| "attribute".into())
                            })
                            .unwrap_or_else(|| initial_target.get_value()),
                    );
                    record.set(Some(live));
                    set_directory_selection.set(selected_attribute.get_value().unwrap_or_default());
                }
                storage_error.set(false);
            }
            _ => storage_error.set(true),
        }
    });
    let authenticated = Signal::derive(move || {
        recovery
            .get()
            .and_then(Result::ok)
            .is_some_and(|loaded| loaded.token == token.get() && loaded.tenant == tenant.get())
    });

    let directory = LocalResource::new(move || {
        let loaded = recovery.get().and_then(Result::ok);
        let generation = read_nonce.get();
        let loc = locale.get_value();
        async move {
            let loaded = loaded.ok_or(())?;
            let value = catalog_transport::read_attribute_lifecycle(
                loaded.token.clone(),
                loaded.tenant.clone(),
                loaded.tenant_id.clone(),
                loaded.actor_id.clone(),
                loc.ok_or(())?,
                AttributeLifecycleRequest {
                    attribute_id: None,
                    option_id: None,
                    include_archived: true,
                },
            )
            .await?;
            if token.try_get_untracked() != Some(loaded.token.clone())
                || tenant.try_get_untracked() != Some(loaded.tenant.clone())
                || read_nonce.try_get_untracked() != Some(generation)
            {
                return Err(());
            }
            Ok::<_, ()>(LoadedWorkspace {
                target: String::new(),
                tenant_id: loaded.tenant_id,
                actor_id: loaded.actor_id,
                token: loaded.token,
                tenant: loaded.tenant,
                value,
            })
        }
    });
    let workspace = LocalResource::new(move || {
        let loaded = recovery.get().and_then(Result::ok);
        let generation = read_nonce.get();
        let target = selected_target.get();
        let attribute_id = selected_attribute.get_value();
        let loc = locale.get_value();
        async move {
            let loaded = loaded.ok_or(())?;
            let Some(attribute_id) = attribute_id else {
                return Ok(None);
            };
            let option_id = core::selected_option(&target)?;
            let value = catalog_transport::read_attribute_lifecycle(
                loaded.token.clone(),
                loaded.tenant.clone(),
                loaded.tenant_id.clone(),
                loaded.actor_id.clone(),
                loc.ok_or(())?,
                AttributeLifecycleRequest {
                    attribute_id: Some(attribute_id),
                    option_id,
                    include_archived: true,
                },
            )
            .await?;
            if token.try_get_untracked() != Some(loaded.token.clone())
                || tenant.try_get_untracked() != Some(loaded.tenant.clone())
                || read_nonce.try_get_untracked() != Some(generation)
                || selected_target.try_get_untracked() != Some(target.clone())
            {
                return Err(());
            }
            Ok::<_, ()>(Some(LoadedWorkspace {
                target,
                tenant_id: loaded.tenant_id,
                actor_id: loaded.actor_id,
                token: loaded.token,
                tenant: loaded.tenant,
                value,
            }))
        }
    });
    let current_directory = Memo::new(move |_| {
        let auth = recovery.get().and_then(Result::ok)?;
        directory
            .get()
            .and_then(Result::ok)
            .filter(|loaded| loaded.matches(&auth) && authenticated.get())
            .map(|loaded| loaded.value)
    });
    let current_workspace = Memo::new(move |_| {
        let auth = recovery.get().and_then(Result::ok)?;
        workspace
            .get()
            .and_then(Result::ok)
            .flatten()
            .filter(|loaded| {
                loaded.matches(&auth)
                    && loaded.target == selected_target.get()
                    && authenticated.get()
            })
            .map(|loaded| loaded.value)
    });
    Effect::new(move |_| {
        if let Some(value) = current_workspace.get() {
            if let Ok(option_id) = core::selected_option(&selected_target.get_untracked()) {
                set_patch
                    .set(core::presentation_json(&value, option_id.as_deref()).unwrap_or_default());
            }
        }
    });
    let controls = Signal::derive(move || {
        let workspace = current_workspace.get();
        let target = workspace
            .as_ref()
            .and_then(|workspace| core::target_archived(workspace, &selected_target.get()));
        core::controls(
            record.get().as_ref(),
            selected_attribute.get_value().is_some(),
            authenticated.get(),
            storage_error.get(),
            busy.get(),
            parent_busy.get(),
            workspace
                .as_ref()
                .is_some_and(|workspace| workspace.attributes.len() == 1),
            target,
        )
    });
    Effect::new(move |_| on_lock_change.run(controls.get().lock_peers));

    Effect::new(move |_| {
        let value = directory_selection.get();
        if selected_attribute.get_value().as_deref() == Some(value.as_str()) || value.is_empty() {
            return;
        }
        if !authenticated.get_untracked()
            || busy.get_untracked()
            || storage_error.get_untracked()
            || record
                .get_untracked()
                .is_some_and(|record| matches!(record.state, DefinitionPhase::Pending { .. }))
        {
            return;
        }
        let result = core::selection_href(&base.get_value(), &value, None, None).and_then(|href| {
            if let Some(saved) = record.get_untracked() {
                write_journal::verify(&saved).map_err(|_| ())?;
            }
            busy.set(true);
            write_journal::navigate_owner_document(&href).map_err(|_| ())
        });
        if result.is_err() {
            busy.set(false);
            storage_error.set(true);
            error.set(Some(text("product-attributeLifecycle-selection")));
        }
    });

    Effect::new(move |_| {
        let target = selected_target.get();
        if target == initial_target.get_value() {
            return;
        }
        let Some(saved) = record
            .get_untracked()
            .filter(|saved| saved.state.is_ready())
        else {
            return;
        };
        if !authenticated.get_untracked()
            || busy.get_untracked()
            || storage_error.get_untracked()
            || parent_busy.get_untracked()
        {
            return;
        }
        let result = core::selected_option(&target).and_then(|option_id| {
            write_journal::verify(&saved).map_err(|_| ())?;
            let href = core::selection_href(
                &base.get_value(),
                &saved.scope.attribute_id,
                option_id.as_deref(),
                Some(saved.document_intent),
            )?;
            busy.set(true);
            write_journal::navigate_owner_document(&href).map_err(|_| ())
        });
        if result.is_err() {
            busy.set(false);
            storage_error.set(true);
        }
    });

    let verify_document = move || {
        write_journal::validate_definition_document_selection(
            selected_attribute.get_value().as_deref(),
            route_option.get_value().as_deref(),
        )
    };
    let send = Callback::new(move |action: Option<AttributeDefinitionAction>| {
        let admitted = controls.get_untracked();
        let allowed = match action {
            Some(AttributeDefinitionAction::Update) => admitted.editable,
            Some(AttributeDefinitionAction::Archive) => admitted.archive,
            Some(AttributeDefinitionAction::Restore) => admitted.restore,
            None => admitted.recover,
        };
        if !allowed {
            return;
        }
        if verify_document().is_err() {
            storage_error.set(true);
            return;
        }
        let Some(saved) = record.get_untracked() else {
            return;
        };
        let Some(auth) = recovery.get_untracked().and_then(Result::ok) else {
            return;
        };
        let expected = if let Some(action) = action {
            let Some(workspace) = current_workspace.get_untracked() else {
                return;
            };
            let Ok(option_id) = core::selected_option(&selected_target.get_untracked()) else {
                return;
            };
            let Some(loc) = locale.get_value() else {
                return;
            };
            match core::prepare(
                &saved.scope.for_locale(loc),
                &workspace,
                option_id,
                action,
                &patch.get_untracked(),
            )
            .and_then(|input| saved.begin_definition(input).map_err(|_| ()))
            {
                Ok(next) => {
                    if write_journal::store(&saved, &next).is_err() {
                        storage_error.set(true);
                        return;
                    }
                    record.set(Some(next.clone()));
                    next
                }
                Err(()) => {
                    error.set(Some(text("product-attributeLifecycle-invalid")));
                    return;
                }
            }
        } else {
            if write_journal::verify(&saved).is_err() {
                storage_error.set(true);
                return;
            }
            saved
        };
        let Some((key, input)) = expected.state.request() else {
            return;
        };
        let key = key.to_owned();
        let input = input.clone();
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            let outcome = catalog_transport::update_attribute_definition(
                auth.token.clone(),
                auth.tenant.clone(),
                expected.scope.clone(),
                key,
                input,
            )
            .await;
            // An old reply must not mutate the busy/storage state of a newer generation.
            if record.try_get_untracked().flatten().as_ref() != Some(&expected) {
                return;
            }
            if token.try_get_untracked() != Some(auth.token)
                || tenant.try_get_untracked() != Some(auth.tenant)
            {
                return;
            }
            if let Ok(next) = expected.settle_definition(outcome) {
                if write_journal::store(&expected, &next).is_ok() {
                    record.set(Some(next));
                } else {
                    storage_error.set(true);
                }
            }
            busy.set(false);
        });
    });
    let fresh = Callback::new(move |_: ()| {
        if !controls.get_untracked().fresh {
            return;
        }
        let Some(saved) = record.get_untracked() else {
            return;
        };
        let Some(loc) = locale.get_value() else {
            return;
        };
        if verify_document().is_err() {
            storage_error.set(true);
            return;
        }
        busy.set(true);
        let result = write_journal::read::<DefinitionPhase>(&saved.scope).and_then(|live| {
            let next = saved.fresh_from_live(&live, loc)?;
            let (_, input) = saved
                .state
                .request()
                .ok_or(crate::core::write_journal::WriteJournalError::Invalid)?;
            let href = core::selection_href(
                &base.get_value(),
                &saved.scope.attribute_id,
                input.option_id.as_deref(),
                Some(saved.document_intent),
            )
            .map_err(|_| crate::core::write_journal::WriteJournalError::Invalid)?;
            write_journal::replace_owner_document_selection(&href)?;
            if live == saved {
                write_journal::store(&saved, &next)?;
            }
            write_journal::navigate_owner_document(&href)
        });
        if result.is_err() {
            busy.set(false);
            storage_error.set(true);
        }
    });

    view! {
        <section class="rounded-xl border border-border bg-card p-6 space-y-4" aria-busy=move || busy.get()>
            <h2 class="text-lg font-semibold">{text("product-attributeLifecycle-title")}</h2>
            <p class="text-sm text-muted-foreground">{text("product-attributeLifecycle-help")}</p>
            <Show when=move || storage_error.get()><Alert variant=AlertVariant::Warning>{text("product-attributeLifecycle-storage")}</Alert></Show>
            <Show when=move || storage_error.get()>
                {move || view! { <Button on_click=Box::new(move || {
                    if record.get_untracked().is_some_and(|saved| saved.state.is_terminal()) {
                        if write_journal::reload_owner_document().is_err() { storage_error.set(true); }
                    } else { auth_nonce.update(|value| *value = value.wrapping_add(1)); }
                }) disabled=busy.get()>{text("product-attributeLifecycle-recoverStorage")}</Button> }}
            </Show>
            <Show when=move || error.get().is_some()><Alert variant=AlertVariant::Warning>{move || error.get()}</Alert></Show>
            <Show when=move || directory.get().is_some_and(|value| value.is_err()) || workspace.get().is_some_and(|value| value.is_err())>
                <Alert variant=AlertVariant::Warning>{text("product-attributeLifecycle-unavailable")}</Alert>
            </Show>
            <div class="flex flex-wrap gap-3 items-end">
                <div class="flex-1 space-y-2">
                    <Label r#for="attribute-definition-directory">{text("product-attributeLifecycle-directory")}</Label>
                    {move || {
                        let options: Vec<SelectOption> = current_directory.get().map(|workspace| core::directory_options(&workspace, &text("product-attributeLifecycle-archived"), &text("product-attributeLifecycle-active"))).unwrap_or_default().into_iter().map(|(value, label)| SelectOption::new(value, label)).collect();
                        view! { <Select id="attribute-definition-directory" options=options value=directory_selection set_value=set_directory_selection placeholder=text("product-attributeLifecycle-choose") disabled=!authenticated.get() || busy.get() || storage_error.get() || record.get().is_some_and(|record| matches!(record.state, DefinitionPhase::Pending { .. })) /> }
                    }}
                </div>
                {move || view! { <Button on_click=Box::new(move || read_nonce.update(|value| *value = value.wrapping_add(1))) disabled=!controls.get().refresh>{text("product-attributeLifecycle-refresh")}</Button> }}
            </div>
            <Show when=move || selected_attribute.get_value().is_some() && authenticated.get()>
                <p class="break-all font-mono text-xs">{move || selected_attribute.get_value()}{move || (selected_target.get() != "attribute").then(|| format!(" / {}", selected_target.get()))}</p>
                {move || record.get().and_then(|record| core::record_message_key(&record)).map(|key| view! { <p role="status">{text(key)}</p> })}
                {move || current_workspace.get().map(|workspace| {
                    let options: Vec<SelectOption> = core::target_options(&workspace, &selected_target.get_untracked(), &text("product-attributeLifecycle-attribute"), &text("product-attributeLifecycle-archived"), &text("product-attributeLifecycle-active"), &text("product-attributeLifecycle-notFound")).into_iter().map(|(value, label)| {
                        let missing = core::target_archived(&workspace, &value).is_none();
                        let option = SelectOption::new(value, label);
                        if missing { option.disabled() } else { option }
                    }).collect();
                    view! {
                        <Label r#for="attribute-definition-target">{text("product-attributeLifecycle-target")}</Label>
                        <Select id="attribute-definition-target" options=options value=selected_target set_value=set_selected_target disabled=!controls.get().choose_target />
                    }
                })}
                <Show when=move || current_workspace.get().is_some_and(|workspace| core::target_archived(&workspace, &selected_target.get()).is_none())>
                    <p role="status">{text("product-attributeLifecycle-notFound")}" · "{move || selected_target.get()}</p>
                </Show>
                <Show when=move || record.get().is_some_and(|record| record.state.is_ready()) && current_workspace.get().is_some_and(|workspace| core::target_archived(&workspace, &selected_target.get()).is_some())>
                    <Label r#for="attribute-definition-patch">{text("product-attributeLifecycle-patch")}</Label>
                    {move || view! { <Textarea id="attribute-definition-patch" class="font-mono text-xs min-h-64" value=patch set_value=set_patch disabled=!controls.get().editable /> }}
                    <p class="text-sm text-muted-foreground">{text("product-attributeLifecycle-patchHelp")}</p>
                </Show>
                {move || {
                    let impact = record.get().and_then(|record| core::recorded_impact(&record)).or_else(|| current_workspace.get().and_then(|workspace| workspace.impact));
                    impact.map(|impact| view! {
                        <p>{text("product-attributeLifecycle-impact")}</p>
                        <ul class="list-inside list-disc text-sm">{impact.references.into_iter().map(|reference| view! { <li>{text(core::reference_key(reference.kind))}" · "<code class="break-all">{reference.resource_id}</code></li> }).collect_view()}</ul>
                        {impact.has_more.then(|| view! { <p>{text("product-attributeLifecycle-more")}</p> })}
                        {(!impact.inspection_complete).then(|| view! { <p>{text("product-attributeLifecycle-limit")}</p> })}
                    })
                }}
                <div class="flex flex-wrap gap-2">
                    {move || view! { <Button on_click=Box::new(move || send.run(Some(AttributeDefinitionAction::Update))) disabled=!controls.get().editable>{text("product-attributeLifecycle-update")}</Button> }}
                    {move || view! { <Button on_click=Box::new(move || send.run(Some(AttributeDefinitionAction::Archive))) disabled=!controls.get().archive>{text("product-attributeLifecycle-archive")}</Button> }}
                    {move || view! { <Button on_click=Box::new(move || send.run(Some(AttributeDefinitionAction::Restore))) disabled=!controls.get().restore>{text("product-attributeLifecycle-restore")}</Button> }}
                    {move || view! { <Button on_click=Box::new(move || send.run(None)) disabled=!controls.get().recover>{text("product-attributeLifecycle-recover")}</Button> }}
                    {move || view! { <Button on_click=Box::new(move || fresh.run(())) disabled=!controls.get().fresh>{text("product-attributeLifecycle-fresh")}</Button> }}

                </div>
                {move || record.get().filter(|record| !record.state.is_ready()).map(|record| {
                    let json = core::inspect(&record).unwrap_or_else(|_| text("product-attributeLifecycle-storage"));
                    view! { <details><summary class="cursor-pointer">{text("product-attributeLifecycle-stored")}</summary><pre class="max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">{json}</pre></details> }
                })}
            </Show>
        </section>
    }
}
