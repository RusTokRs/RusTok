use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    default_groups_admin_filters, groups_admin_error, groups_admin_header,
    selected_transport_profile, GroupsAdminTransportProfile,
};
use crate::i18n::t;
use crate::model::GroupsAdminDirectory;
use crate::transport::{load_groups_admin_directory, GroupsAdminTransportContext};

#[component]
pub fn GroupsAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);
    let filters = default_groups_admin_filters();
    let directory = LocalResource::new(move || {
        let context = transport.clone();
        let request = filters.clone();
        async move { load_groups_admin_directory(context, request).await }
    });
    let header = groups_admin_header(
        t(locale.as_deref(), "groups.admin.title", "Groups"),
        t(
            locale.as_deref(),
            "groups.admin.body",
            "Manage group privacy, memberships, local roles, and modular feature bindings.",
        ),
        t(
            locale.as_deref(),
            "groups.admin.badge",
            "community control room",
        ),
    );
    let loading = t(
        locale.as_deref(),
        "groups.admin.loading",
        "Loading groups...",
    );
    let load_error = t(
        locale.as_deref(),
        "groups.admin.loadError",
        "Failed to load groups",
    );
    let empty = t(
        locale.as_deref(),
        "groups.admin.empty",
        "No groups are available for this tenant.",
    );
    let total_label = t(locale.as_deref(), "groups.admin.total", "Total");

    view! {
        <section class="groups-admin">
            <header class="groups-admin__header">
                <span>{header.badge}</span>
                <h1>{header.title}</h1>
                <p>{header.body}</p>
                <small>{format!("transport: {}", profile.as_str())}</small>
            </header>
            <Suspense fallback=move || view! { <p>{loading.clone()}</p> }>
                {move || directory.get().map(|result| match result {
                    Ok(directory) => render_directory(directory, &total_label, &empty).into_any(),
                    Err(error) => view! {
                        <p class="groups-admin__error">{groups_admin_error(&load_error, &error.to_string())}</p>
                    }.into_any(),
                })}
            </Suspense>
        </section>
    }
}

fn render_directory(
    directory: GroupsAdminDirectory,
    total_label: &str,
    empty: &str,
) -> impl IntoView {
    if directory.items.is_empty() {
        return view! { <p class="groups-admin__empty">{empty.to_string()}</p> }.into_any();
    }

    view! {
        <div class="groups-admin__directory">
            <p>{format!("{total_label}: {}", directory.total)}</p>
            <ul>
                {directory.items.into_iter().map(|group| view! {
                    <li>
                        <article>
                            <h2>{group.title}</h2>
                            <p>{format!("@{} · {} · {}", group.handle, group.visibility, group.status)}</p>
                            <small>{format!("{} members · {}", group.member_count, group.effective_locale)}</small>
                        </article>
                    </li>
                }).collect_view()}
            </ul>
        </div>
    }
    .into_any()
}

fn transport_context(profile: GroupsAdminTransportProfile) -> GroupsAdminTransportContext {
    match profile {
        GroupsAdminTransportProfile::Native => GroupsAdminTransportContext::native(),
        GroupsAdminTransportProfile::Graphql => GroupsAdminTransportContext::graphql(
            None,
            option_env!("RUSTOK_TENANT_SLUG").map(str::to_string),
        ),
    }
}
