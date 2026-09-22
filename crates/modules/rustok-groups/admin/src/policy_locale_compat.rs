use rustok_ui_transport::{UiTransportError, UiTransportPath, UiTransportResult};

use crate::application_model::{GroupsAdminApplicationPolicy, GroupsAdminApplicationPolicyQuery};
use crate::core::GroupsAdminTransportProfile;
use crate::transport::{
    GroupsAdminTransportContext, load_group_admin_application_policy_for_management,
};

pub async fn load_group_admin_application_policy(
    context: GroupsAdminTransportContext,
    query: GroupsAdminApplicationPolicyQuery,
) -> UiTransportResult<GroupsAdminApplicationPolicy> {
    let path = match context.profile {
        GroupsAdminTransportProfile::Native => UiTransportPath::NativeServer,
        GroupsAdminTransportProfile::Graphql => UiTransportPath::Graphql,
    };
    let view = load_group_admin_application_policy_for_management(context, query).await?;
    let Some(policy_id) = view.policy_id else {
        return Err(compatibility_error(
            path,
            "membership application policy does not exist",
        ));
    };
    let Some(revision) = view.revision else {
        return Err(compatibility_error(
            path,
            "membership application policy revision is unavailable",
        ));
    };
    if !view.translation_exists {
        return Err(compatibility_error(
            path,
            "selected membership application policy translation does not exist",
        ));
    }
    Ok(GroupsAdminApplicationPolicy {
        id: policy_id,
        group_id: view.group_id,
        revision,
        enabled: view.enabled,
        locale: view.locale,
        questions: view.questions,
        rules: view.rules,
    })
}

fn compatibility_error(path: UiTransportPath, message: &str) -> UiTransportError {
    match path {
        UiTransportPath::NativeServer => {
            UiTransportError::native("groups.admin.applications.policy.read", message)
        }
        UiTransportPath::Graphql => {
            UiTransportError::graphql("groups.admin.applications.policy.read", message)
        }
    }
}
