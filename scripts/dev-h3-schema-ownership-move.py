"""H-3 (M-12) schema-ownership move: strip moved module-domain migrations
from crates/utils/rustok-migrations after `git mv` relocated them into
crates/modules/rustok-modules/src/migrations with fresh ordinals
(000060..=000074). Retained as evidence of the mechanical steps.
"""

p = '/home/user/RusTok/crates/utils/rustok-migrations/src/lib.rs'
s = open(p).read()

MOVED = [
    'm20250101_000003_create_tenant_modules',
    'm20260403_000002_create_registry_publish_tables',
    'm20260403_000005_create_registry_module_owners',
    'm20260403_000007_create_registry_governance_events',
    'm20260403_000008_add_registry_publish_request_publisher_identity',
    'm20260404_000001_create_registry_validation_jobs',
    'm20260404_000002_create_registry_validation_stages',
    'm20260408_000001_expand_registry_publish_request_governance_states',
    'm20260408_000002_expand_registry_validation_stage_runner_leases',
    'm20260412_000001_reset_registry_identity_and_artifacts',
    'm20260412_000002_split_registry_localized_metadata',
    'm20260419_000001_normalize_registry_governance_event_payloads',
    'm20260717_000001_create_registry_publication_evidence',
    'm20260717_000003_add_registry_artifact_origin_and_external_staging',
    'm20260718_000002_add_registry_publication_idempotency',
]

# 1. InstallerOwnerMigrator first: its publish-tables Box site precedes the
# Migrator vec site in the file.
old = '''        let static_owner = rustok_core::MigrationSource::migrations(&rustok_modules::ModulesModule)
            .into_iter()
            .find(|migration| migration.name() == "m20260722_000034_static_promotions")
            .expect("static-distribution owner migration must exist");
        vec![
            Box::new(m20250101_000001_create_tenants::Migration),
            Box::new(m20260403_000002_create_registry_publish_tables::Migration),
            Box::new(m20260426_000001_create_install_sessions::Migration),
            static_owner,
        ]'''
new = '''        let mut modules_owner = rustok_core::MigrationSource::migrations(
            &rustok_modules::ModulesModule,
        )
        .into_iter();
        let publish_tables = modules_owner
            .find(|migration| {
                migration.name() == "m20260403_000061_create_registry_publish_tables"
            })
            .expect("registry publish tables migration must exist");
        let static_owner = modules_owner
            .find(|migration| migration.name() == "m20260722_000034_static_promotions")
            .expect("static-distribution owner migration must exist");
        vec![
            Box::new(m20250101_000001_create_tenants::Migration),
            publish_tables,
            Box::new(m20260426_000001_create_install_sessions::Migration),
            static_owner,
        ]'''
assert old in s
s = s.replace(old, new, 1)

# 2. Strip mod decls + Migrator-vec entries for moved migrations.
for name in MOVED:
    decl = f'mod {name};\n'
    assert decl in s, name
    s = s.replace(decl, '', 1)
    box = f'            Box::new({name}::Migration),\n'
    if box in s:
        s = s.replace(box, '', 1)
    else:
        push = f'''        all.push(Box::new(
            {name}::Migration,
        ));
'''
        assert push in s, name
        s = s.replace(push, '', 1)

# 3. Test: substitute an unrelated surviving migration for tenant_modules.
old_test = '''            Box::new(super::m20250101_000003_create_tenant_modules::Migration),'''
new_test = '''            Box::new(super::m20250101_000004_create_sessions::Migration),'''
assert old_test in s
s = s.replace(old_test, new_test, 1)
old_name = '''                "m20250101_000003_create_tenant_modules",'''
new_name = '''                "m20250101_000004_create_sessions",'''
assert old_name in s
s = s.replace(old_name, new_name, 1)

for name in MOVED:
    assert name not in s, name

open(p, 'w').write(s)
print('rustok-migrations lib.rs cleaned')
