mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};

use crate::i18n::t;
use crate::model::{
    CommerceAdminBootstrap, CommerceAdminCartSnapshot, CommerceCartPromotionDraft,
    CommerceCartPromotionKind, CommerceCartPromotionPreview, CommerceCartPromotionScope,
    ShippingProfile, ShippingProfileDraft,
};

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
pub fn CommerceAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_profile_query = use_route_query_value(AdminQueryKey::ShippingProfileId.as_str());
    let selected_cart_query = use_route_query_value(AdminQueryKey::CartId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);

    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<ShippingProfile>::None);
    let (slug, set_slug) = signal(String::new());
    let (name, set_name) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (metadata_json, set_metadata_json) = signal(String::new());
    let (search, set_search) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (promotion_cart_id, set_promotion_cart_id) = signal(String::new());
    let (promotion_kind, set_promotion_kind) = signal("fixed_discount".to_string());
    let (promotion_scope, set_promotion_scope) = signal("shipping".to_string());
    let (promotion_line_item_id, set_promotion_line_item_id) = signal(String::new());
    let (promotion_source_id, set_promotion_source_id) = signal("promo-operator".to_string());
    let (promotion_discount_percent, set_promotion_discount_percent) = signal(String::new());
    let (promotion_amount, set_promotion_amount) = signal("4.99".to_string());
    let (promotion_metadata_json, set_promotion_metadata_json) = signal(String::new());
    let (promotion_busy, set_promotion_busy) = signal(false);
    let (promotion_error, set_promotion_error) = signal(Option::<String>::None);
    let (promotion_preview, set_promotion_preview) =
        signal(Option::<CommerceCartPromotionPreview>::None);
    let (promotion_result, set_promotion_result) =
        signal(Option::<CommerceAdminCartSnapshot>::None);

    let badge_label = t(ui_locale.as_deref(), "commerce.badge", "commerce");
    let title_label = t(
        ui_locale.as_deref(),
        "commerce.title",
        "Commerce Shipping Profile Registry",
    );
    let subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.subtitle",
        "Module-owned operator workspace for the typed shipping-profile registry used by catalog and delivery orchestration.",
    );
    let bootstrap_loading_label = t(
        ui_locale.as_deref(),
        "commerce.error.bootstrapLoading",
        "Bootstrap is still loading.",
    );
    let new_label = t(ui_locale.as_deref(), "commerce.action.new", "New");
    let edit_label = t(ui_locale.as_deref(), "commerce.action.edit", "Edit");
    let name_placeholder_label = t(ui_locale.as_deref(), "commerce.field.name", "Name");
    let slug_placeholder_label = t(ui_locale.as_deref(), "commerce.field.slug", "Slug");
    let description_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.description",
        "Description",
    );
    let metadata_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.metadataJsonPatch",
        "Metadata JSON patch",
    );
    let metadata_hint_label = t(
        ui_locale.as_deref(),
        "commerce.metadata.hint",
        "Metadata is sent as an optional JSON patch. Leaving the field blank during update keeps the existing metadata payload unchanged.",
    );
    let shipping_profiles_title_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.title",
        "Shipping Profiles",
    );
    let shipping_profiles_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.subtitle",
        "Manage the typed profile registry used by products and shipping-option compatibility rules.",
    );
    let shipping_profiles_search_placeholder = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.searchPlaceholder",
        "Search slug or name",
    );
    let no_shipping_profiles_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.empty",
        "No shipping profiles match the current filters.",
    );
    let load_shipping_profiles_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadShippingProfiles",
        "Failed to load shipping profiles",
    );
    let editor_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfile.editor",
        "Shipping Profile Editor",
    );
    let create_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfile.create",
        "Create Shipping Profile",
    );
    let editor_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfile.subtitle",
        "Typed registry editor for the slugs referenced by products and shipping options.",
    );
    let required_label = t(
        ui_locale.as_deref(),
        "commerce.error.shippingProfileRequired",
        "Shipping profile slug and name are required.",
    );
    let not_found_label = t(
        ui_locale.as_deref(),
        "commerce.error.shippingProfileNotFound",
        "Shipping profile not found.",
    );
    let load_shipping_profile_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadShippingProfile",
        "Failed to load shipping profile",
    );
    let save_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.saveShippingProfile",
        "Failed to save shipping profile",
    );
    let locale_unavailable_label = t(
        ui_locale.as_deref(),
        "commerce.error.localeUnavailable",
        "Host locale is unavailable.",
    );
    let toggle_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.changeShippingProfileStatus",
        "Failed to change shipping profile status",
    );
    let save_button_label = t(
        ui_locale.as_deref(),
        "commerce.action.saveShippingProfile",
        "Save shipping profile",
    );
    let create_button_label = t(
        ui_locale.as_deref(),
        "commerce.action.createShippingProfile",
        "Create shipping profile",
    );
    let summary_empty_label = t(
        ui_locale.as_deref(),
        "commerce.summary.shippingProfile.empty",
        "Open a shipping profile to inspect its slug, description and lifecycle state.",
    );
    let promotion_title_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.title",
        "Cart Promotion Operator",
    );
    let promotion_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.subtitle",
        "Native operator surface for previewing and applying typed cart promotions over the active cart snapshot.",
    );
    let cart_id_placeholder_label = t(ui_locale.as_deref(), "commerce.field.cartId", "Cart ID");
    let source_id_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.sourceId",
        "Promotion source ID",
    );
    let line_item_id_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.lineItemId",
        "Line item ID",
    );
    let discount_percent_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.discountPercent",
        "Discount percent",
    );
    let amount_placeholder_label = t(ui_locale.as_deref(), "commerce.field.amount", "Amount");
    let preview_button_label = t(
        ui_locale.as_deref(),
        "commerce.action.previewPromotion",
        "Preview promotion",
    );
    let apply_button_label = t(
        ui_locale.as_deref(),
        "commerce.action.applyPromotion",
        "Apply promotion",
    );
    let clear_cart_label = t(
        ui_locale.as_deref(),
        "commerce.action.clearCartSelection",
        "Clear cart selection",
    );
    let promotion_required_label = t(
        ui_locale.as_deref(),
        "commerce.error.cartPromotionRequired",
        "Cart ID and source ID are required.",
    );
    let preview_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.previewPromotion",
        "Failed to preview promotion",
    );
    let apply_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.applyPromotion",
        "Failed to apply promotion",
    );
    let preview_empty_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.previewEmpty",
        "Preview a cart promotion to inspect its typed adjustment impact before applying it.",
    );
    let result_empty_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.resultEmpty",
        "No cart mutation has been applied yet.",
    );
    let preview_title_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.previewTitle",
        "Preview",
    );
    let result_title_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.resultTitle",
        "Applied cart snapshot",
    );
    let fixed_discount_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.kind.fixedDiscount",
        "Fixed discount",
    );
    let percentage_discount_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.kind.percentageDiscount",
        "Percentage discount",
    );
    let shipping_scope_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.scope.shipping",
        "Shipping",
    );
    let cart_scope_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.scope.cart",
        "Cart",
    );
    let line_item_scope_label = t(
        ui_locale.as_deref(),
        "commerce.cartPromotion.scope.lineItem",
        "Line item",
    );
    let ui_locale_for_promotion_preview = ui_locale.clone();
    let ui_locale_for_promotion_result = ui_locale.clone();

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let shipping_profiles = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get(), search.get()),
        move |(token_value, tenant_value, _, search_value)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_profiles(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                text_or_none(search_value),
            )
            .await
        },
    );

    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_slug.set(String::new());
        set_name.set(String::new());
        set_description.set(String::new());
        set_metadata_json.set(String::new());
    };

    let edit_bootstrap_loading_label = bootstrap_loading_label.clone();
    let submit_bootstrap_loading_label = bootstrap_loading_label.clone();
    let toggle_bootstrap_loading_label = bootstrap_loading_label.clone();
    let promotion_query_writer = query_writer.clone();
    let sync_cart_query = Callback::new(move |_| {
        let cart_id = promotion_cart_id.get_untracked().trim().to_string();
        if cart_id.is_empty() {
            promotion_query_writer.clear_key(AdminQueryKey::CartId.as_str());
        } else {
            promotion_query_writer.replace_value(AdminQueryKey::CartId.as_str(), cart_id);
        }
    });

    let edit_profile = Callback::new(move |profile_id: String| {
        let Some(CommerceAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(edit_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let load_error_label = load_shipping_profile_error_label.clone();
        let not_found_label = not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match api::fetch_shipping_profile(
                token_value,
                tenant_value,
                current_tenant.id,
                profile_id,
            )
            .await
            {
                Ok(Some(profile)) => apply_shipping_profile(
                    &profile,
                    set_editing_id,
                    set_selected,
                    set_slug,
                    set_name,
                    set_description,
                    set_metadata_json,
                ),
                Ok(None) => {
                    clear_shipping_profile_form(
                        set_editing_id,
                        set_selected,
                        set_slug,
                        set_name,
                        set_description,
                        set_metadata_json,
                    );
                    set_error.set(Some(not_found_label));
                }
                Err(err) => {
                    clear_shipping_profile_form(
                        set_editing_id,
                        set_selected,
                        set_slug,
                        set_name,
                        set_description,
                        set_metadata_json,
                    );
                    set_error.set(Some(format!("{load_error_label}: {err}")));
                }
            }
            set_busy.set(false);
        });
    });

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let submit_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();
        let Some(CommerceAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(submit_bootstrap_loading_label.clone()));
            return;
        };
        let Some(submit_locale) = submit_ui_locale.clone() else {
            set_error.set(Some(locale_unavailable_label.clone()));
            return;
        };
        let draft = ShippingProfileDraft {
            slug: slug.get_untracked().trim().to_string(),
            name: name.get_untracked().trim().to_string(),
            description: description.get_untracked().trim().to_string(),
            metadata_json: metadata_json.get_untracked().trim().to_string(),
            locale: submit_locale,
        };
        if draft.slug.is_empty() || draft.name.is_empty() {
            set_error.set(Some(required_label.clone()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = editing_id.get_untracked();
        let save_error_label = save_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = match current_id {
                Some(profile_id) => {
                    api::update_shipping_profile(
                        token_value.clone(),
                        tenant_value.clone(),
                        current_tenant.id.clone(),
                        profile_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_shipping_profile(
                        token_value.clone(),
                        tenant_value.clone(),
                        current_tenant.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match result {
                Ok(profile) => {
                    let profile_id = profile.id.clone();
                    apply_shipping_profile(
                        &profile,
                        set_editing_id,
                        set_selected,
                        set_slug,
                        set_name,
                        set_description,
                        set_metadata_json,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer
                        .replace_value(AdminQueryKey::ShippingProfileId.as_str(), profile_id);
                }
                Err(err) => set_error.set(Some(format!("{save_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    };

    let toggle_profile = Callback::new(move |profile: ShippingProfile| {
        let Some(CommerceAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(toggle_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let toggle_error_label = toggle_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = if profile.active {
                api::deactivate_shipping_profile(
                    token_value,
                    tenant_value,
                    current_tenant.id,
                    profile.id.clone(),
                )
                .await
            } else {
                api::reactivate_shipping_profile(
                    token_value,
                    tenant_value,
                    current_tenant.id,
                    profile.id.clone(),
                )
                .await
            };
            match result {
                Ok(updated) => {
                    if editing_id.get_untracked().as_deref() == Some(profile.id.as_str()) {
                        apply_shipping_profile(
                            &updated,
                            set_editing_id,
                            set_selected,
                            set_slug,
                            set_name,
                            set_description,
                            set_metadata_json,
                        );
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("{toggle_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_summary = ui_locale.clone();
    let initial_edit_profile = edit_profile;
    let list_query_writer = query_writer.clone();
    let reset_query_writer = query_writer.clone();
    let reset_current_profile = Callback::new(move |_| {
        reset_query_writer.clear_key(AdminQueryKey::ShippingProfileId.as_str());
        reset_form();
    });
    Effect::new(move |_| match selected_profile_query.get() {
        Some(profile_id) if !profile_id.trim().is_empty() => {
            if bootstrap.get().and_then(Result::ok).is_none() {
                return;
            }
            initial_edit_profile.run(profile_id);
        }
        _ => {
            clear_shipping_profile_form(
                set_editing_id,
                set_selected,
                set_slug,
                set_name,
                set_description,
                set_metadata_json,
            );
        }
    });
    Effect::new(move |_| match selected_cart_query.get() {
        Some(cart_id) if !cart_id.trim().is_empty() => {
            set_promotion_cart_id.set(cart_id);
        }
        _ => {
            set_promotion_cart_id.set(String::new());
            set_promotion_preview.set(None);
            set_promotion_result.set(None);
        }
    });

    let preview_required_label = promotion_required_label.clone();
    let preview_query_writer = query_writer.clone();
    let preview_promotion = Callback::new(move |_| {
        let cart_id = promotion_cart_id.get_untracked().trim().to_string();
        let source_id = promotion_source_id.get_untracked().trim().to_string();
        if cart_id.is_empty() || source_id.is_empty() {
            set_promotion_error.set(Some(preview_required_label.clone()));
            return;
        }
        preview_query_writer.replace_value(AdminQueryKey::CartId.as_str(), cart_id.clone());
        let draft = CommerceCartPromotionDraft {
            kind: parse_promotion_kind(promotion_kind.get_untracked().as_str()),
            scope: parse_promotion_scope(promotion_scope.get_untracked().as_str()),
            line_item_id: promotion_line_item_id.get_untracked().trim().to_string(),
            source_id,
            discount_percent: promotion_discount_percent
                .get_untracked()
                .trim()
                .to_string(),
            amount: promotion_amount.get_untracked().trim().to_string(),
            metadata_json: promotion_metadata_json.get_untracked().trim().to_string(),
        };
        let preview_error_label = preview_error_label.clone();
        set_promotion_busy.set(true);
        set_promotion_error.set(None);
        spawn_local(async move {
            match api::preview_cart_promotion(cart_id, draft).await {
                Ok(preview) => {
                    set_promotion_preview.set(Some(preview));
                    set_promotion_result.set(None);
                }
                Err(err) => set_promotion_error.set(Some(format!("{preview_error_label}: {err}"))),
            }
            set_promotion_busy.set(false);
        });
    });

    let apply_required_label = promotion_required_label.clone();
    let apply_query_writer = query_writer.clone();
    let apply_promotion = Callback::new(move |_| {
        let cart_id = promotion_cart_id.get_untracked().trim().to_string();
        let source_id = promotion_source_id.get_untracked().trim().to_string();
        if cart_id.is_empty() || source_id.is_empty() {
            set_promotion_error.set(Some(apply_required_label.clone()));
            return;
        }
        apply_query_writer.replace_value(AdminQueryKey::CartId.as_str(), cart_id.clone());
        let draft = CommerceCartPromotionDraft {
            kind: parse_promotion_kind(promotion_kind.get_untracked().as_str()),
            scope: parse_promotion_scope(promotion_scope.get_untracked().as_str()),
            line_item_id: promotion_line_item_id.get_untracked().trim().to_string(),
            source_id,
            discount_percent: promotion_discount_percent
                .get_untracked()
                .trim()
                .to_string(),
            amount: promotion_amount.get_untracked().trim().to_string(),
            metadata_json: promotion_metadata_json.get_untracked().trim().to_string(),
        };
        let apply_error_label = apply_error_label.clone();
        set_promotion_busy.set(true);
        set_promotion_error.set(None);
        spawn_local(async move {
            match api::apply_cart_promotion(cart_id, draft).await {
                Ok(result) => {
                    set_promotion_result.set(Some(result));
                }
                Err(err) => set_promotion_error.set(Some(format!("{apply_error_label}: {err}"))),
            }
            set_promotion_busy.set(false);
        });
    });
    let clear_cart_query_writer = query_writer.clone();

    view! {
        <section class="space-y-6">
            <div class="rounded-3xl border border-border bg-card p-8 shadow-sm">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{badge_label.clone()}</span>
                <h2 class="mt-4 text-3xl font-semibold text-card-foreground">{title_label.clone()}</h2>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{subtitle_label.clone()}</p>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{shipping_profiles_title_label.clone()}</h3>
                            <p class="text-sm text-muted-foreground">{shipping_profiles_subtitle_label.clone()}</p>
                        </div>
                        <div class="flex flex-col gap-3 md:flex-row">
                            <input class="min-w-56 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=shipping_profiles_search_placeholder.clone() prop:value=move || search.get() on:input=move |ev| set_search.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match shipping_profiles.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{no_shipping_profiles_label.clone()}</div> }.into_any(),
                            Some(Ok(list)) => list.items.into_iter().map(|profile| {
                                let item_locale = ui_locale_for_list.clone();
                                let edit_id = profile.id.clone();
                                let toggle_item = profile.clone();
                                let item_query_writer = list_query_writer.clone();
                                let active_label = localized_active_label(item_locale.as_deref(), profile.active);
                                let toggle_label = if profile.active {
                                    t(item_locale.as_deref(), "commerce.action.deactivate", "Deactivate")
                                } else {
                                    t(item_locale.as_deref(), "commerce.action.reactivate", "Reactivate")
                                };
                                let description_text = profile.description.clone().unwrap_or_default();
                                let has_description = profile.description.is_some();
                                view! {
                                    <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                        <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                            <div class="space-y-2">
                                                <div class="flex flex-wrap items-center gap-2">
                                                    <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", active_badge(profile.active))>{active_label}</span>
                                                    <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{profile.slug.clone()}</span>
                                                </div>
                                                <h4 class="text-base font-semibold text-card-foreground">{profile.name.clone()}</h4>
                                                <Show when=move || has_description>
                                                    <p class="text-sm text-muted-foreground">{description_text.clone()}</p>
                                                </Show>
                                                <p class="text-xs text-muted-foreground">{profile.updated_at.clone()}</p>
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| item_query_writer.push_value(AdminQueryKey::ShippingProfileId.as_str(), edit_id.clone())>{edit_label.clone()}</button>
                                                <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| toggle_profile.run(toggle_item.clone())>{toggle_label}</button>
                                            </div>
                                        </div>
                                    </article>
                                }
                            }).collect_view().into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_shipping_profiles_error_label}: {err}")}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if editing_id.get().is_some() { editor_label.clone() } else { create_label.clone() }}</h3>
                            <p class="text-sm text-muted-foreground">{editor_subtitle_label.clone()}</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_current_profile.run(())>{new_label.clone()}</button>
                    </div>
                    <Show when=move || error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_profile>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=slug_placeholder_label.clone() prop:value=move || slug.get() on:input=move |ev| set_slug.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=name_placeholder_label.clone() prop:value=move || name.get() on:input=move |ev| set_name.set(event_target_value(&ev)) />
                        </div>
                        <textarea class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=description_placeholder_label.clone() prop:value=move || description.get() on:input=move |ev| set_description.set(event_target_value(&ev)) />
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=metadata_placeholder_label.clone() prop:value=move || metadata_json.get() on:input=move |ev| set_metadata_json.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>{move || if editing_id.get().is_some() { save_button_label.clone() } else { create_button_label.clone() }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected.get().map(|profile| summarize_shipping_profile(ui_locale_for_summary.as_deref(), &profile)).unwrap_or_else(|| summary_empty_label.clone())}
                    </div>
                    <p class="mt-3 text-xs text-muted-foreground">{metadata_hint_label.clone()}</p>
                </section>
            </div>

            <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                    <div>
                        <h3 class="text-lg font-semibold text-card-foreground">{promotion_title_label.clone()}</h3>
                        <p class="text-sm text-muted-foreground">{promotion_subtitle_label.clone()}</p>
                    </div>
                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || promotion_busy.get() on:click=move |_| {
                        clear_cart_query_writer.clear_key(AdminQueryKey::CartId.as_str());
                        set_promotion_line_item_id.set(String::new());
                        set_promotion_discount_percent.set(String::new());
                        set_promotion_amount.set("4.99".to_string());
                        set_promotion_metadata_json.set(String::new());
                        set_promotion_preview.set(None);
                        set_promotion_result.set(None);
                        set_promotion_error.set(None);
                    }>{clear_cart_label.clone()}</button>
                </div>
                <Show when=move || promotion_error.get().is_some()>
                    <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || promotion_error.get().unwrap_or_default()}</div>
                </Show>
                <div class="mt-5 grid gap-6 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
                    <div class="space-y-4 rounded-2xl border border-border bg-background p-5">
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=cart_id_placeholder_label.clone() prop:value=move || promotion_cart_id.get() on:input=move |ev| set_promotion_cart_id.set(event_target_value(&ev)) on:blur=move |_| sync_cart_query.run(()) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=source_id_placeholder_label.clone() prop:value=move || promotion_source_id.get() on:input=move |ev| set_promotion_source_id.set(event_target_value(&ev)) />
                        </div>
                        <div class="grid gap-4 md:grid-cols-2">
                            <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || promotion_kind.get() on:change=move |ev| set_promotion_kind.set(event_target_value(&ev))>
                                <option value="fixed_discount">{fixed_discount_label.clone()}</option>
                                <option value="percentage_discount">{percentage_discount_label.clone()}</option>
                            </select>
                            <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || promotion_scope.get() on:change=move |ev| set_promotion_scope.set(event_target_value(&ev))>
                                <option value="shipping">{shipping_scope_label.clone()}</option>
                                <option value="cart">{cart_scope_label.clone()}</option>
                                <option value="line_item">{line_item_scope_label.clone()}</option>
                            </select>
                        </div>
                        <div class="grid gap-4 md:grid-cols-3">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=line_item_id_placeholder_label.clone() prop:value=move || promotion_line_item_id.get() on:input=move |ev| set_promotion_line_item_id.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=discount_percent_placeholder_label.clone() prop:value=move || promotion_discount_percent.get() on:input=move |ev| set_promotion_discount_percent.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=amount_placeholder_label.clone() prop:value=move || promotion_amount.get() on:input=move |ev| set_promotion_amount.set(event_target_value(&ev)) />
                        </div>
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=metadata_placeholder_label.clone() prop:value=move || promotion_metadata_json.get() on:input=move |ev| set_promotion_metadata_json.set(event_target_value(&ev)) />
                        <div class="flex flex-wrap gap-3">
                            <button type="button" class="inline-flex rounded-xl border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || promotion_busy.get() on:click=move |_| preview_promotion.run(())>{preview_button_label.clone()}</button>
                            <button type="button" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || promotion_busy.get() on:click=move |_| apply_promotion.run(())>{apply_button_label.clone()}</button>
                        </div>
                    </div>
                    <div class="space-y-4">
                        <div class="rounded-2xl border border-border bg-background p-5">
                            <h4 class="text-base font-semibold text-card-foreground">{preview_title_label.clone()}</h4>
                            {move || promotion_preview.get().map(|preview| render_promotion_preview(ui_locale_for_promotion_preview.as_deref(), preview)).unwrap_or_else(|| view! {
                                <p class="mt-3 text-sm text-muted-foreground">{preview_empty_label.clone()}</p>
                            }.into_any())}
                        </div>
                        <div class="rounded-2xl border border-border bg-background p-5">
                            <h4 class="text-base font-semibold text-card-foreground">{result_title_label.clone()}</h4>
                            {move || promotion_result.get().map(|cart| render_cart_snapshot(ui_locale_for_promotion_result.as_deref(), cart)).unwrap_or_else(|| view! {
                                <p class="mt-3 text-sm text-muted-foreground">{result_empty_label.clone()}</p>
                            }.into_any())}
                        </div>
                    </div>
                </div>
            </section>
        </section>
    }
}

fn apply_shipping_profile(
    profile: &ShippingProfile,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingProfile>>,
    set_slug: WriteSignal<String>,
    set_name: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_metadata_json: WriteSignal<String>,
) {
    set_editing_id.set(Some(profile.id.clone()));
    set_selected.set(Some(profile.clone()));
    set_slug.set(profile.slug.clone());
    set_name.set(profile.name.clone());
    set_description.set(profile.description.clone().unwrap_or_default());
    set_metadata_json.set(profile.metadata.clone());
}

fn clear_shipping_profile_form(
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingProfile>>,
    set_slug: WriteSignal<String>,
    set_name: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_metadata_json: WriteSignal<String>,
) {
    set_editing_id.set(None);
    set_selected.set(None);
    set_slug.set(String::new());
    set_name.set(String::new());
    set_description.set(String::new());
    set_metadata_json.set(String::new());
}

fn summarize_shipping_profile(locale: Option<&str>, profile: &ShippingProfile) -> String {
    format!(
        "{} ({}) | {} | {}",
        profile.name,
        profile.slug,
        localized_active_label(locale, profile.active),
        profile.description.clone().unwrap_or_else(|| t(
            locale,
            "commerce.summary.shippingProfile.noDescription",
            "no description"
        ))
    )
}

fn localized_active_label(locale: Option<&str>, active: bool) -> String {
    if active {
        t(locale, "commerce.common.active", "ACTIVE")
    } else {
        t(locale, "commerce.common.inactive", "INACTIVE")
    }
}

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn active_badge(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}

fn parse_promotion_kind(value: &str) -> CommerceCartPromotionKind {
    match value {
        "percentage_discount" => CommerceCartPromotionKind::PercentageDiscount,
        _ => CommerceCartPromotionKind::FixedDiscount,
    }
}

fn parse_promotion_scope(value: &str) -> CommerceCartPromotionScope {
    match value {
        "cart" => CommerceCartPromotionScope::Cart,
        "line_item" => CommerceCartPromotionScope::LineItem,
        _ => CommerceCartPromotionScope::Shipping,
    }
}

fn render_promotion_preview(
    locale: Option<&str>,
    preview: CommerceCartPromotionPreview,
) -> AnyView {
    view! {
        <div class="mt-3 grid gap-3 md:grid-cols-2">
            <MetricCard title=t(locale, "commerce.cartPromotion.metric.kind", "Kind") value=localized_promotion_kind(locale, &preview.kind) />
            <MetricCard title=t(locale, "commerce.cartPromotion.metric.scope", "Scope") value=localized_promotion_scope(locale, &preview.scope) />
            <MetricCard title=t(locale, "commerce.cartPromotion.metric.lineItem", "Line item") value=preview.line_item_id.unwrap_or_else(|| "-".to_string()) />
            <MetricCard title=t(locale, "commerce.cartPromotion.metric.currency", "Currency") value=preview.currency_code />
            <MetricCard title=t(locale, "commerce.cartPromotion.metric.base", "Base") value=preview.base_amount />
            <MetricCard title=t(locale, "commerce.cartPromotion.metric.adjustment", "Adjustment") value=preview.adjustment_amount />
            <MetricCard title=t(locale, "commerce.cartPromotion.metric.adjusted", "Adjusted") value=preview.adjusted_amount />
        </div>
    }
    .into_any()
}

fn render_cart_snapshot(locale: Option<&str>, cart: CommerceAdminCartSnapshot) -> AnyView {
    view! {
        <div class="mt-3 space-y-4">
            <div class="grid gap-3 md:grid-cols-2">
                <MetricCard title=t(locale, "commerce.cartPromotion.metric.cart", "Cart") value=cart.id.clone() />
                <MetricCard title=t(locale, "commerce.cartPromotion.metric.currency", "Currency") value=cart.currency_code.clone() />
                <MetricCard title=t(locale, "commerce.cartPromotion.metric.shippingTotal", "Shipping total") value=cart.shipping_total.clone() />
                <MetricCard title=t(locale, "commerce.cartPromotion.metric.adjustments", "Adjustments") value=cart.adjustment_total.clone() />
                <MetricCard title=t(locale, "commerce.cartPromotion.metric.total", "Total") value=cart.total_amount.clone() />
                <MetricCard title=t(locale, "commerce.cartPromotion.metric.rows", "Rows") value=cart.adjustments.len().to_string() />
            </div>
            <div class="space-y-3">
                {cart.adjustments.into_iter().map(|adjustment| view! {
                    <article class="rounded-xl border border-border p-4">
                        <div class="grid gap-3 md:grid-cols-2">
                            <MetricCard title=t(locale, "commerce.cartPromotion.metric.source", "Source") value=format!("{} / {}", adjustment.source_type, adjustment.source_id.unwrap_or_else(|| "-".to_string())) />
                            <MetricCard title=t(locale, "commerce.cartPromotion.metric.scope", "Scope") value=localized_promotion_scope_value(locale, adjustment.scope.as_deref()) />
                            <MetricCard title=t(locale, "commerce.cartPromotion.metric.lineItem", "Line item") value=adjustment.line_item_id.unwrap_or_else(|| "-".to_string()) />
                            <MetricCard title=t(locale, "commerce.field.amount", "Amount") value=format!("{} {}", adjustment.currency_code, adjustment.amount) />
                        </div>
                        <pre class="mt-3 overflow-x-auto rounded-lg bg-muted px-3 py-2 text-xs text-muted-foreground">{adjustment.metadata}</pre>
                    </article>
                }).collect_view()}
            </div>
        </div>
    }
    .into_any()
}

fn localized_promotion_kind(locale: Option<&str>, kind: &CommerceCartPromotionKind) -> String {
    match kind {
        CommerceCartPromotionKind::FixedDiscount => t(
            locale,
            "commerce.cartPromotion.kind.fixedDiscount",
            "Fixed discount",
        ),
        CommerceCartPromotionKind::PercentageDiscount => t(
            locale,
            "commerce.cartPromotion.kind.percentageDiscount",
            "Percentage discount",
        ),
    }
}

fn localized_promotion_scope(locale: Option<&str>, scope: &CommerceCartPromotionScope) -> String {
    match scope {
        CommerceCartPromotionScope::Cart => t(locale, "commerce.cartPromotion.scope.cart", "Cart"),
        CommerceCartPromotionScope::LineItem => {
            t(locale, "commerce.cartPromotion.scope.lineItem", "Line item")
        }
        CommerceCartPromotionScope::Shipping => {
            t(locale, "commerce.cartPromotion.scope.shipping", "Shipping")
        }
    }
}

fn localized_promotion_scope_value(locale: Option<&str>, scope: Option<&str>) -> String {
    match scope {
        Some("cart") => t(locale, "commerce.cartPromotion.scope.cart", "Cart"),
        Some("line_item") => t(locale, "commerce.cartPromotion.scope.lineItem", "Line item"),
        Some("shipping") => t(locale, "commerce.cartPromotion.scope.shipping", "Shipping"),
        Some(value) => value.to_string(),
        None => "-".to_string(),
    }
}

#[component]
fn MetricCard(title: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-border p-4">
            <p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{title}</p>
            <p class="mt-2 break-all text-sm text-card-foreground">{value}</p>
        </div>
    }
}
