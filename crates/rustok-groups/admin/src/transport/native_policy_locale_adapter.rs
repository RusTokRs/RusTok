use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::application_model::{GroupsAdminApplicationPolicy, GroupsAdminApplicationPolicyQuery};

#[derive(Debug, Clone)]
pub struct NativeGroupsPolicyLocaleError(pub String);

impl Display for NativeGroupsPolicyLocaleError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsPolicyLocaleError {}

impl From<ServerFnError> for NativeGroupsPolicyLocaleError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_group_application_policy(
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicy, NativeGroupsPolicyLocaleError> {
    groups_admin_application_policy_locale_native(query)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "groups/admin/applications/policy-locale")]
async fn groups_admin_application_policy_locale_native(
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicy, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext};
        use rustok_groups::{GroupApplicationReadPort, GroupApplicationService, ReadGroupApplicationPolicyRequest};
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>().await.map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>().await.map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&query.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            query.locale,
            format!("groups-admin-policy-locale-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let policy = GroupApplicationReadPort::read_group_application_policy(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            ReadGroupApplicationPolicyRequest { group_id },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(map_policy(policy))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new("groups admin policy locale native transport requires the `ssr` feature"))
    }
}

#[cfg(feature = "ssr")]
fn map_policy(value: rustok_groups::GroupApplicationPolicy) -> GroupsAdminApplicationPolicy {
    GroupsAdminApplicationPolicy {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        revision: value.revision,
        enabled: value.enabled,
        locale: value.locale,
        questions: value.questions.into_iter().map(|question| crate::application_model::GroupsAdminApplicationQuestion {
            key: question.key,
            prompt: question.prompt,
            help_text: question.help_text,
            required: question.required,
            max_answer_chars: question.max_answer_chars,
        }).collect(),
        rules: value.rules.into_iter().map(|rule| crate::application_model::GroupsAdminApplicationRule {
            key: rule.key,
            title: rule.title,
            body: rule.body,
            required: rule.required,
        }).collect(),
    }
}
