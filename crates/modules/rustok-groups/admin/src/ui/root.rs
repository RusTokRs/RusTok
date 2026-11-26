use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use super::application_bulk_review::GroupsApplicationsBulkReviewAdmin;
use super::applications::GroupsApplicationsAdmin;
use super::invitations::GroupsInvitationsAdmin;
use super::leptos::GroupsAdmin as GroupsAdminCore;
use super::localization::GroupsLocalizationAdmin;
use super::policy_editor::GroupsPolicyEditorAdmin;
use crate::core::{GroupsAdminSection, groups_admin_section_for_subpath};

/// Groups admin root.
///
/// The host renders one module component for every declared `child_pages`
/// entry (`memberships`, `applications`, `invitations`, `features`), so the
/// module owns the subpath dispatch. Each workspace is its own component, and
/// rendering only the selected one is what makes the secondary navigation
/// links mean something — they used to all land on the same stacked markup.
#[component]
pub fn GroupsAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let section = groups_admin_section_for_subpath(route_context.subpath.as_deref());
    // The active section is also visible in the DOM: a host that scrolls,
    // highlights navigation or runs a smoke test can assert on it without
    // parsing the rendered workspace.
    let section_name = match section {
        GroupsAdminSection::Memberships => "memberships",
        GroupsAdminSection::Applications => "applications",
        GroupsAdminSection::Invitations => "invitations",
        GroupsAdminSection::Features => "features",
    };

    let workspace = match section {
        GroupsAdminSection::Memberships => view! { <GroupsAdminCore /> }.into_any(),
        GroupsAdminSection::Applications => view! {
            <GroupsApplicationsAdmin />
            <GroupsApplicationsBulkReviewAdmin />
        }
        .into_any(),
        GroupsAdminSection::Invitations => view! { <GroupsInvitationsAdmin /> }.into_any(),
        GroupsAdminSection::Features => view! {
            <GroupsLocalizationAdmin />
            <GroupsPolicyEditorAdmin />
        }
        .into_any(),
    };

    view! {
        <div class="space-y-6" data-groups-section=section_name>
            {workspace}
        </div>
    }
}
