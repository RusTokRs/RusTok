use leptos::prelude::*;

use super::invitations::GroupsInvitationsAdmin;
use super::leptos::GroupsAdmin as GroupsAdminCore;
use super::localization::GroupsLocalizationAdmin;

#[component]
pub fn GroupsAdmin() -> impl IntoView {
    view! {
        <div class="space-y-6">
            <GroupsAdminCore />
            <GroupsLocalizationAdmin />
            <GroupsInvitationsAdmin />
        </div>
    }
}
