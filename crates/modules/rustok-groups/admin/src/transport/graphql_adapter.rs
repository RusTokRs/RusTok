#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::model::{
    ChangeGroupRoleCommand, DeleteGroupTranslationCommand, GroupsAdminDeleteTranslationResult,
    GroupsAdminDirectory, GroupsAdminFilters, GroupsAdminGovernanceResult, GroupsAdminListItem,
    GroupsAdminTranslation, GroupsAdminTranslationMutationResult, GroupsAdminTranslationQuery,
    TransferGroupOwnershipCommand, UpsertGroupTranslationCommand,
};

pub type GraphqlGroupsAdminError = String;

const DIRECTORY_QUERY: &str = "query GroupsAdminDirectory($page: Int, $perPage: Int, $search: String, $includeNonPublic: Boolean) { groups(page: $page, perPage: $perPage, search: $search, includeNonPublic: $includeNonPublic) { total page per_page: perPage items { id handle title visibility join_policy: joinPolicy status member_count: memberCount effective_locale: effectiveLocale } } }";
const CHANGE_ROLE_MUTATION: &str = "mutation GroupsAdminChangeRole($idempotencyKey: String!, $groupId: UUID!, $targetUserId: UUID!, $role: GroupRoleGql!) { change_group_role: changeGroupRole(idempotencyKey: $idempotencyKey, groupId: $groupId, targetUserId: $targetUserId, role: $role) { group_id: groupId actor_user_id: actorUserId target_user_id: targetUserId previous_role: previousRole current_role: currentRole group_version: groupVersion replayed } }";
const TRANSFER_OWNERSHIP_MUTATION: &str = "mutation GroupsAdminTransferOwnership($idempotencyKey: String!, $groupId: UUID!, $newOwnerUserId: UUID!) { transfer_group_ownership: transferGroupOwnership(idempotencyKey: $idempotencyKey, groupId: $groupId, newOwnerUserId: $newOwnerUserId) { group_id: groupId actor_user_id: actorUserId target_user_id: targetUserId previous_role: previousRole current_role: currentRole group_version: groupVersion replayed } }";
const TRANSLATIONS_QUERY: &str = "query GroupsAdminTranslations($groupId: UUID!) { group_translations: groupTranslations(groupId: $groupId) { id group_id: groupId locale title summary body } }";
const UPSERT_TRANSLATION_MUTATION: &str = "mutation GroupsAdminUpsertTranslation($idempotencyKey: String!, $groupId: UUID!, $input: UpsertGroupTranslationInputGql!) { upsert_group_translation: upsertGroupTranslation(idempotencyKey: $idempotencyKey, groupId: $groupId, input: $input) { translation { id group_id: groupId locale title summary body } group_version: groupVersion created } }";
const DELETE_TRANSLATION_MUTATION: &str = "mutation GroupsAdminDeleteTranslation($idempotencyKey: String!, $groupId: UUID!, $locale: String!) { delete_group_translation: deleteGroupTranslation(idempotencyKey: $idempotencyKey, groupId: $groupId, locale: $locale) { group_id: groupId locale group_version: groupVersion } }";

#[derive(Debug, Serialize)]
struct DirectoryVariables {
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
    search: Option<String>,
    #[serde(rename = "includeNonPublic")]
    include_non_public: bool,
}

#[derive(Debug, Serialize)]
struct ChangeRoleVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "targetUserId")]
    target_user_id: String,
    role: String,
}

#[derive(Debug, Serialize)]
struct TransferOwnershipVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "newOwnerUserId")]
    new_owner_user_id: String,
}

#[derive(Debug, Serialize)]
struct TranslationQueryVariables {
    #[serde(rename = "groupId")]
    group_id: String,
}

#[derive(Debug, Serialize)]
struct UpsertTranslationVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    input: UpsertTranslationInput,
}

#[derive(Debug, Serialize)]
struct UpsertTranslationInput {
    locale: String,
    title: String,
    summary: Option<String>,
    body: Option<String>,
}

#[derive(Debug, Serialize)]
struct DeleteTranslationVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    locale: String,
}

#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    groups: DirectoryWire,
}

#[derive(Debug, Deserialize)]
struct ChangeRoleResponse {
    change_group_role: GovernanceWire,
}

#[derive(Debug, Deserialize)]
struct TransferOwnershipResponse {
    transfer_group_ownership: GovernanceWire,
}

#[derive(Debug, Deserialize)]
struct TranslationsResponse {
    group_translations: Vec<TranslationWire>,
}

#[derive(Debug, Deserialize)]
struct UpsertTranslationResponse {
    upsert_group_translation: TranslationMutationWire,
}

#[derive(Debug, Deserialize)]
struct DeleteTranslationResponse {
    delete_group_translation: DeleteTranslationWire,
}

#[derive(Debug, Deserialize)]
struct DirectoryWire {
    items: Vec<GroupWire>,
    total: u64,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct GroupWire {
    id: String,
    handle: String,
    title: String,
    visibility: String,
    join_policy: String,
    status: String,
    member_count: u64,
    effective_locale: String,
}

#[derive(Debug, Deserialize)]
struct GovernanceWire {
    group_id: String,
    actor_user_id: String,
    target_user_id: String,
    previous_role: String,
    current_role: String,
    group_version: u64,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
struct TranslationWire {
    id: String,
    group_id: String,
    locale: String,
    title: String,
    summary: Option<String>,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TranslationMutationWire {
    translation: TranslationWire,
    group_version: u64,
    created: bool,
}

#[derive(Debug, Deserialize)]
struct DeleteTranslationWire {
    group_id: String,
    locale: String,
    group_version: u64,
}

pub async fn load_directory(
    token: Option<String>,
    tenant_slug: Option<String>,
    filters: GroupsAdminFilters,
) -> Result<GroupsAdminDirectory, GraphqlGroupsAdminError> {
    let page = filters.page.max(1);
    let per_page = filters.per_page.clamp(1, 100);
    let response: DirectoryResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            DIRECTORY_QUERY,
            Some(DirectoryVariables {
                page: page.min(i32::MAX as u64) as i32,
                per_page: per_page.min(i32::MAX as u64) as i32,
                search: filters.search,
                include_non_public: filters.include_non_public,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(GroupsAdminDirectory {
        items: response
            .groups
            .items
            .into_iter()
            .map(|group| GroupsAdminListItem {
                id: group.id,
                handle: group.handle,
                title: group.title,
                visibility: normalize_enum(group.visibility),
                join_policy: normalize_enum(group.join_policy),
                status: normalize_enum(group.status),
                member_count: group.member_count,
                effective_locale: group.effective_locale,
            })
            .collect(),
        total: response.groups.total,
        page: response.groups.page,
        per_page: response.groups.per_page,
    })
}

pub async fn change_group_role(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ChangeGroupRoleCommand,
) -> Result<GroupsAdminGovernanceResult, GraphqlGroupsAdminError> {
    let response: ChangeRoleResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            CHANGE_ROLE_MUTATION,
            Some(ChangeRoleVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                target_user_id: command.target_user_id,
                role: command.role.as_graphql_enum().to_string(),
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.change_group_role.into())
}

pub async fn transfer_group_ownership(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: TransferGroupOwnershipCommand,
) -> Result<GroupsAdminGovernanceResult, GraphqlGroupsAdminError> {
    let response: TransferOwnershipResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            TRANSFER_OWNERSHIP_MUTATION,
            Some(TransferOwnershipVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                new_owner_user_id: command.new_owner_user_id,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.transfer_group_ownership.into())
}

pub async fn load_group_translations(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminTranslationQuery,
) -> Result<Vec<GroupsAdminTranslation>, GraphqlGroupsAdminError> {
    let response: TranslationsResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            TRANSLATIONS_QUERY,
            Some(TranslationQueryVariables {
                group_id: query.group_id,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response
        .group_translations
        .into_iter()
        .map(Into::into)
        .collect())
}

pub async fn upsert_group_translation(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: UpsertGroupTranslationCommand,
) -> Result<GroupsAdminTranslationMutationResult, GraphqlGroupsAdminError> {
    let response: UpsertTranslationResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            UPSERT_TRANSLATION_MUTATION,
            Some(UpsertTranslationVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                input: UpsertTranslationInput {
                    locale: command.locale,
                    title: command.title,
                    summary: command.summary,
                    body: command.body,
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.upsert_group_translation.into())
}

pub async fn delete_group_translation(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: DeleteGroupTranslationCommand,
) -> Result<GroupsAdminDeleteTranslationResult, GraphqlGroupsAdminError> {
    let response: DeleteTranslationResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            DELETE_TRANSLATION_MUTATION,
            Some(DeleteTranslationVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                locale: command.locale,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.delete_group_translation.into())
}

impl From<GovernanceWire> for GroupsAdminGovernanceResult {
    fn from(value: GovernanceWire) -> Self {
        Self {
            group_id: value.group_id,
            actor_user_id: value.actor_user_id,
            target_user_id: value.target_user_id,
            previous_role: normalize_enum(value.previous_role),
            current_role: normalize_enum(value.current_role),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

impl From<TranslationWire> for GroupsAdminTranslation {
    fn from(value: TranslationWire) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            locale: value.locale,
            title: value.title,
            summary: value.summary,
            body: value.body,
        }
    }
}

impl From<TranslationMutationWire> for GroupsAdminTranslationMutationResult {
    fn from(value: TranslationMutationWire) -> Self {
        Self {
            translation: value.translation.into(),
            group_version: value.group_version,
            created: value.created,
        }
    }
}

impl From<DeleteTranslationWire> for GroupsAdminDeleteTranslationResult {
    fn from(value: DeleteTranslationWire) -> Self {
        Self {
            group_id: value.group_id,
            locale: value.locale,
            group_version: value.group_version,
        }
    }
}

fn normalize_enum(value: String) -> String {
    value.to_ascii_lowercase()
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}
