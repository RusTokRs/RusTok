/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Mounted recovery controller for existing Variant update/delete commands.
//! The component owns the per-Product immutable browser journal; row and editor controls
//! submit requests to it instead of manufacturing transient retry identities.

use crate::catalog_transport::{self, VariantMutationOutcome};
use crate::core::variant_mutation::{self, VariantMutationPhase, VariantMutationRecord};
use crate::core::write_journal::{
    JournalState, ProductWriteScope, WriteJournalError,
};
use crate::model::{ProductVariant, VariantUpdateDraft};
use crate::transport::write_journal;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_auth_ui::ui::leptos::hooks::{use_tenant, use_token};
use rustok_ui_leptos::{Alert, AlertVariant, Button};

#[derive(Clone)]
struct LoadedVariantMutation {
    record: VariantMutationRecord,
    token: Option<String>,
    tenant: Option<String>,
}

fn ready_for_new_operation(
    saved: &VariantMutationRecord,
    locale: Option<String>,
) -> Result<VariantMutationRecord, WriteJournalError> {
    let live = write_journal::read::<VariantMutationPhase>(&saved.scope)?;
    let ready = if saved.state.is_terminal() {
        saved.fresh_from_live(
            &live,
            locale.ok_or(WriteJournalError::Invalid)?,
        )?
    } else if saved.state.is_ready() && *saved == live {
        saved.clone()
    } else {
        return Err(WriteJournalError::Changed);
    };

    if live == ready {
        write_journal::verify(&ready)?;
    } else {
        write_journal::store(&live, &ready)?;
    }
    Ok(ready)
}

#[component]
pub(super) fn ProductVariantMutationSection(
    product_id: String,
    locale: Option<String>,
    update_request: RwSignal<Option<(String, VariantUpdateDraft)>>,
    delete_request: RwSignal<Option<(String, String)>>,
    record: RwSignal<Option<VariantMutationRecord>>,
    storage_error: RwSignal<bool>,
    parent_busy: Signal<bool>,
    set_parent_busy: WriteSignal<bool>,
    blocked: Signal<bool>,
    on_lock_change: Callback<bool>,
    on_update_saved: Callback<ProductVariant>,
    on_delete_saved: Callback<String>,
) -> impl IntoView {
    let product_id = StoredValue::new(product_id);
    let locale = StoredValue::new(locale);
    let text = move |key: &'static str| crate::i18n::t(locale.get_value().as_deref(), key, key);
    let token = use_token();
    let tenant = use_tenant();
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0_u64);
    let error = RwSignal::new(None::<String>);

    // The Product GET is not part of recovery. A pending update/delete is replayed from
    // the frozen request even if the current projection has moved or is unavailable.
    let recovery = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let generation = reload.get();
        let pid = product_id.get_value();
        let loc = locale.get_value();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|_| ())?;
            if reload.try_get_untracked() != Some(generation)
                || token.try_get_untracked() != Some(tok.clone())
                || tenant.try_get_untracked() != Some(ten.clone())
            {
                return Err(());
            }
            let scope = ProductWriteScope {
                tenant_id: bootstrap.current_tenant.id,
                actor_id: bootstrap.me.id,
                product_id: pid,
                locale: loc.ok_or(())?,
            };
            let record = write_journal::load::<VariantMutationPhase>(&scope).map_err(|_| ())?;
            Ok::<_, ()>(LoadedVariantMutation {
                record,
                token: tok,
                tenant: ten,
            })
        }
    });

    Effect::new(move |_| {
        let Some(result) = recovery.get() else {
            on_lock_change.run(true);
            return;
        };
        match result {
            Ok(loaded)
                if loaded.token == token.get_untracked()
                    && loaded.tenant == tenant.get_untracked() =>
            {
                let previous = record.get_untracked();
                if previous
                    .as_ref()
                    .is_some_and(|old| !old.can_restore(&loaded.record))
                {
                    storage_error.set(true);
                } else {
                    record.set(Some(loaded.record));
                    storage_error.set(false);
                }
            }
            _ => storage_error.set(true),
        }
    });

    let authenticated = Signal::derive(move || {
        recovery.get().and_then(Result::ok).is_some_and(|loaded| {
            loaded.token == token.get()
                && loaded.tenant == tenant.get()
                && record
                    .get()
                    .is_some_and(|saved| saved.scope.same_identity(&loaded.record.scope))
        })
    });

    Effect::new(move |_| {
        on_lock_change.run(
            storage_error.get()
                || recovery.get().is_none()
                || !authenticated.get()
                || record.get().is_none_or(|saved| saved.state.is_pending()),
        );
    });

    let send_pending = Callback::new(move |expected: VariantMutationRecord| {
        if busy.get_untracked() || parent_busy.get_untracked() {
            return;
        }
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let expected_for_response = expected.clone();
        busy.set(true);
        set_parent_busy.set(true);

        spawn_local(async move {
            let outcome = match &expected.state {
                VariantMutationPhase::PendingUpdate { .. } => {
                    match expected.state.pending_update() {
                        Some((key, variant_id, draft)) => {
                            catalog_transport::update_product_variant_with_key(
                                tok.clone(),
                                ten.clone(),
                                expected.scope.clone(),
                                variant_id,
                                key,
                                draft,
                            )
                            .await
                        }
                        None => VariantMutationOutcome::Unknown,
                    }
                }
                VariantMutationPhase::PendingDelete { .. } => {
                    match expected.state.pending_delete() {
                        Some((key, variant_id, revision)) => {
                            catalog_transport::delete_product_variant_with_key(
                                tok.clone(),
                                ten.clone(),
                                expected.scope.clone(),
                                variant_id,
                                revision,
                                key,
                            )
                            .await
                        }
                        None => VariantMutationOutcome::Unknown,
                    }
                }
                _ => VariantMutationOutcome::Unknown,
            };

            if busy.try_get_untracked().is_none()
                || record.try_get_untracked().flatten().as_ref() != Some(&expected_for_response)
                || token.try_get_untracked() != Some(tok.clone())
                || tenant.try_get_untracked() != Some(ten.clone())
            {
                if busy.try_get_untracked().is_some() {
                    busy.set(false);
                    set_parent_busy.set(false);
                }
                return;
            }

            match outcome {
                VariantMutationOutcome::Updated(saved) => {
                    let persisted = expected_for_response
                        .confirm_update(saved.revision.clone())
                        .and_then(|settled| {
                            write_journal::store(&expected_for_response, &settled)?;
                            Ok(settled)
                        });
                    match persisted {
                        Ok(settled) => {
                            record.set(Some(settled));
                            storage_error.set(false);
                        }
                        Err(_) => storage_error.set(true),
                    }
                    // The owner acknowledgement is authoritative even if browser readback
                    // failed. The pending journal remains replayable with the same key.
                    on_update_saved.run(saved);
                }
                VariantMutationOutcome::Deleted => {
                    let variant_id = expected_for_response
                        .state
                        .pending_delete()
                        .map(|(_, variant_id, _)| variant_id);
                    let persisted = expected_for_response
                        .confirm_delete()
                        .and_then(|settled| {
                            write_journal::store(&expected_for_response, &settled)?;
                            Ok(settled)
                        });
                    match persisted {
                        Ok(settled) => {
                            record.set(Some(settled));
                            storage_error.set(false);
                        }
                        Err(_) => storage_error.set(true),
                    }
                    if let Some(variant_id) = variant_id {
                        on_delete_saved.run(variant_id);
                    }
                }
                VariantMutationOutcome::Rejected { code } => {
                    let persisted = expected_for_response
                        .reject(code)
                        .and_then(|settled| {
                            write_journal::store(&expected_for_response, &settled)?;
                            Ok(settled)
                        });
                    match persisted {
                        Ok(settled) => {
                            record.set(Some(settled));
                            storage_error.set(false);
                        }
                        Err(_) => storage_error.set(true),
                    }
                }
                VariantMutationOutcome::Unknown => {
                    // Keep the exact Pending record and caller key. No projection refresh,
                    // changed body, or new key can settle an unknown owner outcome.
                }
            }

            busy.set(false);
            set_parent_busy.set(false);
        });
    });

    let retry_pending = Callback::new(move |_: ()| {
        if busy.get_untracked()
            || parent_busy.get_untracked()
            || storage_error.get_untracked()
            || !authenticated.get_untracked()
        {
            return;
        }
        let Some(saved) = record.get_untracked() else {
            return;
        };
        if !saved.state.is_pending() {
            return;
        }
        if write_journal::verify(&saved).is_err() {
            storage_error.set(true);
            return;
        }
        send_pending.run(saved);
    });

    let start_update = Callback::new(move |(variant_id, draft): (String, VariantUpdateDraft)| {
        if busy.get_untracked()
            || parent_busy.get_untracked()
            || blocked.get_untracked()
            || storage_error.get_untracked()
            || !authenticated.get_untracked()
        {
            return;
        }
        let Some(saved) = record.get_untracked() else {
            storage_error.set(true);
            return;
        };
        if saved.state.is_pending() {
            return;
        }
        let ready = match ready_for_new_operation(&saved, locale.get_value()) {
            Ok(ready) => ready,
            Err(_) => {
                storage_error.set(true);
                return;
            }
        };
        let pending = match ready.begin_update(variant_id, draft) {
            Ok(pending) => pending,
            Err(_) => {
                error.set(Some(text("product-variantMutation-invalid")));
                return;
            }
        };
        if write_journal::store(&ready, &pending).is_err() {
            storage_error.set(true);
            return;
        }
        error.set(None);
        record.set(Some(pending.clone()));
        send_pending.run(pending);
    });

    let start_delete = Callback::new(move |(variant_id, expected_revision): (String, String)| {
        if busy.get_untracked()
            || parent_busy.get_untracked()
            || blocked.get_untracked()
            || storage_error.get_untracked()
            || !authenticated.get_untracked()
        {
            return;
        }
        let Some(saved) = record.get_untracked() else {
            storage_error.set(true);
            return;
        };
        if saved.state.is_pending() {
            return;
        }
        let ready = match ready_for_new_operation(&saved, locale.get_value()) {
            Ok(ready) => ready,
            Err(_) => {
                storage_error.set(true);
                return;
            }
        };
        let pending = match ready.begin_delete(variant_id, expected_revision) {
            Ok(pending) => pending,
            Err(_) => {
                error.set(Some(text("product-variantMutation-invalid")));
                return;
            }
        };
        if write_journal::store(&ready, &pending).is_err() {
            storage_error.set(true);
            return;
        }
        error.set(None);
        record.set(Some(pending.clone()));
        send_pending.run(pending);
    });

    // Keep queued UI requests until the original recovery context is ready. The callbacks
    // consume them synchronously before any network work begins.
    Effect::new(move |_| {
        if let Some(request) = update_request.get() {
            if !busy.get()
                && !parent_busy.get()
                && !blocked.get()
                && !storage_error.get()
                && authenticated.get()
            {
                update_request.set(None);
                start_update.run(request);
            }
        }
    });
    Effect::new(move |_| {
        if let Some(request) = delete_request.get() {
            if !busy.get()
                && !parent_busy.get()
                && !blocked.get()
                && !storage_error.get()
                && authenticated.get()
            {
                delete_request.set(None);
                start_delete.run(request);
            }
        }
    });

    view! {
        <Show when=move || {
            storage_error.get()
                || recovery.get().is_some_and(|result| result.is_err())
                || record.get().is_some_and(|saved| !saved.state.is_ready())
                || error.get().is_some()
        }>
            <section class="space-y-3 rounded-2xl border border-border bg-card p-5">
                <h3 class="font-semibold">{text("product-variantMutation-title")}</h3>
                <Show when=move || storage_error.get() || recovery.get().is_some_and(|result| result.is_err())>
                    <Alert variant=AlertVariant::Destructive>
                        {text("product-variantMutation-storage")}
                    </Alert>
                    <Button
                        disabled=move || busy.get() || parent_busy.get()
                        on_click=Box::new(move || {
                            if !busy.get_untracked() && !parent_busy.get_untracked() {
                                reload.update(|value| *value = value.wrapping_add(1));
                            }
                        })
                    >
                        {text("product-variantMutation-recover")}
                    </Button>
                </Show>
                {move || error.get().map(|message| view! {
                    <Alert variant=AlertVariant::Destructive>{message}</Alert>
                })}
                {move || record.get().map(|saved| match saved.state {
                    VariantMutationPhase::Ready => view! { <span /> }.into_any(),
                    VariantMutationPhase::PendingUpdate { variant_id, request, .. } => view! {
                        <Alert>{text("product-variantMutation-pendingUpdate")} <code>{variant_id}</code></Alert>
                        <details class="text-xs">
                            <summary>{text("product-variantMutation-request")}</summary>
                            <pre class="max-h-64 overflow-auto whitespace-pre-wrap">{request}</pre>
                        </details>
                        <Button
                            disabled=move || busy.get() || parent_busy.get() || storage_error.get()
                            on_click=Box::new(move || retry_pending.run(()))
                        >
                            {text("product-variantMutation-retry")}
                        </Button>
                    }.into_any(),
                    VariantMutationPhase::PendingDelete { variant_id, expected_revision, .. } => view! {
                        <Alert>{text("product-variantMutation-pendingDelete")} <code>{variant_id}</code></Alert>
                        <p class="text-xs text-muted-foreground">
                            {text("product-variantMutation-expectedRevision")} {expected_revision}
                        </p>
                        <Button
                            disabled=move || busy.get() || parent_busy.get() || storage_error.get()
                            on_click=Box::new(move || retry_pending.run(()))
                        >
                            {text("product-variantMutation-retry")}
                        </Button>
                    }.into_any(),
                    VariantMutationPhase::ConfirmedUpdate { variant_id, revision, .. } => view! {
                        <Alert>{text("product-variantMutation-confirmedUpdate")} <code>{variant_id}</code> · {revision}</Alert>
                    }.into_any(),
                    VariantMutationPhase::ConfirmedDelete { variant_id, .. } => view! {
                        <Alert>{text("product-variantMutation-confirmedDelete")} <code>{variant_id}</code></Alert>
                    }.into_any(),
                    VariantMutationPhase::RejectedUpdate { code, .. } => view! {
                        <Alert variant=AlertVariant::Destructive>
                            {text("product-variantMutation-rejectedUpdate")} {text(variant_mutation::refusal_key(&code))}
                        </Alert>
                    }.into_any(),
                    VariantMutationPhase::RejectedDelete { code, .. } => view! {
                        <Alert variant=AlertVariant::Destructive>
                            {text("product-variantMutation-rejectedDelete")} {text(variant_mutation::refusal_key(&code))}
                        </Alert>
                    }.into_any(),
                })}
            </section>
        </Show>
    }
}
