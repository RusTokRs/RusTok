#[path = "transport/graphql_application_bulk_review_adapter.rs"]
mod graphql_application_bulk_review_adapter;
#[path = "transport/native_application_bulk_review_adapter.rs"]
mod native_application_bulk_review_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath, UiTransportResult};

use crate::application_model::{
    BulkReviewGroupMembershipApplicationsCommand, GroupsAdminBulkReviewApplicationsResult,
};
use crate::core::GroupsAdminTransportProfile;
use crate::transport::GroupsAdminTransportContext;

pub async fn bulk_review_group_admin_membership_applications(
    context: GroupsAdminTransportContext,
    command: BulkReviewGroupMembershipApplicationsCommand,
) -> UiTransportResult<GroupsAdminBulkReviewApplicationsResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    let path = match context.profile {
        GroupsAdminTransportProfile::Native => UiTransportPath::NativeServer,
        GroupsAdminTransportProfile::Graphql => UiTransportPath::Graphql,
    };
    execute_selected_transport(
        "groups.admin.applications.bulk_review",
        path,
        move || {
            native_application_bulk_review_adapter::bulk_review_group_membership_applications(
                native_command,
            )
        },
        move || {
            graphql_application_bulk_review_adapter::bulk_review_group_membership_applications(
                token, tenant, command,
            )
        },
    )
    .await
}
