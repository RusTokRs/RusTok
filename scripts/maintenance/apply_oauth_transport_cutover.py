from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    source = read(path)
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one literal match, found {count}")
    write(path, source.replace(old, new, 1))


def regex_replace_once(path: str, pattern: str, replacement: str) -> None:
    source = read(path)
    updated, count = re.subn(pattern, replacement, source, count=1, flags=re.DOTALL)
    if count != 1:
        raise RuntimeError(f"{path}: expected one regex match, found {count}")
    write(path, updated)


AUTH_ADAPTER = "crates/rustok-auth/admin/src/transport/native_server_adapter.rs"
LEGACY_ADMIN_ADAPTER = "apps/admin/src/features/oauth_apps/transport/native_server_adapter.rs"
CHANNEL_ADAPTER = "crates/rustok-channel/admin/src/transport/native_server_adapter.rs"
CONTRACT = "docs/architecture/database-multilingual-contract.json"
DRIFT = "docs/architecture/database-multilingual-transport-drift.md"
AUDIT = "docs/architecture/database-multilingual-audit.md"


# 1. Auth admin native transport: remove raw projections and route reads/writes through OAuthAdminPort.
regex_replace_once(
    AUTH_ADAPTER,
    r'\#\[cfg\(feature = "ssr"\)\]\nfn parse_json_list\(.*?\n\#\[server\(prefix = "/api/fn", endpoint = "admin/list-oauth-apps"\)\]',
    '''#[cfg(feature = "ssr")]
fn parse_app_type(value: &str) -> AppType {
    match value {
        "embedded" => AppType::Embedded,
        "first_party" => AppType::FirstParty,
        "mobile" => AppType::Mobile,
        "service" => AppType::Service,
        _ => AppType::ThirdParty,
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-oauth-apps")]''',
)

regex_replace_once(
    AUTH_ADAPTER,
    r'\#\[server\(prefix = "/api/fn", endpoint = "admin/list-oauth-apps"\)\]\npub async fn list_oauth_apps_native\(.*?\n}\n\n\#\[cfg\(feature = "ssr"\)\]\nfn oauth_app_from_mutation_record',
    '''#[server(prefix = "/api/fn", endpoint = "admin/list-oauth-apps")]
pub async fn list_oauth_apps_native(limit: i64) -> Result<Vec<OAuthApp>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = oauth_mutation_context().await?;
        let limit = u64::try_from(limit.clamp(1, 100))
            .map_err(|_| server_error("OAuth app limit is out of range"))?;
        let records = runtime
            .port()
            .list_oauth_apps(&context, None, limit)
            .await
            .map_err(|error| server_error(error.to_string()))?;
        Ok(records
            .into_iter()
            .map(oauth_app_from_mutation_record)
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "admin/list-oauth-apps requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn oauth_app_from_mutation_record''',
)

replace_once(
    AUTH_ADAPTER,
    '''    use leptos::prelude::expect_context;
    use rustok_api::AuthContext;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| server_error(error.to_string()))?;
    let app_ctx = AuthAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
''',
    '''    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, RequestContext};

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| server_error(error.to_string()))?;
    let request_context = leptos_axum::extract::<RequestContext>()
        .await
        .map_err(|error| server_error(error.to_string()))?;
    let app_ctx = AuthAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
''',
)
replace_once(
    AUTH_ADAPTER,
    '''            request_id: None,
            locale: None,
''',
    '''            request_id: None,
            locale: Some(request_context.locale),
''',
)

# 2. Legacy admin transport delegates to the owner admin package instead of querying OAuth tables.
write(
    LEGACY_ADMIN_ADAPTER,
    '''use leptos::prelude::*;

use crate::entities::oauth_app::model::{AppType, OAuthApp};

#[cfg(feature = "ssr")]
fn map_app_type(value: rustok_auth_admin::model::AppType) -> AppType {
    match value {
        rustok_auth_admin::model::AppType::Embedded => AppType::Embedded,
        rustok_auth_admin::model::AppType::FirstParty => AppType::FirstParty,
        rustok_auth_admin::model::AppType::Mobile => AppType::Mobile,
        rustok_auth_admin::model::AppType::Service => AppType::Service,
        rustok_auth_admin::model::AppType::ThirdParty => AppType::ThirdParty,
    }
}

#[cfg(feature = "ssr")]
fn map_oauth_app(value: rustok_auth_admin::model::OAuthApp) -> OAuthApp {
    OAuthApp {
        id: value.id,
        name: value.name,
        slug: value.slug,
        description: value.description,
        icon_url: value.icon_url,
        app_type: map_app_type(value.app_type),
        client_id: value.client_id,
        redirect_uris: value.redirect_uris,
        scopes: value.scopes,
        grant_types: value.grant_types,
        manifest_ref: value.manifest_ref,
        auto_created: value.auto_created,
        managed_by_manifest: value.managed_by_manifest,
        is_active: value.is_active,
        can_edit: value.can_edit,
        can_rotate_secret: value.can_rotate_secret,
        can_revoke: value.can_revoke,
        active_token_count: value.active_token_count,
        last_used_at: value.last_used_at,
        created_at: value.created_at,
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-oauth-apps")]
pub(super) async fn list_oauth_apps_native(limit: i64) -> Result<Vec<OAuthApp>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        rustok_auth_admin::transport::native_server_adapter::list_oauth_apps_native(limit)
            .await
            .map(|apps| apps.into_iter().map(map_oauth_app).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "admin/list-oauth-apps requires the `ssr` feature",
        ))
    }
}
''',
)

# 3. Channel bootstrap keeps its broader permission contract but performs an exact locale join.
replace_once(
    CHANNEL_ADAPTER,
    '''        use rustok_api::{AuthContext, OptionalChannel, TenantContext};
''',
    '''        use rustok_api::{
            AuthContext, OptionalChannel, RequestContext, TenantContext, normalize_locale_tag,
        };
''',
)
replace_once(
    CHANNEL_ADAPTER,
    '''        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let current_channel = leptos_axum::extract::<OptionalChannel>()
''',
    '''        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let current_channel = leptos_axum::extract::<OptionalChannel>()
''',
)

regex_replace_once(
    CHANNEL_ADAPTER,
    r'        let stmt = Statement::from_sql_and_values\(\n            DbBackend::Postgres,.*?            \.collect::<Result<Vec<_>, _>>\(\)\?;',
    '''        let effective_locale = normalize_locale_tag(&request_context.locale)
            .filter(|locale| locale != "und")
            .ok_or_else(|| {
                ServerFnError::new(
                    "channel OAuth bootstrap requires a normalized effective locale other than `und`",
                )
            })?;
        let backend = db.get_database_backend();
        let stmt = match backend {
            DbBackend::Postgres => Statement::from_sql_and_values(
                backend,
                r#"
                SELECT oa.id, oat.name, oa.slug, oa.app_type, oa.is_active
                FROM oauth_apps oa
                LEFT JOIN oauth_app_translations oat
                  ON oat.tenant_id = oa.tenant_id
                 AND oat.app_id = oa.id
                 AND oat.locale = $2
                WHERE oa.tenant_id = $1
                  AND oa.is_active = TRUE
                  AND oa.revoked_at IS NULL
                ORDER BY oa.slug ASC
                "#,
                vec![tenant.id.into(), effective_locale.clone().into()],
            ),
            DbBackend::MySql => Statement::from_sql_and_values(
                backend,
                r#"
                SELECT oa.id, oat.name, oa.slug, oa.app_type, oa.is_active
                FROM oauth_apps oa
                LEFT JOIN oauth_app_translations oat
                  ON oat.tenant_id = oa.tenant_id
                 AND oat.app_id = oa.id
                 AND oat.locale = ?
                WHERE oa.tenant_id = ?
                  AND oa.is_active = TRUE
                  AND oa.revoked_at IS NULL
                ORDER BY oa.slug ASC
                "#,
                vec![effective_locale.clone().into(), tenant.id.into()],
            ),
            DbBackend::Sqlite => Statement::from_sql_and_values(
                backend,
                r#"
                SELECT oa.id, oat.name, oa.slug, oa.app_type, oa.is_active
                FROM oauth_apps oa
                LEFT JOIN oauth_app_translations oat
                  ON oat.tenant_id = oa.tenant_id
                 AND oat.app_id = oa.id
                 AND oat.locale = ?2
                WHERE oa.tenant_id = ?1
                  AND oa.is_active = 1
                  AND oa.revoked_at IS NULL
                ORDER BY oa.slug ASC
                "#,
                vec![tenant.id.into(), effective_locale.clone().into()],
            ),
        };
        let oauth_rows = db.query_all(stmt).await.map_err(ServerFnError::new)?;
        let oauth_apps = oauth_rows
            .into_iter()
            .map(
                |row: QueryResult| -> Result<AvailableOauthAppItem, ServerFnError> {
                    let app_id = row
                        .try_get::<uuid::Uuid>("", "id")
                        .map_err(ServerFnError::new)?;
                    let name = row
                        .try_get::<Option<String>>("", "name")
                        .map_err(ServerFnError::new)?
                        .ok_or_else(|| {
                            ServerFnError::new(format!(
                                "OAuth app translation missing: app {app_id}, locale `{effective_locale}`"
                            ))
                        })?;
                    Ok(AvailableOauthAppItem {
                        id: app_id.to_string(),
                        name,
                        slug: row
                            .try_get::<String>("", "slug")
                            .map_err(ServerFnError::new)?,
                        app_type: row
                            .try_get::<String>("", "app_type")
                            .map_err(ServerFnError::new)?,
                        is_active: row
                            .try_get::<bool>("", "is_active")
                            .map_err(ServerFnError::new)?,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;''',
)

# 4. Promote the transport cutover to an executable guarded surface.
contract = json.loads(read(CONTRACT))
oauth_surface = next(
    surface for surface in contract["guarded_surfaces"] if surface["id"] == "oauth_apps"
)
new_files = [
    {
        "path": AUTH_ADAPTER,
        "required_markers": [
            "let (context, runtime) = oauth_mutation_context().await?;",
            ".list_oauth_apps(&context, None, limit)",
            "locale: Some(request_context.locale)",
        ],
        "forbidden_markers": ["oa.name", "oa.description"],
    },
    {
        "path": LEGACY_ADMIN_ADAPTER,
        "required_markers": [
            "rustok_auth_admin::transport::native_server_adapter::list_oauth_apps_native",
            "map_oauth_app",
        ],
        "forbidden_markers": ["FROM oauth_apps", "oa.name", "oa.description"],
    },
    {
        "path": CHANNEL_ADAPTER,
        "required_markers": [
            "oauth_app_translations oat",
            "normalize_locale_tag(&request_context.locale)",
            "oat.locale =",
            "OAuth app translation missing",
        ],
        "forbidden_markers": [
            "SELECT id, name, slug, app_type, is_active",
            "oa.name",
            "oa.description",
        ],
    },
]
existing_paths = {entry["path"] for entry in oauth_surface["files"]}
for entry in new_files:
    if entry["path"] not in existing_paths:
        oauth_surface["files"].append(entry)
write(CONTRACT, json.dumps(contract, ensure_ascii=False, indent=2) + "\n")

write(
    DRIFT,
    '''# Multilingual database transport drift

Last reviewed: 2026-07-21

This document records transport/read-model compatibility with the accepted
language-agnostic database schema. It is not permission to restore localized
copy to base tables.

## OAuth application copy — resolved

The authoritative schema removes `oauth_apps.name` and
`oauth_apps.description`. Localized presentation copy lives in
`oauth_app_translations`, and runtime `und` fallback is forbidden.

The three formerly incompatible adapters are now cut over:

- `crates/rustok-auth/admin/src/transport/native_server_adapter.rs` calls the
  owner `OAuthAdminPort` with the host-resolved `RequestContext.locale`;
- `apps/admin/src/features/oauth_apps/transport/native_server_adapter.rs`
  delegates to the owner Auth admin transport instead of querying OAuth tables;
- `crates/rustok-channel/admin/src/transport/native_server_adapter.rs` keeps
  the Channel permission boundary and performs an exact
  `(tenant_id, app_id, locale)` translation join for PostgreSQL, MySQL, and
  SQLite.

All paths reject absent, invalid, or storage-only `und` runtime locale. They do
not select English, tenant default, `und`, or an arbitrary translation row.
Missing exact copy fails closed rather than silently hiding or relabeling an
OAuth application.

The executable guard is part of the `oauth_apps` surface in
`database-multilingual-contract.json`; the standard DB verifier rejects any
return of raw `oauth_apps.name` / `oauth_apps.description` projections.
''',
)

replace_once(
    AUDIT,
    '''- **OAuth applications** — protocol identity and credentials remain in
  `oauth_apps`; name and description live in tenant-safe translation rows.
  Legacy copy is retained as `und`. Manual writes require effective locale and
  commit base state plus translation atomically; manifest-generated English copy
  uses explicit `en`; runtime reads never return `und` as a translation fallback.
''',
    '''- **OAuth applications** — protocol identity and credentials remain in
  `oauth_apps`; name and description live in tenant-safe translation rows.
  Legacy copy is retained as `und`. Manual writes require effective locale and
  commit base state plus translation atomically; manifest-generated English copy
  uses explicit `en`; runtime reads never return `und` as a translation fallback.
  Auth admin reads now use the owner port, the legacy admin delegates to that
  transport, and Channel bootstrap performs an exact locale translation join on
  PostgreSQL, MySQL, and SQLite. The verifier forbids removed base-copy projections.
''',
)

print("Applied OAuth multilingual transport cutover")
