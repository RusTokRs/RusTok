mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{CommerceAdminBootstrap, ShippingProfile, ShippingProfileDraft};

#[component]
pub fn CommerceAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
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

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let shipping_profiles = Resource::new(
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
                Ok(None) => set_error.set(Some(not_found_label)),
                Err(err) => set_error.set(Some(format!("{load_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let submit_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(CommerceAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(submit_bootstrap_loading_label.clone()));
            return;
        };
        let draft = ShippingProfileDraft {
            slug: slug.get_untracked().trim().to_string(),
            name: name.get_untracked().trim().to_string(),
            description: description.get_untracked().trim().to_string(),
            metadata_json: metadata_json.get_untracked().trim().to_string(),
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
                                                <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| edit_profile.run(edit_id.clone())>{edit_label.clone()}</button>
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
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_form()>{new_label.clone()}</button>
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
