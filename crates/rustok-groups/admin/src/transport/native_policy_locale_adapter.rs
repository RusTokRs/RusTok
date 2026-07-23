use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::application_model::{
    GroupsAdminApplicationPolicy, GroupsAdminApplicationPolicyLocaleCatalog,
    GroupsAdminApplicationPolicyLocaleCatalogQuery, GroupsAdminApplicationPolicyManagementView,
    GroupsAdminApplicationPolicyQuery, GroupsAdminUpsertApplicationPolicyResult,
    UpsertGroupApplicationPolicyCommand,
};

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

pub async fn load_group_application_policy_locale_catalog(
    query: GroupsAdminApplicationPolicyLocaleCatalogQuery,
) -> Result<GroupsAdminApplicationPolicyLocaleCatalog, NativeGroupsPolicyLocaleError> {
    groups_admin_application_policy_locale_catalog_native(query)
        .await
        .map_err(Into::into)
}

pub async fn load_group_application_policy_for_management(
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicyManagementView, NativeGroupsPolicyLocaleError> {
    groups_admin_application_policy_for_management_native(query)
        .await
        .map_err(Into::into)
}

pub async fn upsert_group_application_policy(
    command: UpsertGroupApplicationPolicyCommand,
) -> Result<GroupsAdminUpsertApplicationPolicyResult, NativeGroupsPolicyLocaleError> {
    groups_admin_upsert_application_policy_if_current_native(command)
        .await
        .map_err(Into::into)
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/policy-locales"
)]
async fn groups_admin_application_policy_locale_catalog_native(
    query: GroupsAdminApplicationPolicyLocaleCatalogQuery,
) -> Result<GroupsAdminApplicationPolicyLocaleCatalog, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            GroupApplicationPolicyManagementReadPort, GroupApplicationService,
            ListGroupApplicationPolicyLocalesRequest,
        };
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&query.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-policy-management-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let catalog =
            GroupApplicationPolicyManagementReadPort::list_group_application_policy_locales(
                &GroupApplicationService::new(runtime.db_clone()),
                context,
                ListGroupApplicationPolicyLocalesRequest { group_id },
            )
            .await
            .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;
        Ok(GroupsAdminApplicationPolicyLocaleCatalog {
            group_id: catalog.group_id.to_string(),
            policy_id: catalog.policy_id.map(|value| value.to_string()),
            revision: catalog.revision,
            enabled: catalog.enabled,
            locales: catalog.locales,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups admin policy locale catalog native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/policy-management"
)]
async fn groups_admin_application_policy_for_management_native(
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicyManagementView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            GroupApplicationPolicyManagementReadPort, GroupApplicationService,
            ReadGroupApplicationPolicyForManagementRequest,
        };
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&query.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-policy-management-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let view =
            GroupApplicationPolicyManagementReadPort::read_group_application_policy_for_management(
                &GroupApplicationService::new(runtime.db_clone()),
                context,
                ReadGroupApplicationPolicyForManagementRequest {
                    group_id,
                    locale: query.locale,
                },
            )
            .await
            .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;
        Ok(map_management_view(view))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups admin policy management native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/policy-if-current"
)]
async fn groups_admin_upsert_application_policy_if_current_native(
    command: UpsertGroupApplicationPolicyCommand,
) -> Result<GroupsAdminUpsertApplicationPolicyResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            GroupApplicationCasCommandPort, GroupApplicationPolicyPrecondition,
            GroupApplicationQuestion, GroupApplicationRule, GroupApplicationService,
            UpsertGroupApplicationPolicyIfCurrentRequest, UpsertGroupApplicationPolicyRequest,
        };
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&command.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let expected_policy = command
            .expected_policy
            .map(|expected| {
                Ok::<GroupApplicationPolicyPrecondition, ServerFnError>(GroupApplicationPolicyPrecondition {
                    policy_id: Uuid::parse_str(&expected.policy_id)
                        .map_err(|_| ServerFnError::new("policy_id must be a UUID"))?,
                    revision: expected.revision,
                    locale: expected.locale,
                })
            })
            .transpose()?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-policy-cas-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupApplicationCasCommandPort::upsert_group_application_policy_if_current(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            UpsertGroupApplicationPolicyIfCurrentRequest {
                expected_policy,
                policy: UpsertGroupApplicationPolicyRequest {
                    group_id,
                    locale: command.locale,
                    enabled: command.enabled,
                    questions: command
                        .questions
                        .into_iter()
                        .map(|question| GroupApplicationQuestion {
                            key: question.key,
                            prompt: question.prompt,
                            help_text: question.help_text,
                            required: question.required,
                            max_answer_chars: question.max_answer_chars,
                        })
                        .collect(),
                    rules: command
                        .rules
                        .into_iter()
                        .map(|rule| GroupApplicationRule {
                            key: rule.key,
                            title: rule.title,
                            body: rule.body,
                            required: rule.required,
                        })
                        .collect(),
                },
            },
        )
        .await
        .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;
        Ok(GroupsAdminUpsertApplicationPolicyResult {
            policy: map_policy(result.policy),
            group_version: result.group_version,
            created: result.created,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin policy CAS native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_management_view(
    value: rustok_groups::GroupApplicationPolicyManagementView,
) -> GroupsAdminApplicationPolicyManagementView {
    GroupsAdminApplicationPolicyManagementView {
        group_id: value.group_id.to_string(),
        policy_id: value.policy_id.map(|value| value.to_string()),
        revision: value.revision,
        enabled: value.enabled,
        locale: value.locale,
        translation_exists: value.translation_exists,
        questions: value
            .questions
            .into_iter()
            .map(
                |question| crate::application_model::GroupsAdminApplicationQuestion {
                    key: question.key,
                    prompt: question.prompt,
                    help_text: question.help_text,
                    required: question.required,
                    max_answer_chars: question.max_answer_chars,
                },
            )
            .collect(),
        rules: value
            .rules
            .into_iter()
            .map(
                |rule| crate::application_model::GroupsAdminApplicationRule {
                    key: rule.key,
                    title: rule.title,
                    body: rule.body,
                    required: rule.required,
                },
            )
            .collect(),
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
        questions: value
            .questions
            .into_iter()
            .map(
                |question| crate::application_model::GroupsAdminApplicationQuestion {
                    key: question.key,
                    prompt: question.prompt,
                    help_text: question.help_text,
                    required: question.required,
                    max_answer_chars: question.max_answer_chars,
                },
            )
            .collect(),
        rules: value
            .rules
            .into_iter()
            .map(
                |rule| crate::application_model::GroupsAdminApplicationRule {
                    key: rule.key,
                    title: rule.title,
                    body: rule.body,
                    required: rule.required,
                },
            )
            .collect(),
    }
}
