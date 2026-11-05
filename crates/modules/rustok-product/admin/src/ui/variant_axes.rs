/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::catalog_transport;
use crate::core::variant_axes::*;
use crate::core::write_journal::{ProductWriteScope, WriteJournalError};
use crate::model::{
    ProductDetail, ProductEffectiveForm, VariantAxesApplyOutcome, VariantAxisReassignmentDraft,
};
use crate::transport::write_journal;

#[derive(Clone)]
struct LoadedAxes {
    record: AxisJournalRecord,
    token: Option<String>,
    tenant: Option<String>,
    product: Option<ProductDetail>,
    form: Option<ProductEffectiveForm>,
    form_available: bool,
}

/// Mounted owner-preview editor. Draft/planner/recovery rules are in core; this adapter only
/// binds controls, async effects and the browser journal. A confirmed axes command never silently
/// rebases dirty Product document fields onto a newer revision.
#[component]
pub(super) fn ProductVariantAxesSection(
    product_id: String,
    locale: Option<String>,
    on_saved: Callback<()>,
    on_lock_change: Callback<bool>,
    before_begin: Callback<(), bool>,
    record: RwSignal<Option<AxisJournalRecord>>,
    storage_error: RwSignal<bool>,
    #[prop(optional)] blocked: Option<Signal<bool>>,
    #[prop(optional)] parent_busy: Option<Signal<bool>>,
) -> impl IntoView {
    let product_id = StoredValue::new(product_id);
    let locale = StoredValue::new(locale);
    let text = move |key: &'static str| crate::i18n::t(locale.get_value().as_deref(), key, key);
    let blocked = blocked.unwrap_or_else(|| Signal::derive(|| false));
    let parent_busy = parent_busy.unwrap_or_else(|| Signal::derive(|| false));
    let token = use_token();
    let tenant = use_tenant();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let rows = RwSignal::new(Vec::<AxisRow>::new());
    let form = RwSignal::new(None::<ProductEffectiveForm>);
    let reassignments = RwSignal::new(Vec::<VariantAxisReassignmentDraft>::new());
    let review = RwSignal::new(None::<AxisReview>);
    let review_valid = RwSignal::new(false);
    let confirmed = RwSignal::new(false);
    let add_selection = RwSignal::new(String::new());
    let page = RwSignal::new(0_usize);
    let refresh = RwSignal::new(0_u64);
    on_lock_change.run(true);

    let data = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let pid = product_id.get_value();
        let loc = locale.get_value();
        let generation = refresh.get();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|_| ())?;
            if refresh.try_get_untracked() != Some(generation)
                || token.try_get_untracked() != Some(tok.clone())
                || tenant.try_get_untracked() != Some(ten.clone())
            {
                return Err(());
            }
            let scope = ProductWriteScope {
                tenant_id: bootstrap.current_tenant.id.clone(),
                actor_id: bootstrap.me.id,
                product_id: pid.clone(),
                locale: loc.clone().ok_or(())?,
            };
            let saved = write_journal::load::<AxisJournalPhase>(&scope).map_err(|_| ())?;
            // Receipt recovery is independent of a later missing/unavailable Product projection.
            let product = match catalog_transport::fetch_product(
                tok.clone(),
                ten.clone(),
                scope.tenant_id.clone(),
                pid.clone(),
                loc.clone(),
            )
            .await
            {
                Ok(product) => product,
                Err(_) => {
                    tracing::warn!(
                        operation = "variant_axes_projection",
                        "Product axes projection unavailable; only stored receipt recovery may continue"
                    );
                    None
                }
            };
            let (effective_form, form_available) = if product.is_some() {
                match catalog_transport::fetch_effective_product_form(
                    tok.clone(),
                    ten.clone(),
                    scope.tenant_id,
                    Some(pid),
                    None,
                    loc.ok_or(())?,
                )
                .await
                {
                    Ok(form) => (form, true),
                    Err(_) => {
                        tracing::warn!(
                            operation = "variant_axes_form",
                            "Product axes schema projection unavailable; stored receipt recovery remains available"
                        );
                        (None, false)
                    }
                }
            } else {
                (None, false)
            };
            if refresh.try_get_untracked() != Some(generation)
                || token.try_get_untracked() != Some(tok.clone())
                || tenant.try_get_untracked() != Some(ten.clone())
            {
                return Err(());
            }
            Ok::<_, ()>(LoadedAxes {
                record: saved,
                token: tok,
                tenant: ten,
                product,
                form: effective_form,
                form_available,
            })
        }
    });

    Effect::new(move |_| {
        let Some(result) = data.get() else {
            return;
        };
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(()) => {
                storage_error.set(true);
                on_lock_change.run(true);
                return;
            }
        };
        if loaded.token != token.get_untracked() || loaded.tenant != tenant.get_untracked() {
            storage_error.set(true);
            on_lock_change.run(true);
            return;
        }
        if let Some(current) = record.get_untracked() {
            if !current.can_restore(&loaded.record) {
                storage_error.set(true);
                on_lock_change.run(true);
                return;
            }
        } else {
            if let Some(product) = &loaded.product {
                rows.set(seed_axes(product, loaded.form.as_ref()));
            }
            if let AxisJournalPhase::Pending { input, .. } = &loaded.record.state {
                reassignments.set(input.reassignments.clone());
            }
        }
        if loaded.form_available {
            rows.update(|rows| refresh_choices(rows, loaded.form.as_ref()));
            form.set(loaded.form.clone());
            review_valid.set(false);
            confirmed.set(false);
        }
        // Restore an exact/one-step journal successor, never rebase dirty rows on resource refresh.
        record.set(Some(loaded.record.clone()));
        storage_error.set(false);
        on_lock_change.run(
            !matches!(loaded.record.state, AxisJournalPhase::Ready) || loaded.product.is_none(),
        );
    });

    let is_ready = move || {
        record
            .get()
            .is_some_and(|record| matches!(record.state, AxisJournalPhase::Ready))
    };
    let is_pending = move || {
        record
            .get()
            .is_some_and(|record| matches!(record.state, AxisJournalPhase::Pending { .. }))
    };
    let is_confirmed = move || {
        record
            .get()
            .is_some_and(|record| matches!(record.state, AxisJournalPhase::Confirmed { .. }))
    };
    let is_rejected = move || {
        record
            .get()
            .is_some_and(|record| matches!(record.state, AxisJournalPhase::Rejected { .. }))
    };
    let has_product = move || {
        data.get()
            .and_then(Result::ok)
            .is_some_and(|loaded| loaded.product.is_some())
    };
    let authenticated = move || {
        data.get().and_then(Result::ok).is_some_and(|loaded| {
            loaded.token == token.get()
                && loaded.tenant == tenant.get()
                && record
                    .get()
                    .is_some_and(|saved| saved.scope.same_identity(&loaded.record.scope))
        })
    };
    let locked = move || {
        !authenticated()
            || busy.get()
            || parent_busy.get()
            || blocked.get()
            || storage_error.get()
            || !is_ready()
            || !has_product()
    };
    let changed = Callback::new(move |()| {
        review_valid.set(false);
        confirmed.set(false);
        error.set(None);
    });
    let current = move || {
        review_valid.get()
            && review.get().is_some_and(|review| {
                review.is_current(&proposal(&rows.get(), &reassignments.get()))
            })
    };

    let preview = move |_| {
        if locked() {
            return;
        }
        let Some(saved) = record.get_untracked() else {
            return;
        };
        if write_journal::verify(&saved).is_err() {
            storage_error.set(true);
            on_lock_change.run(true);
            return;
        }
        let input = proposal(&rows.get_untracked(), &reassignments.get_untracked());
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        busy.set(true);
        error.set(None);
        confirmed.set(false);
        review_valid.set(false);
        on_lock_change.run(true);
        spawn_local(async move {
            let result = catalog_transport::preview_variant_axes(
                tok.clone(),
                ten.clone(),
                saved.scope.clone(),
                input.clone(),
            )
            .await;
            if busy.try_get_untracked().is_none()
                || record.try_get_untracked().flatten().as_ref() != Some(&saved)
                || token.try_get_untracked() != Some(tok)
                || tenant.try_get_untracked() != Some(ten)
            {
                if busy.try_get_untracked().is_some() {
                    busy.set(false);
                }
                return;
            }
            match result {
                Ok(preview) if valid_preview(&saved.scope.product_id, &preview) => {
                    review.set(Some(AxisReview {
                        proposal: input,
                        preview,
                    }));
                    review_valid.set(true);
                    page.set(0);
                }
                Ok(_) | Err(_) => error.set(Some(text("product-axes-failed"))),
            }
            busy.set(false);
            on_lock_change.run(storage_error.get_untracked() || !is_ready() || !has_product());
        });
    };

    let apply = move |_| {
        if busy.get_untracked()
            || parent_busy.get_untracked()
            || storage_error.get_untracked()
            || !authenticated()
        {
            return;
        }
        let Some(saved) = record.get_untracked() else {
            return;
        };
        let pending = match &saved.state {
            AxisJournalPhase::Pending { .. } => {
                if write_journal::verify(&saved).is_err() {
                    storage_error.set(true);
                    on_lock_change.run(true);
                    return;
                }
                saved
            }
            AxisJournalPhase::Ready => {
                if blocked.get_untracked()
                    || !confirmed.get_untracked()
                    || !current()
                    || !before_begin.run(())
                {
                    return;
                }
                let Some(input) = review.get_untracked().and_then(|review| {
                    review.application(&proposal(
                        &rows.get_untracked(),
                        &reassignments.get_untracked(),
                    ))
                }) else {
                    return;
                };
                let next = match saved.begin(input).and_then(|next| {
                    write_journal::store(&saved, &next)?;
                    Ok(next)
                }) {
                    Ok(next) => next,
                    Err(_) => {
                        storage_error.set(true);
                        on_lock_change.run(true);
                        return;
                    }
                };
                record.set(Some(next.clone()));
                next
            }
            _ => return,
        };
        let AxisJournalPhase::Pending { key, input } = pending.state.clone() else {
            return;
        };
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        busy.set(true);
        error.set(None);
        confirmed.set(false);
        on_lock_change.run(true);
        spawn_local(async move {
            let outcome = catalog_transport::set_variant_axes(
                tok.clone(),
                ten.clone(),
                pending.scope.clone(),
                key,
                input,
            )
            .await;
            if busy.try_get_untracked().is_none()
                || record.try_get_untracked().flatten().as_ref() != Some(&pending)
                || token.try_get_untracked() != Some(tok)
                || tenant.try_get_untracked() != Some(ten)
            {
                if busy.try_get_untracked().is_some() {
                    busy.set(false);
                }
                return;
            }
            let terminal = match outcome {
                VariantAxesApplyOutcome::Confirmed { .. } => pending.settle(None),
                VariantAxesApplyOutcome::Rejected { code } => pending.settle(Some(&code)),
                VariantAxesApplyOutcome::Unknown => {
                    error.set(Some(text("product-axes-pending")));
                    busy.set(false);
                    return;
                }
            };
            match terminal.and_then(|terminal| {
                write_journal::store(&pending, &terminal)?;
                Ok(terminal)
            }) {
                Ok(terminal) => {
                    if let AxisJournalPhase::Rejected { code, .. } = &terminal.state {
                        error.set(Some(text(refusal_key(code))));
                    }
                    let success = matches!(terminal.state, AxisJournalPhase::Confirmed { .. });
                    record.set(Some(terminal));
                    review_valid.set(false);
                    if success {
                        on_saved.run(());
                    }
                }
                Err(_) => storage_error.set(true),
            }
            busy.set(false);
            // Both terminal outcomes need an explicit next intent; dirty document fields remain
            // on their original predecessor. Unknown outcomes retain the exact pending request.
            on_lock_change.run(true);
        });
    };

    let fresh = Callback::new(move |reload: bool| {
        if busy.get_untracked() || parent_busy.get_untracked() || !authenticated() {
            return;
        }
        let Some(saved) = record.get_untracked() else {
            return;
        };
        let result = write_journal::read(&saved.scope).and_then(|live| {
            let next = saved
                .fresh_from_live(&live, locale.get_value().ok_or(WriteJournalError::Invalid)?)?;
            if next != live {
                write_journal::store(&live, &next)?;
            }
            Ok(next)
        });
        match result {
            Ok(next) if reload => {
                if write_journal::reload_owner_document().is_err() {
                    storage_error.set(true);
                }
                // Do not unlock this old document even if browser navigation is blocked.
                drop(next);
            }
            Ok(next) => {
                record.set(Some(next));
                storage_error.set(false);
                review_valid.set(false);
                confirmed.set(false);
                error.set(None);
                on_lock_change.run(!has_product());
            }
            Err(_) => {
                storage_error.set(true);
                on_lock_change.run(true);
            }
        }
    });

    view! {
        <section class="space-y-3 rounded-xl border border-border p-3">
            <h4 class="text-sm font-semibold">{text("product-axes-title")}</h4>
            <p class="text-xs text-muted-foreground">{text("product-axes-description")} " " {text("product-axes-bounds")}</p>
            <p class="text-xs text-muted-foreground">{text("product-axes-retained")}</p>
            <Show when=move || blocked.get()><p role="alert" class="text-xs">{text("product-axes-blocked")}</p></Show>
            <Show when=move || storage_error.get()><p role="alert" class="text-xs text-destructive">{text("product-axes-storageUnavailable")}</p></Show>
            <Show when=move || record.get().is_some() && data.get().and_then(Result::ok).is_some_and(|data| data.product.is_none() || !data.form_available)><p role="alert" class="text-xs text-destructive">{text("product-axes-unavailable")}</p></Show>
            <Show when=move || storage_error.get() || data.get().and_then(Result::ok).is_some_and(|data| data.product.is_none() || !data.form_available)>
                <button type="button" disabled=move || busy.get() class="rounded border px-3 py-1 text-xs" on:click=move |_| refresh.update(|value| *value += 1)>{text("product-axes-restore")}</button>
            </Show>
            <Show when=move || error.get().is_some()><p role="alert" class="text-xs text-destructive">{move || error.get()}</p></Show>
            <Show when=is_pending>
                <p role="status" class="text-xs">{text("product-axes-pending")}</p>
                <details class="text-xs"><summary>{text("product-axes-savedRequest")}</summary><pre class="max-h-64 overflow-auto whitespace-pre-wrap">{move || record.get().and_then(|record| match record.state {
                    AxisJournalPhase::Pending { input, .. } => Some(match serde_json::to_string_pretty(&input) {
                        Ok(json) => json,
                        Err(_) => { tracing::error!(operation = "variant_axes_request_view", "Stored axes request cannot be rendered; retaining pending recovery"); text("product-axes-storageUnavailable") }
                    }),
                    _ => None,
                })}</pre></details>
            </Show>
            <Show when=is_confirmed><p role="status" class="text-xs">{text("product-axes-saved")}</p></Show>
            <Show when=move || is_confirmed() || is_rejected()>
                <button type="button" class="rounded border px-3 py-1 text-xs" disabled=move || busy.get() || parent_busy.get() on:click=move |_| fresh.run(true)>{text("product-axes-fresh")}</button>
                <Show when=is_rejected><button type="button" class="ml-2 rounded border px-3 py-1 text-xs" disabled=move || busy.get() || parent_busy.get() on:click=move |_| fresh.run(false)>{text("product-axes-correct")}</button></Show>
            </Show>
            <fieldset disabled=locked class="space-y-3">
                <Show when=move || rows.get().is_empty()><p class="text-xs">{text("product-axes-empty")}</p></Show>
                <For each=move || rows.get() key=|row| (row.attribute_id.clone(), row.label.clone(), row.code.clone(), row.dictionary_limited,
                    row.options.iter().map(|option| (option.id.clone(), option.label.clone(), option.available)).collect::<Vec<_>>()) children=move |row| {
                    let attribute = StoredValue::new(row.attribute_id.clone());
                    view! {
                        <div class="space-y-2 rounded border border-border p-3">
                            <div class="flex items-center justify-between gap-2">
                                <span class="text-xs font-semibold">{row.label} " " <span class="font-mono text-muted-foreground">{row.code}</span></span>
                                <div class="flex gap-1">
                                    <button type="button" class="rounded border px-2" aria-label=text("product-axes-up") on:click=move |_| { changed.run(()); rows.update(|rows| move_axis(rows, &attribute.get_value(), -1)); }>"↑"</button>
                                    <button type="button" class="rounded border px-2" aria-label=text("product-axes-down") on:click=move |_| { changed.run(()); rows.update(|rows| move_axis(rows, &attribute.get_value(), 1)); }>"↓"</button>
                                    <button type="button" class="rounded border px-2" aria-label=text("product-axes-remove") on:click=move |_| { changed.run(()); rows.update(|rows| rows.retain(|row| row.attribute_id != attribute.get_value())); }>"×"</button>
                                </div>
                            </div>
                            {row.dictionary_limited.then(|| view! { <p role="status" class="text-xs text-muted-foreground">{text("product-axes-dictionaryLimit")}</p> })}
                            <p class="text-xs text-muted-foreground">{text("product-axes-allowed")}</p>
                            <div class="flex flex-wrap gap-3">
                                {row.options.into_iter().map(move |option| {
                                    let option_id = StoredValue::new(option.id);
                                    let selected = move || rows.get().iter().find(|row| row.attribute_id == attribute.get_value()).is_some_and(|row| row.allowed_option_ids.contains(&option_id.get_value()));
                                    view! { <label class="inline-flex items-center gap-1 text-xs">
                                        <input type="checkbox" prop:checked=selected disabled=move || locked() || (!option.available && !selected()) on:change=move |_| { changed.run(()); rows.update(|rows| toggle_allowed(rows, &attribute.get_value(), &option_id.get_value())); } />
                                        {option.label}{(!option.available).then(|| format!(" ({})", text("product-axes-unavailableValue")))}
                                    </label> }
                                }).collect_view()}
                            </div>
                        </div>
                    }
                } />
                <div class="flex flex-wrap items-end gap-2">
                    <label class="grid gap-1 text-xs"><span>{text("product-axes-attribute")}</span>
                        <select class="rounded border border-border bg-background px-2 py-1" disabled=move || (locked() || rows.get().len() >= 32) prop:value=move || add_selection.get() on:change=move |event| add_selection.set(event_target_value(&event))>
                            <option value="">{text("product-axes-attribute")}</option>
                            {move || form.get().map(|form| form.attributes.iter().filter(|attribute| !rows.get().iter().any(|row| row.attribute_id == attribute.attribute_id) && axis_candidate(&form, &attribute.attribute_id).is_some()).map(|attribute| view! { <option value=attribute.attribute_id.clone()>{attribute.label.clone()}</option> }).collect_view())}
                        </select>
                    </label>
                    <button type="button" class="rounded border px-3 py-1 text-xs" disabled=move || (locked() || rows.get().len() >= 32 || add_selection.get().is_empty()) on:click=move |_| {
                        let Some(candidate) = form.get_untracked().and_then(|form| axis_candidate(&form, &add_selection.get_untracked())) else { return; };
                        if rows.get_untracked().len() >= 32 || rows.get_untracked().iter().any(|row| row.attribute_id == candidate.attribute_id) { return; }
                        changed.run(()); rows.update(|rows| rows.push(candidate)); add_selection.set(String::new());
                    }>{text("product-axes-add")}</button>
                </div>
            </fieldset>
            <button type="button" class="rounded border px-3 py-1 text-xs" disabled=locked on:click=preview>{move || text(if busy.get() { "product-axes-previewing" } else { "product-axes-preview" })}</button>
            <Show when=move || review.get().is_some()>
                <p role="status" class="text-xs">{move || text(if !current() { "product-axes-stale" } else if review.get().is_some_and(|review| !review.preview.can_apply) { "product-axes-conflicts" } else if review.get().is_some_and(|review| review.preview.has_changes) { "product-axes-ready" } else { "product-axes-noChanges" })}</p>
                <ul class="space-y-2 text-xs" aria-live="polite">{move || review.get().map(|review| review.preview.conflicts.into_iter().map(|conflict| view! {
                    <li class="rounded border border-destructive/40 p-2">{text(conflict_key(&conflict.kind))} ": " <span class="font-mono">{conflict.variant_ids.join(", ")}</span>
                        <span class="block font-mono">{conflict.attribute_id} " " {conflict.option_id}</span>
                        {conflict.has_more_variants.then(|| text("product-axes-moreVariants"))}
                    </li>
                }).collect_view())}</ul>
                <Show when=move || review.get().is_some_and(|review| review.preview.has_more_conflicts)><p role="alert" class="text-xs">{text("product-axes-moreConflicts")}</p></Show>
                <h5 class="text-xs font-semibold">{text("product-axes-tuples")} " (" {move || review.get().map(|review| review.preview.variant_count)} ")"</h5>
                <p class="text-xs text-muted-foreground">{text("product-axes-tuplesHelp")}</p>
                <fieldset disabled=locked class="space-y-3">
                    <For each=move || review.get().map(|review| {
                        let token = review.preview.state_token; review.preview.variants.into_iter().skip(page.get() * AXIS_PREVIEW_PAGE_SIZE).take(AXIS_PREVIEW_PAGE_SIZE).map(|variant| (token.clone(), variant)).collect::<Vec<_>>()
                    }).unwrap_or_default() key=|(token, variant)| (token.clone(), variant.variant_id.clone()) children=move |(_, variant)| {
                        let source = StoredValue::new(variant);
                        let values = move || target_values(&reassignments.get(), &source.get_value());
                        view! {
                            <div class="space-y-2 rounded border border-border p-3">
                                <p class="text-xs font-semibold">{source.get_value().sku.unwrap_or_else(|| source.get_value().variant_id)}</p>
                                <p class="font-mono text-xs text-muted-foreground">{source.get_value().variant_id}</p>
                                <p class="text-xs">{text("product-axes-current")} ": " {move || { let label = tuple_summary(&source.get_value().current_values, &rows.get(), form.get().as_ref()); if label.is_empty() { text("product-axes-default") } else { label } }}</p>
                                <p class="text-xs">{text("product-axes-proposed")} ": " {move || { let label = tuple_summary(&values(), &rows.get(), form.get().as_ref()); if label.is_empty() { text(if rows.get().is_empty() { "product-axes-default" } else { "product-axes-unassigned" }) } else { label } }}</p>
                                <Show when=move || values().iter().any(|value| !rows.get().iter().any(|row| row.attribute_id == value.attribute_id))><p role="alert" class="text-xs text-destructive">{text("product-axes-unexpected")}</p></Show>
                                <div class="grid gap-2 sm:grid-cols-2">
                                    <For each=move || rows.get() key=|row| row.attribute_id.clone() children=move |row| {
                                        let attribute = StoredValue::new(row.attribute_id);
                                        let selected = move || values().into_iter().find(|value| value.attribute_id == attribute.get_value()).map(|value| value.option_id).unwrap_or_default();
                                        view! { <label class="grid gap-1 text-xs"><span>{move || rows.with(|rows| rows.iter().find(|row| row.attribute_id == attribute.get_value()).map(|row| row.label.clone()).unwrap_or_default())}</span>
                                            <select class="rounded border border-border bg-background px-2 py-1" prop:value=selected on:change=move |event| { changed.run(()); reassignments.update(|rows| reassign_value(rows, &source.get_value(), &attribute.get_value(), &event_target_value(&event))); }>
                                                <option value="">{text("product-axes-unassigned")}</option>
                                                {move || {
                                                    let axis = rows.get().into_iter().find(|row| row.attribute_id == attribute.get_value());
                                                    let selected = selected();
                                                    let unavailable = axis.as_ref().is_some_and(|row| !selected.is_empty() && !row.allowed_option_ids.contains(&selected));
                                                    view! {
                                                        {unavailable.then(|| view! { <option value=selected.clone() disabled=true>{selected.clone()} " (" {text("product-axes-unavailableValue")} ")"</option> })}
                                                        {axis.map(|row| row.options.into_iter().filter(|option| row.allowed_option_ids.contains(&option.id)).map(|option| view! { <option value=option.id disabled=(!option.available)>{option.label}</option> }).collect_view())}
                                                    }
                                                }}
                                            </select>
                                        </label> }
                                    } />
                                </div>
                                <Show when=move || reassignments.get().iter().any(|row| row.variant_id == source.get_value().variant_id)>
                                    <button type="button" class="rounded border px-3 py-1 text-xs" on:click=move |_| { changed.run(()); reassignments.update(|rows| rows.retain(|row| row.variant_id != source.get_value().variant_id)); }>{text("product-axes-reset")}</button>
                                </Show>
                            </div>
                        }
                    } />
                    <div class="flex justify-between gap-2">
                        <button type="button" class="rounded border px-3 py-1 text-xs" disabled=move || page.get() == 0 on:click=move |_| page.update(|page| *page = page.saturating_sub(1))>{text("product-axes-previous")}</button>
                        <span class="text-xs">{move || page.get() + 1}</span>
                        <button type="button" class="rounded border px-3 py-1 text-xs" disabled=move || review.get().is_none_or(|review| (page.get() + 1) * AXIS_PREVIEW_PAGE_SIZE >= review.preview.variants.len()) on:click=move |_| page.update(|page| *page += 1)>{text("product-axes-next")}</button>
                    </div>
                </fieldset>
                <label class="flex items-start gap-2 text-xs"><input type="checkbox" prop:checked=move || confirmed.get() disabled=move || locked() || !current() || review.get().is_none_or(|review| !review.preview.can_apply || !review.preview.has_changes) on:change=move |event| confirmed.set(event_target_checked(&event)) />{text("product-axes-confirm")}</label>
            </Show>
            <button type="button" class="rounded bg-primary px-3 py-1 text-xs text-primary-foreground disabled:opacity-50" disabled=move || busy.get() || parent_busy.get() || storage_error.get() || (!is_pending() && (locked() || !current() || !confirmed.get())) on:click=apply>
                {move || text(if busy.get() { "product-axes-applying" } else if is_pending() { "product-axes-retry" } else { "product-axes-apply" })}
            </button>
        </section>
    }
}
