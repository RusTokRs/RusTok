use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    default_groups_storefront_filters, groups_storefront_error, selected_transport_profile,
    GroupsStorefrontTransportProfile,
};
use crate::i18n::t;
use crate::model::GroupsStorefrontDirectory;
use crate::transport::{load_groups_storefront_directory, GroupsStorefrontTransportContext};

#[component]
pub fn GroupsView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);
    let filters = default_groups_storefront_filters();
    let directory = LocalResource::new(move || {
        let context = transport.clone();
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

    view! {
        <section class="groups-storefront">
            <header>
                <h1>{title}</h1>
                <p>{body}</p>
                <small>{format!("transport: {}", profile.as_str())}</small>
            </header>
            <Suspense fallback=move || view! { <p>{loading.clone()}</p> }>
                {move || directory.get().map(|result| match result {
                    Ok(directory) => render_directory(directory, &empty, &members_label).into_any(),
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
) -> impl IntoView {
    if directory.items.is_empty() {
        return view! { <p class="groups-storefront__empty">{empty.to_string()}</p> }.into_any();
    }

    view! {
        <div class="groups-storefront__grid">
            {directory.items.into_iter().map(|group| {
                let summary = group.summary.unwrap_or_default();
                view! {
                    <article class="groups-storefront__card">
                        <h2>{group.title}</h2>
                        <p>{summary}</p>
                        <p>{format!("@{} · {}", group.handle, group.visibility)}</p>
                        <small>{format!("{} {}", group.member_count, members_label)}</small>
                    </article>
                }
            }).collect_view()}
        </div>
    }
    .into_any()
}

fn transport_context(profile: GroupsStorefrontTransportProfile) -> GroupsStorefrontTransportContext {
    match profile {
        GroupsStorefrontTransportProfile::Native => GroupsStorefrontTransportContext::native(),
        GroupsStorefrontTransportProfile::Graphql => GroupsStorefrontTransportContext::graphql(
            option_env!("RUSTOK_TENANT_SLUG").map(str::to_string),
        ),
    }
}
