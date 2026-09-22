use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    DeleteGroupTranslationCommand, GroupsAdminDeleteTranslationResult, GroupsAdminTranslation,
    GroupsAdminTranslationMutationResult, GroupsAdminTranslationQuery,
    UpsertGroupTranslationCommand,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsLocalizationError(pub String);

impl Display for NativeGroupsLocalizationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsLocalizationError {}

impl From<ServerFnError> for NativeGroupsLocalizationError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_group_translations(
    query: GroupsAdminTranslationQuery,
) -> Result<Vec<GroupsAdminTranslation>, NativeGroupsLocalizationError> {
    groups_admin_translations_native(query)
        .await
        .map_err(Into::into)
}

pub async fn upsert_group_translation(
    command: UpsertGroupTranslationCommand,
) -> Result<GroupsAdminTranslationMutationResult, NativeGroupsLocalizationError> {
    groups_admin_upsert_translation_native(command)
        .await
        .map_err(Into::into)
}

pub async fn delete_group_translation(
    command: DeleteGroupTranslationCommand,
) -> Result<GroupsAdminDeleteTranslationResult, NativeGroupsLocalizationError> {
    groups_admin_delete_translation_native(command)
        .await
        .map_err(Into::into)
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/localization/translations"
)]
async fn groups_admin_translations_native(
    query: GroupsAdminTranslationQuery,
) -> Result<Vec<GroupsAdminTranslation>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            GroupLocalizationReadPort, GroupLocalizationService, ListGroupTranslationsRequest,
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
            format!("groups-admin-localization-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let translations = GroupLocalizationReadPort::list_group_translations(
            &GroupLocalizationService::new(runtime.db_clone()),
            context,
            ListGroupTranslationsRequest { group_id },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(translations
            .into_iter()
            .map(|translation| GroupsAdminTranslation {
                id: translation.id.to_string(),
                group_id: translation.group_id.to_string(),
                locale: translation.locale,
                title: translation.title,
                summary: translation.summary,
                body: translation.body,
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups admin localization native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/localization/upsert-translation"
)]
async fn groups_admin_upsert_translation_native(
    command: UpsertGroupTranslationCommand,
) -> Result<GroupsAdminTranslationMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            GroupLocalizationCommandPort, GroupLocalizationService, UpsertGroupTranslationRequest,
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
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-localization-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupLocalizationCommandPort::upsert_group_translation(
            &GroupLocalizationService::new(runtime.db_clone()),
            context,
            UpsertGroupTranslationRequest {
                group_id,
                locale: command.locale,
                title: command.title,
                summary: command.summary,
                body: command.body,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminTranslationMutationResult {
            translation: GroupsAdminTranslation {
                id: result.translation.id.to_string(),
                group_id: result.translation.group_id.to_string(),
                locale: result.translation.locale,
                title: result.translation.title,
                summary: result.translation.summary,
                body: result.translation.body,
            },
            group_version: result.group_version,
            created: result.created,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin localization native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/localization/delete-translation"
)]
async fn groups_admin_delete_translation_native(
    command: DeleteGroupTranslationCommand,
) -> Result<GroupsAdminDeleteTranslationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            DeleteGroupTranslationRequest, GroupLocalizationCommandPort, GroupLocalizationService,
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
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-localization-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupLocalizationCommandPort::delete_group_translation(
            &GroupLocalizationService::new(runtime.db_clone()),
            context,
            DeleteGroupTranslationRequest {
                group_id,
                locale: command.locale,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminDeleteTranslationResult {
            group_id: result.group_id.to_string(),
            locale: result.locale,
            group_version: result.group_version,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin localization native transport requires the `ssr` feature",
        ))
    }
}
