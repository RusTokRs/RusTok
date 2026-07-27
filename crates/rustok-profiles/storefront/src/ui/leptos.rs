use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::AuthContext;
use leptos_ui_routing::read_route_query_value;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    ProfilesStorefrontTransportProfile, normalize_profile_handle, prepare_follow_command,
    recovered_follow_state, selected_transport_profile,
};
use crate::i18n::t;
use crate::model::{ProfilesStorefrontImage, ProfilesStorefrontPage, ProfilesStorefrontProfile};
use crate::transport::{
    ProfilesStorefrontTransportContext, load_profiles_storefront_page,
    set_profiles_storefront_follow,
};

#[component]
pub fn ProfilesView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let auth_context = use_context::<AuthContext>();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "profiles".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile, auth_context.as_ref());
    let requested_handle = read_route_query_value(&route_context, "handle")
        .and_then(|value| normalize_profile_handle(value.as_str()).ok());

    let badge = t(
        locale.as_deref(),
        "profiles.storefront.badge",
        "Public profile",
    );
    let title = t(
        locale.as_deref(),
        "profiles.storefront.title",
        "Discover people",
    );
    let body = t(
        locale.as_deref(),
        "profiles.storefront.body",
        "Open a profile by handle and follow people whose updates you want to see.",
    );
    let search_label = t(
        locale.as_deref(),
        "profiles.storefront.searchLabel",
        "Profile handle",
    );
    let search_placeholder = t(
        locale.as_deref(),
        "profiles.storefront.searchPlaceholder",
        "alice",
    );
    let search_action = t(
        locale.as_deref(),
        "profiles.storefront.searchAction",
        "Open profile",
    );
    let loading = t(
        locale.as_deref(),
        "profiles.storefront.loading",
        "Loading profile...",
    );
    let load_error = t(
        locale.as_deref(),
        "profiles.storefront.loadError",
        "Failed to load profile",
    );
    let transport_label = t(
        locale.as_deref(),
        "profiles.storefront.transport",
        "transport",
    );

    let search = view! {
        <form method="get" action=module_route_base.clone() class="mt-6 flex flex-col gap-3 sm:flex-row sm:items-end">
            <label class="flex-1 text-sm font-medium text-foreground">
                <span class="mb-2 block">{search_label}</span>
                <input
                    name="handle"
                    value=requested_handle.clone().unwrap_or_default()
                    placeholder=search_placeholder
                    autocomplete="off"
                    autocapitalize="none"
                    spellcheck="false"
                    class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-foreground outline-none transition focus:border-primary"
                />
            </label>
            <button type="submit" class="inline-flex min-h-12 items-center justify-center rounded-2xl bg-primary px-5 py-3 text-sm font-semibold text-primary-foreground transition hover:opacity-90">
                {search_action}
            </button>
        </form>
    };

    let Some(handle) = requested_handle else {
        return view! {
            <section class="overflow-hidden rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/40 p-8 shadow-sm">
                <span class="inline-flex rounded-full border border-border bg-background/80 px-3 py-1 text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">{badge}</span>
                <h1 class="mt-4 text-3xl font-semibold text-card-foreground">{title}</h1>
                <p class="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">{body}</p>
                {search}
                <p class="mt-4 text-xs text-muted-foreground">{format!("{transport_label}: {}", profile.as_str())}</p>
            </section>
        }
        .into_any();
    };

    let page_transport = transport.clone();
    let page_locale = locale.clone();
    let page = LocalResource::new(move || {
        let transport = page_transport.clone();
        let handle = handle.clone();
        let locale = page_locale.clone();
        async move { load_profiles_storefront_page(transport, handle, locale).await }
    });

    view! {
        <section class="space-y-6">
            <header class="rounded-[2rem] border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-wrap items-start justify-between gap-4">
                    <div>
                        <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">{badge}</span>
                        <h1 class="mt-3 text-2xl font-semibold text-card-foreground">{title}</h1>
                        <p class="mt-2 text-sm text-muted-foreground">{body}</p>
                    </div>
                    <small class="text-muted-foreground">{format!("{transport_label}: {}", profile.as_str())}</small>
                </div>
                {search}
            </header>

            <Suspense fallback=move || view! {
                <div role="status" aria-live="polite">
                    <div class="h-80 animate-pulse rounded-[2rem] bg-muted" aria-hidden="true"></div>
                    <p class="sr-only">{loading.clone()}</p>
                </div>
            }>
                {move || {
                    let transport = transport.clone();
                    let locale = locale.clone();
                    page.get().map(|result| match result {
                        Ok(page) => view! {
                            <ProfilePanel page transport locale />
                        }.into_any(),
                        Err(_) => view! {
                            <div role="alert" class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {load_error.clone()}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </section>
    }
    .into_any()
}

#[component]
fn ProfilePanel(
    page: ProfilesStorefrontPage,
    transport: ProfilesStorefrontTransportContext,
    locale: Option<String>,
) -> impl IntoView {
    let not_found = t(
        locale.as_deref(),
        "profiles.storefront.notFound",
        "This profile is unavailable.",
    );
    let Some(profile) = page.profile.clone() else {
        return view! {
            <div role="status" class="rounded-[2rem] border border-dashed border-border bg-card p-10 text-center text-muted-foreground">
                {not_found}
            </div>
        }
        .into_any();
    };

    let follow_label = t(locale.as_deref(), "profiles.storefront.follow", "Follow");
    let unfollow_label = t(
        locale.as_deref(),
        "profiles.storefront.unfollow",
        "Unfollow",
    );
    let updating_label = t(
        locale.as_deref(),
        "profiles.storefront.updating",
        "Updating...",
    );
    let sign_in_label = t(
        locale.as_deref(),
        "profiles.storefront.signIn",
        "Sign in to follow this profile.",
    );
    let self_label = t(
        locale.as_deref(),
        "profiles.storefront.self",
        "This is your profile.",
    );
    let unavailable_label = t(
        locale.as_deref(),
        "profiles.storefront.followUnavailable",
        "Follow controls are temporarily unavailable.",
    );
    let recovered_label = t(
        locale.as_deref(),
        "profiles.storefront.followRecovered",
        "The follow state changed elsewhere. We refreshed it; try again.",
    );
    let mutation_failure_label = unavailable_label.clone();
    let bio_fallback = t(
        locale.as_deref(),
        "profiles.storefront.bioFallback",
        "No biography has been added yet.",
    );
    let visibility_label = t(
        locale.as_deref(),
        "profiles.storefront.visibility",
        "Visibility",
    );
    let tags_label = t(locale.as_deref(), "profiles.storefront.tags", "Interests");

    let viewer_authenticated = page.viewer_authenticated;
    let is_self = page.is_self;
    let controls_available = page.follow_state.is_some();
    let (follow_state, set_follow_state) = signal(page.follow_state.clone());
    let (mutation_busy, set_mutation_busy) = signal(false);
    let (mutation_message, set_mutation_message) = signal(Option::<String>::None);
    let target_user_id = profile.user_id.clone();
    let profile_handle = profile.handle.clone();
    let mutation_transport = transport.clone();
    let mutation_locale = locale.clone();
    let on_toggle = move |_| {
        if mutation_busy.get_untracked() {
            return;
        }
        let current_state = follow_state.get_untracked();
        let current = current_state
            .as_ref()
            .map(|state| state.following)
            .unwrap_or(false);
        let expected_revision = current_state.and_then(|state| state.revision);
        let command = match prepare_follow_command(
            target_user_id.as_str(),
            !current,
            expected_revision.as_deref(),
        ) {
            Ok(command) => command,
            Err(_) => {
                set_mutation_message.set(Some(mutation_failure_label.clone()));
                return;
            }
        };
        let transport = mutation_transport.clone();
        let recovery_transport = mutation_transport.clone();
        let recovery_handle = profile_handle.clone();
        let recovery_locale = mutation_locale.clone();
        let recovery_target = target_user_id.clone();
        let failure_label = mutation_failure_label.clone();
        let recovered_label = recovered_label.clone();
        set_mutation_busy.set(true);
        set_mutation_message.set(None);
        spawn_local(async move {
            match set_profiles_storefront_follow(transport, command).await {
                Ok(state) if state.user_id == recovery_target => {
                    set_follow_state.set(Some(state));
                }
                Ok(_) => {
                    set_mutation_message.set(Some(failure_label));
                }
                Err(_) => {
                    let recovered = load_profiles_storefront_page(
                        recovery_transport,
                        recovery_handle,
                        recovery_locale,
                    )
                    .await
                    .ok()
                    .and_then(|page| recovered_follow_state(page, recovery_target.as_str()));
                    if let Some(state) = recovered {
                        set_follow_state.set(Some(state));
                        set_mutation_message.set(Some(recovered_label));
                    } else {
                        set_mutation_message.set(Some(failure_label));
                    }
                }
            }
            set_mutation_busy.set(false);
        });
    };

    let initials = profile_initials(&profile);
    let profile_for_tags = profile.clone();
    let avatar_image = profile.avatar_image.clone();
    let banner_image = profile.banner_image.clone();
    let avatar_alt = profile.display_name.clone();

    view! {
        <article class="overflow-hidden rounded-[2rem] border border-border bg-card shadow-sm">
            <ProfileBanner image=banner_image />
            <div class="p-6 sm:p-8">
                <div class="flex flex-col gap-6 sm:flex-row sm:items-start sm:justify-between">
                    <div class="flex gap-4">
                        <ProfileAvatar image=avatar_image fallback_alt=avatar_alt initials />
                        <div>
                            <h2 class="text-2xl font-semibold text-card-foreground">{profile.display_name.clone()}</h2>
                            <p class="mt-1 text-sm font-medium text-muted-foreground">{format!("@{}", profile.handle)}</p>
                            <p class="mt-4 max-w-2xl text-sm leading-6 text-muted-foreground">
                                {profile.bio.clone().unwrap_or(bio_fallback)}
                            </p>
                        </div>
                    </div>

                    <div class="min-w-44 space-y-2">
                        {if is_self {
                            view! { <p role="status" class="rounded-2xl bg-muted px-4 py-3 text-sm text-muted-foreground">{self_label}</p> }.into_any()
                        } else if !viewer_authenticated {
                            view! { <p role="status" class="rounded-2xl bg-muted px-4 py-3 text-sm text-muted-foreground">{sign_in_label}</p> }.into_any()
                        } else if controls_available {
                            view! {
                                <button
                                    type="button"
                                    disabled=move || mutation_busy.get()
                                    aria-pressed=move || follow_state.get().map(|state| state.following).unwrap_or(false).to_string()
                                    aria-busy=move || mutation_busy.get().to_string()
                                    on:click=on_toggle
                                    class=move || {
                                        let following = follow_state.get().map(|state| state.following).unwrap_or(false);
                                        if following {
                                            "w-full rounded-2xl border border-border bg-background px-5 py-3 text-sm font-semibold text-foreground transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:cursor-not-allowed disabled:opacity-60"
                                        } else {
                                            "w-full rounded-2xl bg-primary px-5 py-3 text-sm font-semibold text-primary-foreground transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:cursor-not-allowed disabled:opacity-60"
                                        }
                                    }
                                >
                                    {move || {
                                        if mutation_busy.get() {
                                            updating_label.clone()
                                        } else if follow_state.get().map(|state| state.following).unwrap_or(false) {
                                            unfollow_label.clone()
                                        } else {
                                            follow_label.clone()
                                        }
                                    }}
                                </button>
                            }.into_any()
                        } else {
                            view! { <p role="status" class="rounded-2xl bg-muted px-4 py-3 text-sm text-muted-foreground">{unavailable_label}</p> }.into_any()
                        }}
                        {move || mutation_message.get().map(|message| view! {
                            <p role="status" aria-live="polite" class="rounded-xl border border-border bg-muted px-3 py-2 text-xs text-muted-foreground">{message}</p>
                        })}
                    </div>
                </div>

                <div class="mt-8 grid gap-4 md:grid-cols-2">
                    <section class="rounded-2xl border border-border bg-background/60 p-4">
                        <p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{visibility_label}</p>
                        <p class="mt-2 text-sm font-medium text-foreground">{profile.visibility}</p>
                    </section>
                    <section class="rounded-2xl border border-border bg-background/60 p-4">
                        <p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{tags_label}</p>
                        <div class="mt-3 flex flex-wrap gap-2">
                            {profile_for_tags.tags.into_iter().map(|tag| view! {
                                <span class="rounded-full bg-muted px-3 py-1 text-xs font-medium text-muted-foreground">{tag}</span>
                            }).collect_view()}
                        </div>
                    </section>
                </div>
            </div>
        </article>
    }
    .into_any()
}

#[component]
fn ProfileBanner(image: Option<ProfilesStorefrontImage>) -> impl IntoView {
    match image {
        Some(image) => view! {
            <img
                src=image.url
                alt=""
                class="h-32 w-full object-cover"
                decoding="async"
            />
        }
        .into_any(),
        None => view! {
            <div class="h-32 bg-gradient-to-r from-primary/25 via-muted to-primary/10" aria-hidden="true"></div>
        }
        .into_any(),
    }
}

#[component]
fn ProfileAvatar(
    image: Option<ProfilesStorefrontImage>,
    fallback_alt: String,
    initials: String,
) -> impl IntoView {
    match image {
        Some(image) => view! {
            <div class="-mt-16 flex h-24 w-24 shrink-0 items-center justify-center overflow-hidden rounded-[1.75rem] border-4 border-card bg-primary text-2xl font-bold text-primary-foreground shadow-lg">
                <img
                    src=image.url
                    alt=image.alt.unwrap_or(fallback_alt)
                    class="h-full w-full object-cover"
                    decoding="async"
                />
            </div>
        }
        .into_any(),
        None => view! {
            <div
                role="img"
                aria-label=fallback_alt
                class="-mt-16 flex h-24 w-24 shrink-0 items-center justify-center overflow-hidden rounded-[1.75rem] border-4 border-card bg-primary text-2xl font-bold text-primary-foreground shadow-lg"
            >
                <span aria-hidden="true">{initials}</span>
            </div>
        }
        .into_any(),
    }
}

fn transport_context(
    profile: ProfilesStorefrontTransportProfile,
    auth_context: Option<&AuthContext>,
) -> ProfilesStorefrontTransportContext {
    match profile {
        ProfilesStorefrontTransportProfile::Native => ProfilesStorefrontTransportContext::native(),
        ProfilesStorefrontTransportProfile::Graphql => {
            let access_token = auth_context.and_then(AuthContext::get_token);
            let tenant_slug = auth_context
                .and_then(AuthContext::get_tenant)
                .or_else(|| option_env!("RUSTOK_TENANT_SLUG").map(str::to_string));
            let current_user_id = auth_context.and_then(|auth| auth.user.get().map(|user| user.id));
            ProfilesStorefrontTransportContext::graphql_with_access_token(
                access_token,
                tenant_slug,
                current_user_id,
            )
        }
    }
}

fn profile_initials(profile: &ProfilesStorefrontProfile) -> String {
    let initials = profile
        .display_name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>();
    if initials.is_empty() {
        profile
            .handle
            .chars()
            .next()
            .map(|value| value.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string())
    } else {
        initials.to_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_initials_use_display_name_then_handle() {
        let profile = ProfilesStorefrontProfile {
            user_id: "user".into(),
            handle: "alice".into(),
            display_name: "Alice Example".into(),
            bio: None,
            tags: Vec::new(),
            avatar_media_id: None,
            banner_media_id: None,
            avatar_image: None,
            banner_image: None,
            preferred_locale: None,
            visibility: "public".into(),
        };
        assert_eq!(profile_initials(&profile), "AE");
    }
}
