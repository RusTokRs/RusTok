use leptos::prelude::*;
use leptos_auth::AuthContext;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    GroupsStorefrontTransportProfile, default_groups_storefront_filters, groups_storefront_error,
    selected_transport_profile,
};
use crate::i18n::t;
use crate::model::GroupsStorefrontDirectory;
use crate::transport::{GroupsStorefrontTransportContext, load_groups_storefront_directory};
use crate::ui::application::GroupsMembershipApplication;
use crate::ui::invitation_acceptance::GroupsInvitationAcceptance;

#[component]
pub fn GroupsView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let auth_context = use_context::<AuthContext>();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile, auth_context.as_ref());
    let directory_transport = transport.clone();
    let filters = default_groups_storefront_filters();
    let directory = LocalResource::new(move || {
        let context = directory_transport.clone();
        let request = filters.clone();
        async move { load_groups_storefront_directory(context, request).await }
    });
    let title = t(locale.as_deref(), "groups.storefront.title", "Communities");
    let body = t(
        locale.as_deref(),
        "groups.storefront.body",
        "Discover public groups and join communities around shared interests.",
    );
    let loading = t(
        locale.as_deref(),
        "groups.storefront.loading",
        "Loading communities...",
    );
    let load_error = t(
        locale.as_deref(),
        "groups.storefront.loadError",
        "Failed to load communities",
    );
    let empty = t(
        locale.as_deref(),
        "groups.storefront.empty",
        "No public groups are available yet.",
    );
    let members_label = t(locale.as_deref(), "groups.storefront.members", "members");
    let apply_label = t(
        locale.as_deref(),
        "groups.storefront.application.open",
        "Apply to join",
    );

    view! {
        <section class="groups-storefront space-y-8">
            <header>
                <h1>{title}</h1>
                <p>{body}</p>
                <small>{format!("transport: {}", profile.as_str())}</small>
            </header>

            <GroupsInvitationAcceptance transport=transport.clone() />
            <GroupsMembershipApplication transport=transport />

            <Suspense fallback=move || view! { <p>{loading.clone()}</p> }>
                {move || directory.get().map(|result| match result {
                    Ok(directory) => render_directory(directory, &empty, &members_label, &apply_label).into_any(),
                    Err(error) => view! {
                        <p class="groups-storefront__error">{groups_storefront_error(&load_error, &error.to_string())}</p>
                    }.into_any(),
                })}
            </Suspense>
        </section>
    }
}

fn render_directory(
    directory: GroupsStorefrontDirectory,
    empty: &str,
    members_label: &str,
    apply_label: &str,
) -> impl IntoView {
    if directory.items.is_empty() {
        return view! { <p class="groups-storefront__empty">{empty.to_string()}</p> }.into_any();
    }

    view! {
        <div class="groups-storefront__grid">
            {directory.items.into_iter().map(|group| {
                let summary = group.summary.unwrap_or_default();
                let application_href = format!("/modules/groups?apply={}", group.id);
                let can_apply = group.join_policy == "request";
                view! {
                    <article class="groups-storefront__card">
                        <h2>{group.title}</h2>
                        <p>{summary}</p>
                        <p>{format!("@{} · {}", group.handle, group.visibility)}</p>
                        <small>{format!("{} {}", group.member_count, members_label)}</small>
                        <Show when=move || can_apply>
                            <a class="mt-4 inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" href=application_href.clone()>{apply_label.to_string()}</a>
                        </Show>
                    </article>
                }
            }).collect_view()}
        </div>
    }
    .into_any()
}

fn transport_context(
    profile: GroupsStorefrontTransportProfile,
    auth_context: Option<&AuthContext>,
) -> GroupsStorefrontTransportContext {
    match profile {
        GroupsStorefrontTransportProfile::Native => GroupsStorefrontTransportContext::native(),
        GroupsStorefrontTransportProfile::Graphql => {
            let access_token = auth_context.and_then(AuthContext::get_token);
            let tenant_slug = auth_context
                .and_then(AuthContext::get_tenant)
                .or_else(|| option_env!("RUSTOK_TENANT_SLUG").map(str::to_string));
            GroupsStorefrontTransportContext::graphql_with_access_token(access_token, tenant_slug)
        }
    }
}
