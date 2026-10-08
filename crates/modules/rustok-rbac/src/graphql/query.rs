use std::collections::{BTreeMap, HashMap};

use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    Action, AuthContext, AuthPrincipalContext, Permission, Resource, StoredLocale, TenantContext,
    graphql::GraphQLError, has_effective_permission,
};
use rustok_core::{Rbac, UserRole, i18n::Locale};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use uuid::Uuid;

use crate::{RbacPresentationResourceKind, RbacPresentationStore, SeaOrmRbacPresentationStore};

use super::control_plane::{prepare_statement, require_direct_control_plane_user};
use super::types::{PlatformPermissionItem, RoleInfo};

#[derive(Default)]
pub struct RbacQuery;

const ALL_ROLES: &[UserRole] = &[
    UserRole::SuperAdmin,
    UserRole::Admin,
    UserRole::Manager,
    UserRole::Customer,
];

fn legacy_seed_display_name(role: &UserRole) -> &'static str {
    match role {
        UserRole::SuperAdmin => "Super Admin",
        UserRole::Admin => "Admin",
        UserRole::Manager => "Manager",
        UserRole::Customer => "Customer",
    }
}

async fn role_display_name(
    store: &SeaOrmRbacPresentationStore,
    tenant_id: uuid::Uuid,
    role: &UserRole,
    requested_locale: &StoredLocale,
) -> Result<String> {
    let resource_key = role.to_string();
    let exact = store
        .find_exact(
            tenant_id,
            RbacPresentationResourceKind::Role,
            &resource_key,
            requested_locale,
        )
        .await
        .map_err(|error| {
            tracing::error!(
                error = %error,
                %tenant_id,
                role = %resource_key,
                locale = %requested_locale.as_str(),
                "RBAC owner presentation read failed"
            );
            FieldError::new("RBAC role presentation is temporarily unavailable")
        })?;
    if let Some(presentation) = exact {
        return Ok(presentation.name);
    }

    let source_locale = StoredLocale::new("en")
        .expect("RBAC built-in presentation source locale must remain concrete");
    if requested_locale != &source_locale {
        let source = store
            .find_exact(
                tenant_id,
                RbacPresentationResourceKind::Role,
                &resource_key,
                &source_locale,
            )
            .await
            .map_err(|error| {
                tracing::error!(
                    error = %error,
                    %tenant_id,
                    role = %resource_key,
                    "RBAC owner source presentation read failed"
                );
                FieldError::new("RBAC role presentation is temporarily unavailable")
            })?;
        if let Some(presentation) = source {
            return Ok(presentation.name);
        }
    }

    // Transitional TR-3 bridge only. This fallback preserves current tenants until
    // built-in bootstrap presentation is seeded through the owner plane. It must be
    // removed together with the legacy native-admin path when the seed cutover lands.
    Ok(legacy_seed_display_name(role).to_string())
}

fn platform_permission_catalog() -> Vec<(Resource, Vec<Action>)> {
    vec![
        (
            Resource::Users,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Tenants,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Modules,
            vec![Action::Read, Action::List, Action::Manage],
        ),
        (
            Resource::Settings,
            vec![Action::Read, Action::Update, Action::List, Action::Manage],
        ),
        (
            Resource::FlexSchemas,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::FlexEntries,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Products,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Categories,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Orders,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Customers,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Profiles,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Groups,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Moderate,
                Action::Manage,
            ],
        ),
        (
            Resource::Regions,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Payments,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Fulfillments,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Inventory,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Discounts,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::MarketplaceSellers,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::MarketplaceListings,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Publish,
                Action::Moderate,
                Action::Manage,
            ],
        ),
        (
            Resource::Posts,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Publish,
                Action::Manage,
            ],
        ),
        (
            Resource::Pages,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Navigation,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Nodes,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Media,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Seo,
            vec![
                Action::Read,
                Action::Update,
                Action::Publish,
                Action::Execute,
                Action::Manage,
            ],
        ),
        (
            Resource::Comments,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Moderate,
                Action::Manage,
            ],
        ),
        (
            Resource::Tags,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Taxonomy,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::Analytics,
            vec![Action::Read, Action::Export, Action::Manage],
        ),
        (
            Resource::Logs,
            vec![Action::Read, Action::List, Action::Manage],
        ),
        (
            Resource::Webhooks,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::BlogPosts,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Publish,
                Action::Manage,
            ],
        ),
        (
            Resource::BlogCategories,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::ForumCategories,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::ForumTopics,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Moderate,
                Action::Manage,
            ],
        ),
        (
            Resource::ForumReplies,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Moderate,
                Action::Manage,
            ],
        ),
        (
            Resource::ModerationCases,
            vec![Action::Read, Action::List, Action::Override, Action::Manage],
        ),
        (
            Resource::Scripts,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Execute,
                Action::Manage,
            ],
        ),
        (
            Resource::Mcp,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (Resource::AiProviders, vec![Action::Read, Action::Manage]),
        (Resource::AiTaskProfiles, vec![Action::Read, Action::Manage]),
        (
            Resource::AiSessions,
            vec![Action::Read, Action::Run, Action::Manage],
        ),
        (Resource::AiRuns, vec![Action::Cancel, Action::Manage]),
        (Resource::AiApprovals, vec![Action::Resolve, Action::Manage]),
        (Resource::AiRouter, vec![Action::Override, Action::Manage]),
        (Resource::AiTextTasks, vec![Action::Run, Action::Manage]),
        (Resource::AiImageTasks, vec![Action::Run, Action::Manage]),
        (Resource::AiCodeTasks, vec![Action::Run, Action::Manage]),
        (Resource::AiAlloyTasks, vec![Action::Run, Action::Manage]),
        (
            Resource::AiMultimodalTasks,
            vec![Action::Run, Action::Manage],
        ),
        (
            Resource::Workflows,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Execute,
                Action::Manage,
            ],
        ),
        (
            Resource::WorkflowExecutions,
            vec![Action::Read, Action::List, Action::Manage],
        ),
        (
            Resource::Translations,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::TranslationMemory,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
        (
            Resource::TranslationGlossaries,
            vec![
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Manage,
            ],
        ),
    ]
}

#[Object]
impl RbacQuery {
    /// List all platform roles with their permission sets.
    /// Requires a direct, session-bound user principal with `settings:read` or `users:read`.
    async fn roles(&self, ctx: &Context<'_>) -> Result<Vec<RoleInfo>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)
            && !has_effective_permission(&auth.permissions, &Permission::USERS_READ)
        {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read or users:read required to list roles",
            ));
        }

        let db = ctx.data::<DatabaseConnection>()?;
        let backend = db.get_database_backend();
        let locale = ctx.data_opt::<Locale>().copied().unwrap_or_default();
        let requested_locale = StoredLocale::new(locale.as_str())
            .expect("canonical GraphQL locale must be a valid stored locale");
        let store = SeaOrmRbacPresentationStore::new(db.clone());

        let sql_roles = "SELECT id, name, slug, description, is_system FROM roles WHERE tenant_id = ? ORDER BY is_system DESC, name ASC";
        let mut role_rows = db
            .query_all_raw(prepare_statement(
                backend,
                sql_roles,
                vec![tenant.id.into()],
            ))
            .await
            .map_err(|error| FieldError::new(format!("Failed to query roles: {error}")))?;

        // If tenant has no roles seeded in DB yet, bootstrap system roles
        if role_rows.is_empty() {
            let _ = crate::repair::repair_system_roles(
                db,
                crate::repair::RbacSystemRoleRepairOptions {
                    tenant_id: Some(tenant.id),
                    apply: true,
                },
            )
            .await;
            role_rows = db
                .query_all_raw(prepare_statement(
                    backend,
                    sql_roles,
                    vec![tenant.id.into()],
                ))
                .await
                .unwrap_or_default();
        }

        if !role_rows.is_empty() {
            let sql_perms = "SELECT rp.role_id, p.resource, p.action FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id WHERE p.tenant_id = ?";
            let perm_rows = db
                .query_all_raw(prepare_statement(
                    backend,
                    sql_perms,
                    vec![tenant.id.into()],
                ))
                .await
                .unwrap_or_default();

            let mut perms_by_role: HashMap<Uuid, Vec<String>> = HashMap::new();
            for row in perm_rows {
                if let (Ok(role_id), Ok(res), Ok(act)) = (
                    row.try_get::<Uuid>("", "role_id"),
                    row.try_get::<String>("", "resource"),
                    row.try_get::<String>("", "action"),
                ) {
                    perms_by_role
                        .entry(role_id)
                        .or_default()
                        .push(format!("{res}:{act}"));
                }
            }

            let mut roles = Vec::with_capacity(role_rows.len());
            for row in role_rows {
                let id: Uuid = row
                    .try_get("", "id")
                    .map_err(|e| FieldError::new(e.to_string()))?;
                let name: String = row
                    .try_get("", "name")
                    .map_err(|e| FieldError::new(e.to_string()))?;
                let slug: String = row
                    .try_get("", "slug")
                    .map_err(|e| FieldError::new(e.to_string()))?;
                let description: Option<String> = row.try_get("", "description").ok();
                let is_system: bool = row.try_get("", "is_system").unwrap_or(false);

                let mut perms = perms_by_role.remove(&id).unwrap_or_default();
                perms.sort();
                perms.dedup();

                let display_name = if is_system {
                    if let Ok(builtin_role) = slug.parse::<UserRole>() {
                        role_display_name(&store, tenant.id, &builtin_role, &requested_locale)
                            .await
                            .unwrap_or(name)
                    } else {
                        name
                    }
                } else {
                    name
                };

                roles.push(RoleInfo {
                    id: Some(id.to_string()),
                    slug,
                    display_name,
                    description,
                    is_system,
                    permissions: perms,
                });
            }

            return Ok(roles);
        }

        // Fallback for static built-in roles
        let mut roles = Vec::with_capacity(ALL_ROLES.len());
        for role in ALL_ROLES {
            let mut perms: Vec<String> = Rbac::permissions_for_role(role)
                .iter()
                .map(|permission| permission.to_string())
                .collect();
            perms.sort();
            roles.push(RoleInfo {
                id: None,
                slug: role.to_string(),
                display_name: role_display_name(&store, tenant.id, role, &requested_locale).await?,
                description: None,
                is_system: true,
                permissions: perms,
            });
        }

        Ok(roles)
    }

    /// List all platform permissions grouped by module / resource.
    /// Requires a direct, session-bound user principal with `settings:read` or `users:read`.
    async fn platform_permissions(&self, ctx: &Context<'_>) -> Result<Vec<PlatformPermissionItem>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)
            && !has_effective_permission(&auth.permissions, &Permission::USERS_READ)
        {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read or users:read required to inspect platform permissions",
            ));
        }

        let mut permissions_map: BTreeMap<String, PlatformPermissionItem> = BTreeMap::new();

        // 1. Seed canonical platform catalog
        for (res, actions) in platform_permission_catalog() {
            let res_str = res.to_string();
            for act in actions {
                let act_str = act.to_string();
                let perm_id = format!("{res_str}:{act_str}");
                permissions_map.insert(
                    perm_id.clone(),
                    PlatformPermissionItem {
                        id: perm_id,
                        resource: res_str.clone(),
                        action: act_str,
                        description: None,
                    },
                );
            }
        }

        // 2. Overlay any database records for tenant (captures custom/artifact permissions & descriptions)
        let db = ctx.data::<DatabaseConnection>()?;
        let backend = db.get_database_backend();
        let perm_query_sql = "SELECT DISTINCT resource, action, description FROM permissions WHERE tenant_id = ? ORDER BY resource ASC, action ASC";
        if let Ok(rows) = db
            .query_all_raw(prepare_statement(
                backend,
                perm_query_sql,
                vec![tenant.id.into()],
            ))
            .await
        {
            for row in rows {
                if let (Ok(res), Ok(act)) = (
                    row.try_get::<String>("", "resource"),
                    row.try_get::<String>("", "action"),
                ) {
                    let desc = row
                        .try_get::<Option<String>>("", "description")
                        .ok()
                        .flatten();
                    let perm_id = format!("{res}:{act}");
                    permissions_map.insert(
                        perm_id.clone(),
                        PlatformPermissionItem {
                            id: perm_id,
                            resource: res,
                            action: act,
                            description: desc,
                        },
                    );
                }
            }
        }

        Ok(permissions_map.into_values().collect())
    }
}
