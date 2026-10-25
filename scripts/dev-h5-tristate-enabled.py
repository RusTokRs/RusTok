"""H-5 (M-5 residual): tri-state tenant_modules.enabled.

NULL = settings without recorded enablement intent (inherit platform
default). Applied with assertions; retained as evidence.
"""

ROOT = '/home/user/RusTok/'


def edit(rel, pairs):
    p = ROOT + rel
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (rel, old[:80])
        s = s.replace(old, new)
        print(f'{rel}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


M = 'crates/modules/rustok-modules/src/'

# ---------------------------------------------------------------- migration
edit(M + 'migrations/m20250101_000060_create_tenant_modules.rs', [
    ('''                    .col(
                        ColumnDef::new(TenantModules::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )''',
     '''                    // Tri-state enablement intent: NULL records "no tenant
                    // intent" (the module inherits the platform default);
                    // only an explicit toggle stores true/false. A settings
                    // save must never freeze a default into intent.
                    .col(ColumnDef::new(TenantModules::Enabled).boolean().null())'''),
])

# ------------------------------------------------------------ operation_store
edit(M + 'operation_store.rs', [
    ('''pub struct TenantModuleStateRecord {
    pub id: Uuid,
    /// Explicit enablement persisted by this operation.
    pub enabled: bool,
    /// Enablement recorded before this operation, or `None` when no
    /// `tenant_modules` row existed. (Previously the insert path fabricated
    /// `!enabled` here, inventing a predecessor state that never existed.)
    pub previous_enabled: Option<bool>,
    pub changed: bool,
}''',
     '''pub struct TenantModuleStateRecord {
    pub id: Uuid,
    /// Tenant enablement intent recorded after this operation, or `None`
    /// when the tenant has recorded no intent and the module inherits the
    /// platform default.
    pub enabled: Option<bool>,
    /// Tenant enablement intent recorded before this operation, or `None`
    /// when none was recorded (no `tenant_modules` row existed, or the row
    /// carried settings without intent). (Previously the insert path
    /// fabricated `!enabled` here, inventing a predecessor state that never
    /// existed.)
    pub previous_enabled: Option<bool>,
    pub changed: bool,
}'''),
    ('''pub struct TenantModuleSettingsRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub settings: serde_json::Value,
    /// Enablement recorded only when this save creates the tenant's first
    /// row for the module. Callers pass the platform default (`true` for Core
    /// and default-enabled modules, `false` otherwise). Settings persistence
    /// never changes recorded enablement intent for an existing row and never
    /// freezes derived effective availability into intent;
    /// see [`TenantModuleStateStore::persist_settings`].
    pub default_enabled: bool,
}''',
     '''pub struct TenantModuleSettingsRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub settings: serde_json::Value,
}'''),
    ('''pub struct TenantModuleSettingsRecord {
    pub id: Uuid,
    pub module_slug: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
}''',
     '''pub struct TenantModuleSettingsRecord {
    pub id: Uuid,
    pub module_slug: String,
    /// Recorded tenant enablement intent after the save, or `None` when the
    /// row carries settings without intent (platform default applies).
    pub enabled: Option<bool>,
    pub settings: serde_json::Value,
}'''),
    # read()
    ('''        .map(|row| {
            let enabled: bool = row.try_get("", "enabled").map_err(database_error)?;
            Ok(TenantModuleStateRecord {
                id: row.try_get("", "id").map_err(database_error)?,
                enabled,
                previous_enabled: Some(enabled),
                changed: false,
            })
        })''',
     '''        .map(|row| {
            let enabled: Option<bool> = row.try_get("", "enabled").map_err(database_error)?;
            Ok(TenantModuleStateRecord {
                id: row.try_get("", "id").map_err(database_error)?,
                enabled,
                previous_enabled: enabled,
                changed: false,
            })
        })'''),
    # persist()
    ('''            let previous_enabled = row
                .try_get("", "enabled")
                .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
            if previous_enabled != request.enabled {''',
     '''            let previous_enabled: Option<bool> = row
                .try_get("", "enabled")
                .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
            if previous_enabled != Some(request.enabled) {'''),
    ('''            return Ok(TenantModuleStateRecord {
                id,
                enabled: request.enabled,
                previous_enabled: Some(previous_enabled),
                changed: previous_enabled != request.enabled,
            });''',
     '''            return Ok(TenantModuleStateRecord {
                id,
                enabled: Some(request.enabled),
                previous_enabled,
                changed: previous_enabled != Some(request.enabled),
            });'''),
    ('''        Ok(TenantModuleStateRecord {
            id,
            enabled: request.enabled,
            // The row was just created: there is no predecessor state.
            previous_enabled: None,
            changed: true,
        })''',
     '''        Ok(TenantModuleStateRecord {
            id,
            enabled: Some(request.enabled),
            // The row was just created: there is no predecessor state.
            previous_enabled: None,
            changed: true,
        })'''),
    # persist_settings() existing row
    ('''            let id: Uuid = row.try_get("", "id").map_err(database_error)?;
            let previous_enabled: bool = row.try_get("", "enabled").map_err(database_error)?;
            // A settings save never changes recorded enablement intent.
            // (Core modules stay available through the effective policy,
            // not through this flag.)
            let enabled = previous_enabled;
            execute(
                db,
                "UPDATE tenant_modules SET enabled = {1}, settings = {2}, updated_at = CURRENT_TIMESTAMP WHERE id = {3}",
                vec![
                    enabled.into(),
                    json_value(request.settings),
                    id.into(),
                ],
            )
            .await?;''',
     '''            let id: Uuid = row.try_get("", "id").map_err(database_error)?;
            let enabled: Option<bool> = row.try_get("", "enabled").map_err(database_error)?;
            // A settings save never changes recorded enablement intent:
            // the column is not part of this UPDATE at all.
            execute(
                db,
                "UPDATE tenant_modules SET settings = {1}, updated_at = CURRENT_TIMESTAMP WHERE id = {2}",
                vec![json_value(request.settings), id.into()],
            )
            .await?;'''),
    # persist_settings() insert
    ('''        let id = rustok_core::generate_id();
        let enabled = request.default_enabled;
        execute(
            db,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES ({1}, {2}, {3}, {4}, {5})",
            vec![
                id.into(),
                backend.uuid_value(request.tenant_id),
                request.module_slug.into(),
                enabled.into(),
                json_value(request.settings),
            ],
        )
        .await?;
        Ok(TenantModuleSettingsRecord {
            id,
            module_slug,
            enabled,
            settings,
        })''',
     '''        let id = rustok_core::generate_id();
        // A first row created by a settings save records no enablement
        // intent (NULL): the platform default keeps applying until the
        // tenant explicitly toggles the module.
        execute(
            db,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES ({1}, {2}, {3}, {4}, {5})",
            vec![
                id.into(),
                backend.uuid_value(request.tenant_id),
                request.module_slug.into(),
                Option::<bool>::None.into(),
                json_value(request.settings),
            ],
        )
        .await?;
        Ok(TenantModuleSettingsRecord {
            id,
            module_slug,
            enabled: None,
            settings,
        })'''),
    # persist_settings() doc
    ('''    /// Persists tenant module settings without changing enablement intent.
    ///
    /// An existing row keeps its recorded enablement. A newly created row
    /// records `request.default_enabled` (the platform/Core default), never
    /// the module's derived effective availability: freezing a transient
    /// denial (unavailable dependency, security hold, ...) as explicit
    /// tenant intent would outlive the condition that caused it. The boolean
    /// `enabled` column cannot express "settings without intent", so
    /// first-row creation necessarily records the default; splitting intent
    /// from settings storage is tracked with module schema ownership
    /// (audit M-12).''',
     '''    /// Persists tenant module settings without creating or changing
    /// enablement intent.
    ///
    /// An existing row keeps its recorded intent untouched (the column is
    /// not part of the UPDATE). A newly created row stores NULL intent —
    /// "settings without intent" — so the platform default keeps applying
    /// until the tenant explicitly toggles the module; freezing a default
    /// (or, worse, derived effective availability such as a transient
    /// denial) into the column would fabricate tenant intent that outlives
    /// the condition that produced it (audit M-5).'''),
    # tests: DDL + request fields + assertions
    ('                    enabled BOOLEAN NOT NULL, \\',
     '                    enabled BOOLEAN NULL, \\'),
    ('                default_enabled: false,\n', ''),
    ('                default_enabled: true,\n', ''),
])
"""H-5 part 2: crate-internal tri-state consumers."""

ROOT = '/home/user/RusTok/'


def edit(rel, pairs):
    p = ROOT + rel
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (rel, old[:80])
        s = s.replace(old, new)
        print(f'{rel}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


M = 'crates/modules/rustok-modules/src/'

# ------------------------------------------------------------ lifecycle_writer
edit(M + 'lifecycle_writer.rs', [
    ('''pub struct TenantModuleOverrideSnapshot {
    pub module_slug: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
}''',
     '''pub struct TenantModuleOverrideSnapshot {
    pub module_slug: String,
    /// Recorded tenant enablement intent, or `None` when the row carries
    /// settings without intent (the platform default applies).
    pub enabled: Option<bool>,
    pub settings: serde_json::Value,
}'''),
    # overrides(): only explicit intent rows are policy overrides.
    ('"SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = $1 LIMIT 1001"',
     '"SELECT module_slug, enabled FROM tenant_modules \\\n                 WHERE tenant_id = $1 AND enabled IS NOT NULL LIMIT 1001"'),
    ('"SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = ?1 LIMIT 1001"',
     '"SELECT module_slug, enabled FROM tenant_modules \\\n                 WHERE tenant_id = ?1 AND enabled IS NOT NULL LIMIT 1001"'),
    # persist_settings caller + result projection.
    ('''            let state = TenantModuleStateStore::persist_settings(
                &transaction,
                TenantModuleSettingsRequest {
                    tenant_id: command.tenant_id,
                    module_slug: definition.slug.clone(),
                    settings: command.settings.clone(),
                    // A first settings row records the platform default,
                    // never the module's current derived availability; an
                    // existing row keeps its recorded intent untouched.
                    default_enabled: definition.kind == ModuleDefinitionKind::Core
                        || self.default_enabled_modules.contains(&definition.slug),
                },
            )''',
     '''            // Effective enablement for the result projection: recorded
            // intent wins; without intent the platform default applies
            // (Core and default-enabled modules). A settings save itself
            // records no intent (see `persist_settings`).
            let platform_default_enabled = definition.kind == ModuleDefinitionKind::Core
                || self.default_enabled_modules.contains(&definition.slug);
            let state = TenantModuleStateStore::persist_settings(
                &transaction,
                TenantModuleSettingsRequest {
                    tenant_id: command.tenant_id,
                    module_slug: definition.slug.clone(),
                    settings: command.settings.clone(),
                },
            )'''),
    ('''            let result = ModuleLifecycleSettingsResult {
                module_slug: state.module_slug,
                enabled: state.enabled,''',
     '''            let result = ModuleLifecycleSettingsResult {
                module_slug: state.module_slug,
                enabled: state.enabled.unwrap_or(platform_default_enabled),'''),
    # static_tenant_module_views: resolve effective state at the owner.
    ('''        let overrides = self.tenant_override_snapshots(tenant_id, limit).await?;
        let lifecycle_revisions = self
            .static_lifecycle_snapshots(
                tenant_id,
                overrides.iter().map(|module| module.module_slug.clone()),
            )
            .await?;
        project_static_tenant_module_views(overrides, &lifecycle_revisions)''',
     '''        let overrides = self.tenant_override_snapshots(tenant_id, limit).await?;
        let lifecycle_revisions = self
            .static_lifecycle_snapshots(
                tenant_id,
                overrides.iter().map(|module| module.module_slug.clone()),
            )
            .await?;
        let catalog = self.definition_catalog()?;
        project_static_tenant_module_views(overrides, &lifecycle_revisions, &|module_slug| {
            catalog.get(module_slug).map(|definition| {
                definition.kind == ModuleDefinitionKind::Core
                    || self
                        .default_enabled_modules
                        .iter()
                        .any(|slug| slug == module_slug)
            })
        })'''),
    ('''fn project_static_tenant_module_views(
    overrides: Vec<TenantModuleOverrideSnapshot>,
    lifecycle_revisions: &BTreeMap<String, StaticTenantLifecycleSnapshot>,
) -> Result<Vec<StaticTenantModuleView>, ModuleLifecycleDbWriterError> {''',
     '''fn project_static_tenant_module_views(
    overrides: Vec<TenantModuleOverrideSnapshot>,
    lifecycle_revisions: &BTreeMap<String, StaticTenantLifecycleSnapshot>,
    platform_default_enabled: &dyn Fn(&str) -> Option<bool>,
) -> Result<Vec<StaticTenantModuleView>, ModuleLifecycleDbWriterError> {'''),
    ('''            Ok(StaticTenantModuleView {
                module_slug: module.module_slug,
                enabled: module.enabled,''',
     '''            // The browser projection presents effective enablement:
            // recorded intent wins; without intent the platform default
            // applies. An unknown slug has no resolvable default and must
            // fail loudly instead of presenting fabricated state.
            let enabled = match module.enabled {
                Some(enabled) => enabled,
                None => platform_default_enabled(&module.module_slug).ok_or_else(|| {
                    ModuleLifecycleDbWriterError::Configuration(format!(
                        "cannot resolve the platform default for unknown module `{}`",
                        module.module_slug
                    ))
                })?,
            };
            Ok(StaticTenantModuleView {
                module_slug: module.module_slug,
                enabled,'''),
    # project-fn tests.
    ('''        let views = project_static_tenant_module_views(
            vec![TenantModuleOverrideSnapshot {
                module_slug: "forum".to_string(),
                enabled: true,
                settings: serde_json::json!({ "visibility": "public" }),
            }],
            &revisions,
        )
        .expect("owner projection");''',
     '''        let views = project_static_tenant_module_views(
            vec![TenantModuleOverrideSnapshot {
                module_slug: "forum".to_string(),
                enabled: Some(true),
                settings: serde_json::json!({ "visibility": "public" }),
            }],
            &revisions,
            // Explicit intent never consults the platform default.
            &|_| None,
        )
        .expect("owner projection");'''),
    ('''        let error = project_static_tenant_module_views(
            vec![TenantModuleOverrideSnapshot {
                module_slug: "forum".to_string(),
                enabled: true,
                settings: serde_json::json!({}),
            }],
            &BTreeMap::new(),
        )
        .expect_err("missing aggregate revision must not become a synthetic response");''',
     '''        let error = project_static_tenant_module_views(
            vec![TenantModuleOverrideSnapshot {
                module_slug: "forum".to_string(),
                enabled: Some(true),
                settings: serde_json::json!({}),
            }],
            &BTreeMap::new(),
            &|_| None,
        )
        .expect_err("missing aggregate revision must not become a synthetic response");'''),
])

# ------------------------------------------------------- policy_revision_consumer
edit(M + 'policy_revision_consumer.rs', [
    ('"SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = $1"',
     '"SELECT module_slug, enabled FROM tenant_modules \\\n             WHERE tenant_id = $1 AND enabled IS NOT NULL"'),
    ('"SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = ?1"',
     '"SELECT module_slug, enabled FROM tenant_modules \\\n             WHERE tenant_id = ?1 AND enabled IS NOT NULL"'),
])

# --------------------------------------------------------------------- recovery
edit(M + 'recovery.rs', [
    ('''    db.query_one_raw(Statement::from_sql_and_values(
        backend.db_backend(),
        sql,
        vec![backend.uuid_value(tenant_id), module_slug.into()],
    ))
    .await
    .map_err(store_database_error)?
    .map(|row| row.try_get("", "enabled").map_err(store_database_error))
    .transpose()
}''',
     '''    let intent = db
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            vec![backend.uuid_value(tenant_id), module_slug.into()],
        ))
        .await
        .map_err(store_database_error)?
        .map(|row| {
            row.try_get::<Option<bool>>("", "enabled")
                .map_err(store_database_error)
        })
        .transpose()?;
    // No row and a row without recorded intent both mean "no override".
    Ok(intent.flatten())
}'''),
    ('''            SupportedBackend::Postgres => {
                "DELETE FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2"
            }
            SupportedBackend::Sqlite => {
                "DELETE FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2"
            }''',
     '''            // Clearing the override returns the tenant to the platform
            // default; the row and its settings survive (tri-state intent).
            SupportedBackend::Postgres => {
                "UPDATE tenant_modules SET enabled = NULL, updated_at = CURRENT_TIMESTAMP \\
                 WHERE tenant_id = $1 AND module_slug = $2"
            }
            SupportedBackend::Sqlite => {
                "UPDATE tenant_modules SET enabled = NULL, updated_at = CURRENT_TIMESTAMP \\
                 WHERE tenant_id = ?1 AND module_slug = ?2"
            }'''),
])

# --------------------------------------------------------- static_settings_reader
edit(M + 'static_settings_reader.rs', [
    ('        let enabled: bool = row.try_get("", "enabled").map_err(|_| {',
     '        let enabled: Option<bool> = row.try_get("", "enabled").map_err(|_| {'),
    ('                enabled BOOLEAN NOT NULL,',
     '                enabled BOOLEAN NULL,'),
    ('        assert!(!snapshot.enabled);',
     '        assert_eq!(snapshot.enabled, Some(false));'),
    ('        assert!(foreign.enabled);',
     '        assert_eq!(foreign.enabled, Some(true));'),
])

# --------------------------------------------------------------------- executor
edit(M + 'executor.rs', [
    ('                    enabled BOOLEAN NOT NULL, \\',
     '                    enabled BOOLEAN NULL, \\'),
])
"""H-5 part 3: API surface, tenant entity/DTO, snapshot consumers."""

import re

ROOT = '/home/user/RusTok/'


def edit(rel, pairs):
    p = ROOT + rel
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (rel, old[:80])
        s = s.replace(old, new)
        print(f'{rel}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


# ------------------------------------------------------------------ rustok-api
edit('crates/libs/rustok-api/src/module_lifecycle.rs', [
    ('''pub struct StaticModuleSettingsSnapshot {
    pub enabled: bool,
    pub settings: Value,
}''',
     '''pub struct StaticModuleSettingsSnapshot {
    /// Recorded tenant enablement intent, or `None` when the tenant has
    /// recorded no intent and the module inherits the platform default
    /// (tri-state `tenant_modules.enabled`).
    pub enabled: Option<bool>,
    pub settings: Value,
}'''),
    ('''    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,''',
     '''    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    /// Effective enablement: the recorded tenant intent, or the platform
    /// default when no intent is recorded. Resolved by the control-plane
    /// owner; transports must not re-derive it.
    pub enabled: bool,'''),
])

edit('crates/libs/rustok-api/src/runtime.rs', [
    # settings reads: a NULL-intent row is not disabled; only an explicit
    # false hides its settings.
    ('"SELECT CAST(settings AS TEXT) AS settings_json FROM tenant_modules WHERE (tenant_id = ?1 OR tenant_id = ?2) AND module_slug = ?3 AND enabled = 1 LIMIT 1"',
     '"SELECT CAST(settings AS TEXT) AS settings_json FROM tenant_modules WHERE (tenant_id = ?1 OR tenant_id = ?2) AND module_slug = ?3 AND enabled IS NOT 0 LIMIT 1"'),
    ('"SELECT settings::text AS settings_json FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 AND enabled = true LIMIT 1 FOR SHARE"',
     '"SELECT settings::text AS settings_json FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 AND enabled IS NOT FALSE LIMIT 1 FOR SHARE"'),
    ('"SELECT CAST(settings AS CHAR) AS settings_json FROM tenant_modules WHERE tenant_id = ? AND module_slug = ? AND enabled = true LIMIT 1 FOR SHARE"',
     '"SELECT CAST(settings AS CHAR) AS settings_json FROM tenant_modules WHERE tenant_id = ? AND module_slug = ? AND enabled IS NOT FALSE LIMIT 1 FOR SHARE"'),
    ('"SELECT settings::text AS settings_json FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 AND enabled = true LIMIT 1"',
     '"SELECT settings::text AS settings_json FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 AND enabled IS NOT FALSE LIMIT 1"'),
    ('"SELECT CAST(settings AS CHAR) AS settings_json FROM tenant_modules WHERE tenant_id = ? AND module_slug = ? AND enabled = true LIMIT 1"',
     '"SELECT CAST(settings AS CHAR) AS settings_json FROM tenant_modules WHERE tenant_id = ? AND module_slug = ? AND enabled IS NOT FALSE LIMIT 1"'),
    ('''/// Returns the settings snapshot for one exact enabled tenant module while
/// participating in a caller-owned transaction.''',
     '''/// Returns the settings snapshot for one exact tenant module that is not
/// explicitly disabled, while participating in a caller-owned transaction.
/// A row without recorded enablement intent (NULL, "settings without
/// intent") exposes its settings; only an explicit disable hides them.'''),
    ('''/// Returns the settings snapshot for one exact enabled tenant module.
///
/// This is the read-only counterpart to [`is_tenant_module_enabled`]. Internal
/// GraphQL and native server-function adapters can consume the same persisted
/// tenant-module control-plane snapshot without importing tenant persistence
/// entities or inventing a parallel settings store. The caller must supply the
/// tenant id from a trusted request context; this helper only applies the exact
/// tenant/module/enabled-row lookup. Disabled or missing rows return `None`.''',
     '''/// Returns the settings snapshot for one exact tenant module that is not
/// explicitly disabled.
///
/// This is the read-only counterpart to [`is_tenant_module_enabled`]. Internal
/// GraphQL and native server-function adapters can consume the same persisted
/// tenant-module control-plane snapshot without importing tenant persistence
/// entities or inventing a parallel settings store. The caller must supply the
/// tenant id from a trusted request context; this helper only applies the exact
/// tenant/module/not-disabled-row lookup. Explicitly disabled or missing rows
/// return `None`; a row without recorded intent (NULL) exposes its settings.'''),
    ('''/// Returns whether an optional module is enabled for the tenant snapshot that
/// owns the current request.''',
     '''/// Returns whether an optional module is explicitly enabled for the tenant
/// snapshot that owns the current request. A row without recorded enablement
/// intent (NULL) does not count as enabled: optional modules are off until
/// the tenant explicitly enables them.'''),
    ('    enabled INTEGER NOT NULL,',
     '    enabled INTEGER NULL,'),
    # NULL-intent coverage in the tx settings test.
    ('''             ('{tenant_id}', 'forum-disabled', 0, '{{"use_reactions":true}}'),''',
     '''             ('{tenant_id}', 'forum-disabled', 0, '{{"use_reactions":true}}'),
             ('{tenant_id}', 'forum-inherited', NULL, '{{"use_reactions":true}}'),'''),
    ('''        assert!(
            tenant_module_settings_in_tx(&txn, tenant_id, "forum-disabled")
                .await
                .expect("disabled settings lookup should succeed")
                .is_none()
        );''',
     '''        assert!(
            tenant_module_settings_in_tx(&txn, tenant_id, "forum-disabled")
                .await
                .expect("disabled settings lookup should succeed")
                .is_none()
        );
        // A row without recorded intent still exposes its settings.
        assert_eq!(
            tenant_module_settings_in_tx(&txn, tenant_id, "forum-inherited")
                .await
                .expect("inherited settings lookup should succeed")
                .expect("row without intent should be visible")["use_reactions"],
            true
        );
        // ... but does not count as explicitly enabled.
        assert!(
            !is_tenant_module_enabled(&db, tenant_id, "forum-inherited")
                .await
                .expect("inherited enablement lookup should succeed")
        );'''),
])

# ---------------------------------------------------------------- rustok-tenant
edit('crates/modules/rustok-tenant/src/entities/tenant_module.rs', [
    ('    pub enabled: bool,',
     '''    /// Tri-state enablement intent: `None` records "no tenant intent"
    /// (the module inherits the platform default); only an explicit
    /// toggle stores `Some(true)`/`Some(false)`.
    pub enabled: Option<bool>,'''),
])
edit('crates/modules/rustok-tenant/src/dto/mod.rs', [
    ('''pub struct TenantModuleResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub enabled: bool,''',
     '''pub struct TenantModuleResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    /// Recorded tenant enablement intent, or `None` when the platform
    /// default applies (tri-state `tenant_modules.enabled`).
    pub enabled: Option<bool>,'''),
])
edit('crates/modules/rustok-tenant/admin/src/transport/native_server_adapter.rs', [
    ('''                    source: if is_core {
                        "core-default".to_string()
                    } else if explicit.is_some() {''',
     '''                    source: if is_core {
                        "core-default".to_string()
                    } else if matches!(explicit, Some(Some(_))) {'''),
])

# ------------------------------------------------------- snapshot constructions
files = [
    'crates/modules/rustok-blog/tests/integration.rs',
    'crates/modules/rustok-content-orchestration/src/tests/helpers.rs',
    'crates/modules/rustok-forum/src/reaction_subject.rs',
    'crates/modules/rustok-forum/src/seo_audience_targets.rs',
    'crates/modules/rustok-forum/src/seo_audience_targets_sqlite_tests.rs',
    'crates/modules/rustok-forum/src/services/engagement_mode.rs',
    'crates/modules/rustok-forum/src/services/mention_relation_tests.rs',
    'crates/modules/rustok-forum/tests/anonymous_discovery_gate_sqlite.rs',
    'crates/modules/rustok-forum/tests/moderation_report_sqlite.rs',
    'crates/modules/rustok-forum/tests/notification_richer_visibility_sqlite.rs',
    'crates/modules/rustok-forum/tests/posting_limits_sqlite.rs',
    'crates/modules/rustok-forum/tests/support/forum_public_settings.rs',
    'crates/modules/rustok-forum/tests/votes.rs',
]
pat = re.compile(r'(StaticModuleSettingsSnapshot \{\s*\n\s*enabled: )([^,\n]+)(,)')
for rel in files:
    p = ROOT + rel
    s = open(p).read()
    def wrap(m):
        expr = m.group(2).strip()
        if expr == 'm.enabled' or expr.startswith('Some('):
            return m.group(0)
        return f'{m.group(1)}Some({expr}){m.group(3)}'
    s2, n = pat.subn(wrap, s)
    print(f'{rel}: snapshot constructions touched={n}')
    open(p, 'w').write(s2)

# ------------------------------------------------------------- snapshot reads
edit('crates/modules/rustok-forum/src/reaction_subject.rs', [
    ('        if !snapshot.enabled {',
     '        if snapshot.enabled != Some(true) {'),
])
edit('crates/modules/rustok-forum/src/services/engagement_mode.rs', [
    ('        Some(snapshot) if snapshot.enabled => {',
     '        Some(snapshot) if snapshot.enabled == Some(true) => {'),
    ('.is_some_and(|snapshot| snapshot.enabled)',
     '.is_some_and(|snapshot| snapshot.enabled == Some(true))'),
    ('.is_some_and(|snapshot| snapshot.enabled);',
     '.is_some_and(|snapshot| snapshot.enabled == Some(true));'),
])

# ---------------------------------------------------------- entity ActiveModels
set_files = [
    'crates/modules/rustok-forum/tests/posting_limits_sqlite.rs',
    'crates/modules/rustok-forum/tests/votes.rs',
    'crates/modules/rustok-seo/src/graphql/mod.rs',
    'crates/modules/rustok-seo/src/services/routing.rs',
    'crates/modules/rustok-seo/src/services/sitemaps/tests.rs',
    'crates/modules/rustok-seo/tests/redirect_transaction.rs',
    'crates/modules/rustok-seo/tests/sitemap_transaction.rs',
]
for rel in set_files:
    p = ROOT + rel
    s = open(p).read()
    n = s.count('enabled: Set(true),')
    s = s.replace('enabled: Set(true),', 'enabled: Set(Some(true)),')
    print(f'{rel}: Set(true) wrapped={n}')
    open(p, 'w').write(s)
edit('crates/modules/rustok-seo/admin/src/transport/native_server_adapter.rs', [
    ('            enabled: Set(enabled),',
     '            enabled: Set(Some(enabled)),'),
])

# ------------------------------------------------------------------ server test
edit('apps/server/src/services/module_lifecycle.rs', [
    ('''        assert_eq!(stored.settings["workspaceName"], serde_json::json!("Acme"));
        assert!(stored.enabled);''',
     '''        assert_eq!(stored.settings["workspaceName"], serde_json::json!("Acme"));
        // A settings save records no enablement intent (tri-state): the
        // Core module stays available through the platform default, not
        // through a frozen flag.
        assert_eq!(stored.enabled, None);'''),
])
"""H-5 part 3b: remainder after part 3 assertion stop."""

ROOT = '/home/user/RusTok/'


def edit(rel, pairs):
    p = ROOT + rel
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (rel, old[:80])
        s = s.replace(old, new)
        print(f'{rel}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


edit('crates/modules/rustok-forum/src/services/engagement_mode.rs', [
    ('        Some(snapshot) if snapshot.enabled => {',
     '        Some(snapshot) if snapshot.enabled == Some(true) => {'),
    ('.is_some_and(|snapshot| snapshot.enabled)',
     '.is_some_and(|snapshot| snapshot.enabled == Some(true))'),
])

set_files = [
    'crates/modules/rustok-forum/tests/posting_limits_sqlite.rs',
    'crates/modules/rustok-forum/tests/votes.rs',
    'crates/modules/rustok-seo/src/graphql/mod.rs',
    'crates/modules/rustok-seo/src/services/routing.rs',
    'crates/modules/rustok-seo/src/services/sitemaps/tests.rs',
    'crates/modules/rustok-seo/tests/redirect_transaction.rs',
    'crates/modules/rustok-seo/tests/sitemap_transaction.rs',
]
for rel in set_files:
    p = ROOT + rel
    s = open(p).read()
    n = s.count('enabled: Set(true),')
    assert n >= 1, rel
    s = s.replace('enabled: Set(true),', 'enabled: Set(Some(true)),')
    print(f'{rel}: Set(true) wrapped={n}')
    open(p, 'w').write(s)

edit('crates/modules/rustok-seo/admin/src/transport/native_server_adapter.rs', [
    ('            enabled: Set(enabled),',
     '            enabled: Set(Some(enabled)),'),
])

edit('apps/server/src/services/module_lifecycle.rs', [
    ('''        assert_eq!(stored.settings["workspaceName"], serde_json::json!("Acme"));
        assert!(stored.enabled);''',
     '''        assert_eq!(stored.settings["workspaceName"], serde_json::json!("Acme"));
        // A settings save records no enablement intent (tri-state): the
        // Core module stays available through the platform default, not
        // through a frozen flag.
        assert_eq!(stored.enabled, None);'''),
])
"""H-5 part 3c: artifact settings record + server toggle/recovery mapping."""

ROOT = '/home/user/RusTok/'


def edit(rel, pairs):
    p = ROOT + rel
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (rel, old[:80])
        s = s.replace(old, new)
        print(f'{rel}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


edit('crates/modules/rustok-modules/src/artifact_settings.rs', [
    ('''    Ok(TenantModuleSettingsRecord {
        id: installation.settings_instance_id,
        module_slug: module_slug.to_string(),
        enabled: true,
        settings,
    })''',
     '''    Ok(TenantModuleSettingsRecord {
        id: installation.settings_instance_id,
        module_slug: module_slug.to_string(),
        // Artifact enablement is owned by the artifact tenant lifecycle,
        // not by `tenant_modules`; this path records no tenant_modules
        // intent (previously this fabricated a constant `true`).
        enabled: None,
        settings,
    })'''),
])

edit('apps/server/src/services/module_lifecycle.rs', [
    ('''        let state = result.state.ok_or_else(|| {
            ToggleModuleError::Policy(
                "explicit module toggle did not persist a tenant override row".to_string(),
            )
        })?;
        Ok(ModuleLifecycleStateSnapshot {
            module_slug: module_slug.to_string(),
            enabled: state.enabled,''',
     '''        let state = result.state.ok_or_else(|| {
            ToggleModuleError::Policy(
                "explicit module toggle did not persist a tenant override row".to_string(),
            )
        })?;
        let enabled = state.enabled.ok_or_else(|| {
            ToggleModuleError::Policy(
                "explicit module toggle did not record tenant enablement intent".to_string(),
            )
        })?;
        Ok(ModuleLifecycleStateSnapshot {
            module_slug: module_slug.to_string(),
            enabled,'''),
    ('''        let (enabled, settings) = match result.state {
            Some(state) => (state.enabled, result.settings),
            None => (policy.contains(&module_slug), serde_json::json!({})),
        };''',
     '''        let (enabled, settings) = match result.state {
            // A restored "no recorded intent" state falls back to the
            // effective policy, exactly like a missing row.
            Some(state) => (
                state
                    .enabled
                    .unwrap_or_else(|| policy.contains(&module_slug)),
                result.settings,
            ),
            None => (policy.contains(&module_slug), serde_json::json!({})),
        };'''),
])
