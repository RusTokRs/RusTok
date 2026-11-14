/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! A stable recovery controller outside the Product projection's remounting SKU grid.

use crate::catalog_transport;
use crate::core::variant_create::{self, CreateField, VariantCreatePhase, VariantCreateRecord};
use crate::core::write_journal::{JournalState, ProductWriteScope, WriteJournalError};
use crate::model::{ProductDetail, VariantCreateOutcome, VariantDraft};
use crate::transport::write_journal;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui::{Alert, AlertVariant, Button, Input, Select, SelectOption};
use rustok_auth_ui::ui::leptos::hooks::{use_tenant, use_token};

#[derive(Clone)]
struct LoadedRecovery {
    record: VariantCreateRecord,
    token: Option<String>,
    tenant: Option<String>,
}

#[component]
pub(super) fn ProductVariantCreateSection(
    product_id: String,
    locale: Option<String>,
    product: Signal<Option<ProductDetail>>,
    record: RwSignal<Option<VariantCreateRecord>>,
    storage_error: RwSignal<bool>,
    on_saved: Callback<String>,
    on_lock_change: Callback<bool>,
    blocked: Signal<bool>,
    parent_busy: Signal<bool>,
    before_begin: Callback<(), bool>,
) -> impl IntoView {
    let product_id = StoredValue::new(product_id);
    let locale = StoredValue::new(locale);
    let text = move |key: &'static str| crate::i18n::t(locale.get_value().as_deref(), key, key);
    let token = use_token();
    let tenant = use_tenant();
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0_u64);
    let error = RwSignal::new(None::<String>);
    let show_form = RwSignal::new(false);
    let draft = RwSignal::new(variant_create::empty_draft("USD"));
    let (quantity, set_quantity) = signal("0".to_string());

    // Authentication and browser recovery do not depend on the Product read succeeding.
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
            let record = write_journal::load::<VariantCreatePhase>(&scope).map_err(|_| ())?;
            Ok::<_, ()>(LoadedRecovery {
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
            Ok(loaded) if loaded.token == token.get() && loaded.tenant == tenant.get() => {
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
            busy.get()
                || storage_error.get()
                || !authenticated.get()
                || recovery.get().is_none()
                || record.get().is_none_or(|saved| !saved.state.is_ready()),
        );
    });
    let editable = Signal::derive(move || {
        !busy.get()
            && !parent_busy.get()
            && !blocked.get()
            && !storage_error.get()
            && authenticated.get()
            && record.get().is_some_and(|saved| saved.state.is_ready())
            && product
                .get()
                .is_some_and(|value| value.id == product_id.get_value())
    });

    let submit = Callback::new(move |_: ()| {
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
        let expected = if saved.state.is_ready() {
            if !editable.get_untracked() || !before_begin.run(()) {
                return;
            }
            let Some(current) = product.get_untracked() else {
                return;
            };
            let input = match variant_create::prepare(
                draft.get_untracked(),
                &quantity.get_untracked(),
                &current,
            ) {
                Ok(input) => input,
                Err(_) => {
                    error.set(Some(text("product-variantCreate-invalid")));
                    return;
                }
            };
            let result = locale
                .get_value()
                .ok_or(WriteJournalError::Invalid)
                .and_then(|locale| saved.begin(input, locale))
                .and_then(|pending| {
                    write_journal::store(&saved, &pending)?;
                    Ok(pending)
                });
            match result {
                Ok(pending) => pending,
                Err(_) => {
                    storage_error.set(true);
                    on_lock_change.run(true);
                    return;
                }
            }
        } else if matches!(saved.state, VariantCreatePhase::Pending { .. }) {
            // Recovery ignores current form fields, projection, locale and changed axis metadata.
            if write_journal::verify(&saved).is_err() {
                storage_error.set(true);
                return;
            }
            saved
        } else {
            return;
        };
        let VariantCreatePhase::Pending { key, input } = &expected.state else {
            return;
        };
        let key = key.clone();
        let input = input.clone();
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        record.set(Some(expected.clone()));
        busy.set(true);
        on_lock_change.run(true);
        error.set(None);
        spawn_local(async move {
            let outcome = catalog_transport::create_product_variant(
                tok.clone(),
                ten.clone(),
                expected.scope.clone(),
                key,
                input,
            )
            .await;
            // A late response cannot update a disposed page or a different authenticated scope.
            // Pending remains durable and can be replayed when its original context returns.
            if busy.try_get_untracked().is_none()
                || record.try_get_untracked().flatten().as_ref() != Some(&expected)
                || token.try_get_untracked() != Some(tok)
                || tenant.try_get_untracked() != Some(ten)
            {
                if busy.try_get_untracked().is_some() {
                    busy.set(false);
                }
                return;
            }
            if matches!(outcome, VariantCreateOutcome::Unknown) {
                error.set(Some(text("product-variantCreate-pending")));
            } else {
                let confirmed = matches!(outcome, VariantCreateOutcome::Confirmed { .. });
                match expected.settle(outcome).and_then(|settled| {
                    write_journal::store(&expected, &settled)?;
                    Ok(settled)
                }) {
                    Ok(settled) => {
                        record.set(Some(settled));
                        if confirmed {
                            on_saved.run(expected.scope.product_id);
                        }
                    }
                    Err(_) => storage_error.set(true),
                }
            }
            busy.set(false);
        });
    });

    // This is always an explicit terminal action, including retry of a failed fresh readback.
    // A confirmed creation opens a fresh owner document instead of rebasing dirty fields.
    let fresh = Callback::new(move |correct: bool| {
        if busy.get_untracked() || parent_busy.get_untracked() || !authenticated.get_untracked() {
            return;
        }
        let Some(saved) = record
            .get_untracked()
            .filter(|saved| saved.state.is_terminal())
        else {
            return;
        };
        if correct && !matches!(saved.state, VariantCreatePhase::Rejected { .. }) {
            return;
        }
        let result = write_journal::read::<VariantCreatePhase>(&saved.scope).and_then(|live| {
            let next = saved
                .fresh_from_live(&live, locale.get_value().ok_or(WriteJournalError::Invalid)?)?;
            if live == next {
                write_journal::verify(&next)?;
            } else {
                write_journal::store(&live, &next)?;
            }
            Ok(next)
        });
        match result {
            Ok(next) if correct => {
                if let Some(input) = saved.state.input() {
                    draft.set(input.clone());
                    set_quantity.set(input.inventory_quantity.unwrap_or_default().to_string());
                }
                record.set(Some(next));
                storage_error.set(false);
                show_form.set(true);
                error.set(None);
            }
            Ok(_) => {
                // Keep this mounted terminal editor locked until navigation actually replaces it.
                if write_journal::reload_owner_document().is_err() {
                    storage_error.set(true);
                }
            }
            Err(_) => storage_error.set(true),
        }
    });

    let picker_axes = Signal::derive(move || {
        product
            .get()
            .map(|value| crate::core::combinations::saved_axes(&value.variant_axes))
            .unwrap_or(Err(crate::core::combinations::CombinationError::Axes))
    });
    let picker_members = Signal::derive(move || {
        product
            .get()
            .map(|value| crate::core::combinations::saved_members(&value.variants))
            .unwrap_or(Ok(Vec::new()))
    });
    let pick = Callback::new(move |values| {
        if editable.get_untracked() {
            draft.update(|draft| draft.axis_values = values);
        }
    });
    view! {
        <section class="space-y-3 rounded-2xl border border-border bg-card p-5" aria-busy=move || busy.get()>
            <h3 class="font-semibold">{text("product-variantCreate-title")}</h3>
            <p class="text-sm text-muted-foreground">{text("product-variantCreate-help")}</p>
            <Show when=move || storage_error.get() || recovery.get().is_some_and(|result| result.is_err())>
                <Alert variant=AlertVariant::Destructive>{text("product-variantCreate-storage")}</Alert>
                {move || view! { <Button disabled=busy.get() on_click=Box::new(move || { if !busy.get_untracked() { reload.update(|value| *value = value.wrapping_add(1)); } })>{text("product-variantCreate-recover")}</Button> }}
            </Show>
            {move || error.get().map(|message| view! { <Alert variant=AlertVariant::Destructive>{message}</Alert> })}
            {move || record.get().map(|saved| match saved.state {
                VariantCreatePhase::Pending { .. } => view! {
                    <Alert>{text("product-variantCreate-pending")}</Alert>
                    <Button disabled=busy.get() || parent_busy.get() || storage_error.get() on_click=Box::new(move || submit.run(()))>{text("product-variantCreate-retry")}</Button>
                }.into_any(),
                VariantCreatePhase::Confirmed { variant_id, .. } => view! {
                    <Alert>{text("product-variantCreate-confirmed")}<code class="ml-2">{variant_id}</code></Alert>
                    <p class="text-xs">{text("product-variantCreate-freshWarning")}</p>
                    <Button disabled=busy.get() || parent_busy.get() on_click=Box::new(move || fresh.run(false))>{text("product-variantCreate-fresh")}</Button>
                }.into_any(),
                VariantCreatePhase::Rejected { code, .. } => view! {
                    <Alert variant=AlertVariant::Destructive>{text(variant_create::refusal_key(&code))}</Alert>
                    <Button disabled=busy.get() || parent_busy.get() on_click=Box::new(move || fresh.run(true))>{text("product-variantCreate-correct")}</Button>
                    <Button disabled=busy.get() || parent_busy.get() on_click=Box::new(move || fresh.run(false))>{text("product-variantCreate-fresh")}</Button>
                }.into_any(),
                VariantCreatePhase::Ready => view! {
                    <Button disabled=!editable.get() || product.get().is_none_or(|product| !variant_create::can_create(&product)) on_click=Box::new(move || { if editable.get_untracked() && product.get_untracked().is_some_and(|product| variant_create::can_create(&product)) { show_form.update(|open| *open = !*open); } })>{text("product-variantCreate-open")}</Button>
                }.into_any(),
            })}
            {move || record.get().and_then(|saved| saved.state.input().cloned()).map(|input| view! {
                <details class="text-xs"><summary>{text("product-variantCreate-savedRequest")}</summary><pre class="max-h-64 overflow-auto whitespace-pre-wrap">{serde_json::to_string_pretty(&input).unwrap_or_else(|_| text("product-variantCreate-storage"))}</pre></details>
            })}
            <Show when=move || product.get().is_some_and(|product| !variant_create::can_create(&product))><p class="text-sm text-muted-foreground">{text("product-variantCreate-creationUnavailable")}</p></Show>
            <Show when=move || product.get().is_none()><Alert>{text("product-variantCreate-projectionUnavailable")}</Alert></Show>
            <Show when=move || show_form.get() && record.get().is_some_and(|saved| saved.state.is_ready())>
                <form on:submit=move |event| { event.prevent_default(); submit.run(()); }>
                <fieldset disabled=move || !editable.get() class="space-y-3">
                    <super::combinations::CombinationPicker axes=picker_axes members=picker_members locale=locale.get_value()
                        disabled=Signal::derive(move || !editable.get()) at_capacity=Signal::derive(move || product.get().is_none_or(|value| value.variants.len() >= 256)) on_pick=pick />
                    <div class="grid gap-3 md:grid-cols-3">
                        <CreateTextField field=CreateField::Sku draft=draft editable=editable locale=locale.get_value() />
                        <CreateTextField field=CreateField::Barcode draft=draft editable=editable locale=locale.get_value() />
                        <CreateTextField field=CreateField::ShippingProfile draft=draft editable=editable locale=locale.get_value() />
                    </div>
                    <For each={move || variant_create::assignment_ids(product.get().as_ref(), &draft.get())} key={|id| id.clone()} children={move |id| view! {
                        <CreateAssignment attribute_id=id draft=draft product=product editable=editable locale=locale.get_value() />
                    }} />
                    <For each={move || (0..draft.get().prices.len()).collect::<Vec<_>>()} key={|index| *index} children={move |index| view! {
                        <div class="grid gap-3 md:grid-cols-4">
                            <CreateTextField field=CreateField::Currency(index) draft=draft editable=editable locale=locale.get_value() />
                            <CreateTextField field=CreateField::Amount(index) draft=draft editable=editable locale=locale.get_value() />
                            <CreateTextField field=CreateField::CompareAt(index) draft=draft editable=editable locale=locale.get_value() />
                            <Button on_click=Box::new(move || { if editable.get_untracked() { draft.update(|draft| { if index < draft.prices.len() { draft.prices.remove(index); } }); } })>{text("product-variantCreate-removePrice")}</Button>
                        </div>
                    }} />
                    {move || view! { <Button disabled=(draft.get().prices.len() >= variant_create::MAX_PRICE_ROWS) on_click=Box::new(move || { if editable.get_untracked() { draft.update(|draft| { if draft.prices.len() < variant_create::MAX_PRICE_ROWS { draft.prices.push(crate::model::VariantPriceDraft { currency_code: "USD".into(), ..Default::default() }); } }); } })>{text("product-variantCreate-addPrice")}</Button> }}
                    <p class="text-xs">{text("product-combinations-noPrice")}</p>
                    <label class="block"><span>{text("product-variantCreate-quantity")}</span><Input r#type="number" value=quantity set_value=set_quantity /></label>
                    <CreateInventoryPolicy draft=draft editable=editable locale=locale.get_value() />
                    <Button r#type="submit" disabled=(!editable.get())>{text("product-variantCreate-save")}</Button>
                </fieldset>
                </form>
            </Show>
        </section>
    }
}

#[component]
fn CreateTextField(
    field: CreateField,
    draft: RwSignal<VariantDraft>,
    editable: Signal<bool>,
    locale: Option<String>,
) -> impl IntoView {
    let (value, set_value) = signal(draft.with_untracked(|draft| field.value(draft)));
    Effect::new(move |_| {
        let next = draft.with(|draft| field.value(draft));
        if value.get_untracked() != next {
            set_value.set(next);
        }
    });
    view! {
        <label class="block" on:input=move |event| {
            if editable.get_untracked() { draft.update(|draft| field.set(draft, event_target_value(&event))); }
        }><span>{crate::i18n::t(locale.as_deref(), field.message_key(), field.message_key())}</span><Input value=value set_value=set_value /></label>
    }
}

#[component]
fn CreateAssignment(
    attribute_id: String,
    draft: RwSignal<VariantDraft>,
    product: Signal<Option<ProductDetail>>,
    editable: Signal<bool>,
    locale: Option<String>,
) -> impl IntoView {
    let id = StoredValue::new(attribute_id);
    let locale = StoredValue::new(locale);
    let (value, set_value) = signal(crate::core::initial_catalog::pending_assignment_value(
        &draft.get_untracked().axis_values,
        &id.get_value(),
    ));
    Effect::new(move |_| {
        let next = crate::core::initial_catalog::pending_assignment_value(
            &draft.get().axis_values,
            &id.get_value(),
        );
        if value.get_untracked() != next {
            set_value.set(next);
        }
    });
    view! { <label class="block" on:change=move |event| {
        if editable.get_untracked() { draft.update(|draft| crate::core::initial_catalog::set_pending_assignment(&mut draft.axis_values, &id.get_value(), event_target_value(&event))); }
    }>{move || {
        let (label, choices) = variant_create::assignment_choices(product.get().as_ref(), &draft.get(), &id.get_value());
        let options: Vec<SelectOption> = choices.into_iter().map(|choice| {
            let mut option = SelectOption::new(choice.id, if choice.available { choice.label } else { format!("{} · {}", crate::i18n::t(locale.get_value().as_deref(), "product-combinations-retainedAssignment", "product-combinations-retainedAssignment"), choice.label) });
            option.disabled = !choice.available; option
        }).collect();
        view! { <span>{label}</span><Select options=options value=value placeholder=crate::i18n::t(locale.get_value().as_deref(), "product-variantCreate-selectValue", "product-variantCreate-selectValue") /> }
    }}</label> }
}

#[component]
fn CreateInventoryPolicy(
    draft: RwSignal<VariantDraft>,
    editable: Signal<bool>,
    locale: Option<String>,
) -> impl IntoView {
    let (value, set_value) = signal(draft.get_untracked().inventory_policy.unwrap_or_default());
    Effect::new(move |_| {
        let next = draft.get().inventory_policy.unwrap_or_default();
        if value.get_untracked() != next {
            set_value.set(next);
        }
    });
    let options = vec![
        SelectOption::new(
            "deny",
            crate::i18n::t(
                locale.as_deref(),
                "product-variantCreate-deny",
                "product-variantCreate-deny",
            ),
        ),
        SelectOption::new(
            "continue",
            crate::i18n::t(
                locale.as_deref(),
                "product-variantCreate-continue",
                "product-variantCreate-continue",
            ),
        ),
    ];
    view! { <label class="block" on:change=move |event| {
        if editable.get_untracked() { draft.update(|draft| draft.inventory_policy = Some(event_target_value(&event))); }
    }><span>{crate::i18n::t(locale.as_deref(), "product-variantCreate-policy", "product-variantCreate-policy")}</span><Select options=options value=value /></label> }
}
