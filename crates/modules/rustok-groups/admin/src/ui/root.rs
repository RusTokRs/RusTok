use leptos::prelude::*;

use super::application_bulk_review::GroupsApplicationsBulkReviewAdmin;
use super::applications::GroupsApplicationsAdmin;
use super::invitations::GroupsInvitationsAdmin;
use super::leptos::GroupsAdmin as GroupsAdminCore;
use super::localization::GroupsLocalizationAdmin;
use super::policy_editor::GroupsPolicyEditorAdmin;

#[component]
pub fn GroupsAdmin() -> impl IntoView {
    view! {
        <div class="space-y-6">
            <GroupsAdminCore />
            <GroupsLocalizationAdmin />
            <GroupsPolicyEditorAdmin />
            <GroupsApplicationsAdmin />
            <GroupsApplicationsBulkReviewAdmin />
            <GroupsInvitationsAdmin />
        </div>
    }
}
