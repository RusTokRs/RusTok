"""H-3: convert moved migrations to the crate SupportedBackend contract
(H-1 canon). Applied incrementally per file with assertions; retained as
evidence of the mechanical steps.
"""

BASE = '/home/user/RusTok/crates/modules/rustok-modules/src/migrations/'


def edit(name, pairs):
    p = BASE + name
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (name, old[:80])
        s = s.replace(old, new)
        print(f'{name}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


edit('m20260408_000067_expand_registry_publish_request_governance_states.rs', [
    ('use sea_orm_migration::sea_orm::DatabaseBackend;',
     'use crate::backend::SupportedBackend;'),
    ('        let sqlite = manager.get_database_backend() == DatabaseBackend::Sqlite;',
     '        let sqlite =\n            SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Sqlite;'),
])

edit('m20260408_000068_expand_registry_validation_stage_runner_leases.rs', [
    ('use sea_orm_migration::sea_orm::DatabaseBackend;',
     'use crate::backend::SupportedBackend;'),
    ('        if manager.get_database_backend() == DatabaseBackend::Sqlite {',
     '        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Sqlite {'),
])
"""H-3: convert moved migrations to the crate SupportedBackend contract
(H-1 canon). Applied incrementally per file with assertions; retained as
evidence of the mechanical steps.
"""

BASE = '/home/user/RusTok/crates/modules/rustok-modules/src/migrations/'


def edit(name, pairs):
    p = BASE + name
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (name, old[:80])
        s = s.replace(old, new)
        print(f'{name}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


# --- m20260412_000069 reset_registry_identity_and_artifacts ---
edit('m20260412_000069_reset_registry_identity_and_artifacts.rs', [
    ('use sea_orm::{ConnectionTrait, DbBackend, Statement};',
     'use sea_orm::{ConnectionTrait, Statement};'),
    ('use uuid::Uuid;',
     'use uuid::Uuid;\n\nuse crate::backend::SupportedBackend;'),
    ('    if manager.get_database_backend() == DbBackend::Sqlite {',
     '    if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Sqlite {'),
    ('    let backend = db.get_database_backend();',
     '    let backend = SupportedBackend::new(db.get_database_backend())?;'),
    ('            backend,',
     '            backend.db_backend(),'),
    ('    db.query_all_raw(Statement::from_string(\n        db.get_database_backend(),',
     '    let backend = SupportedBackend::new(db.get_database_backend())?;\n'
     '    db.query_all_raw(Statement::from_string(\n        backend.db_backend(),'),
    ('Statement::from_sql_and_values(backend, sql, values)',
     'Statement::from_sql_and_values(backend.db_backend(), sql, values)'),
    ('''fn placeholder_sql(backend: DbBackend, template: &str) -> String {
    let placeholders = match backend {
        DbBackend::Sqlite => ["?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8"],
        _ => ["$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8"],
    };
    let mut sql = template.to_string();
    for (index, placeholder) in placeholders.into_iter().enumerate() {
        sql = sql.replace(&format!("{{json{}}}", index + 1), placeholder);
    }
    let pk_placeholder = placeholders[values_index_from_template(template) - 1];
    sql.replace("{pk}", pk_placeholder)
}''',
     '''fn placeholder_sql(backend: SupportedBackend, template: &str) -> String {
    let mut sql = template.to_string();
    for index in 1..=8 {
        sql = sql.replace(&format!("{{json{index}}}"), &backend.placeholder(index));
    }
    let pk_placeholder = backend.placeholder(values_index_from_template(template));
    sql.replace("{pk}", &pk_placeholder)
}'''),
])

# --- m20260412_000070 split_registry_localized_metadata ---
edit('m20260412_000070_split_registry_localized_metadata.rs', [
    ('use sea_orm::{ConnectionTrait, DbBackend, Statement};\nuse sea_orm_migration::prelude::*;',
     'use sea_orm::{ConnectionTrait, Statement};\nuse sea_orm_migration::prelude::*;\n\nuse crate::backend::SupportedBackend;'),
    ('    let backend = db.get_database_backend();',
     '    let backend = SupportedBackend::new(db.get_database_backend())?;'),
    ('            backend,',
     '            backend.db_backend(),'),
    ('Statement::from_sql_and_values(backend, sql, values)',
     'Statement::from_sql_and_values(backend.db_backend(), sql, values)'),
    ('''fn placeholder_sql(backend: DbBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = match backend {
            DbBackend::Postgres => format!("${}", index + 1),
            DbBackend::MySql => "?".to_string(),
            DbBackend::Sqlite => format!("?{}", index + 1),
            _ => unreachable!("unsupported SeaORM database backend"),
        };
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}''',
     '''fn placeholder_sql(backend: SupportedBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = backend.placeholder(index + 1);
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}'''),
])

# --- m20260419_000071 normalize_registry_governance_event_payloads ---
edit('m20260419_000071_normalize_registry_governance_event_payloads.rs', [
    ('use sea_orm::{ConnectionTrait, DbBackend, Statement};',
     'use sea_orm::{ConnectionTrait, Statement};'),
    ('use uuid::Uuid;',
     'use uuid::Uuid;\n\nuse crate::backend::SupportedBackend;'),
    ('    let backend = db.get_database_backend();',
     '    let backend = SupportedBackend::new(db.get_database_backend())?;'),
    ('            backend,',
     '            backend.db_backend(),'),
    ('Statement::from_sql_and_values(backend, sql, values)',
     'Statement::from_sql_and_values(backend.db_backend(), sql, values)'),
    ('''fn placeholder_sql(backend: DbBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = match backend {
            DbBackend::Sqlite => format!("?{}", index + 1),
            _ => format!("${}", index + 1),
        };
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}''',
     '''fn placeholder_sql(backend: SupportedBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = backend.placeholder(index + 1);
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}'''),
])
"""H-3: convert moved migrations to the crate SupportedBackend contract
(H-1 canon). Applied incrementally per file with assertions; retained as
evidence of the mechanical steps.
"""

BASE = '/home/user/RusTok/crates/modules/rustok-modules/src/migrations/'


def edit(name, pairs):
    p = BASE + name
    s = open(p).read()
    for old, new in pairs:
        count = s.count(old)
        assert count >= 1, (name, old[:80])
        s = s.replace(old, new)
        print(f'{name}: replaced {count}x  {old[:60]!r}')
    open(p, 'w').write(s)


# --- m20260717_000073 add_registry_artifact_origin_and_external_staging ---
edit('m20260717_000073_add_registry_artifact_origin_and_external_staging.rs', [
    ('use sea_orm::{ConnectionTrait, DbBackend, Statement};\nuse sea_orm_migration::prelude::*;',
     'use sea_orm::{ConnectionTrait, Statement};\nuse sea_orm_migration::prelude::*;\n\nuse crate::backend::SupportedBackend;'),
    ('        let statements: &[&str] = match manager.get_database_backend() {\n            DbBackend::Postgres => &[',
     '        let backend = SupportedBackend::new(manager.get_database_backend())?;\n'
     '        let statements: &[&str] = match backend {\n            SupportedBackend::Postgres => &['),
    ('            DbBackend::Sqlite => &[',
     '            SupportedBackend::Sqlite => &['),
    ('''            backend => {
                return Err(DbErr::Migration(format!(
                    "registry external artifact staging does not support database backend {backend:?}"
                )));
            }
''', ''),
    ('                    manager.get_database_backend(),',
     '                    backend.db_backend(),'),
    ('        let backend = manager.get_database_backend();',
     '        let backend = SupportedBackend::new(manager.get_database_backend())?;'),
    ('.execute_raw(Statement::from_string(backend, statement.to_string()))',
     '.execute_raw(Statement::from_string(\n                    backend.db_backend(),\n                    statement.to_string(),\n                ))'),
])

# --- m20260718_000074 add_registry_publication_idempotency ---
edit('m20260718_000074_add_registry_publication_idempotency.rs', [
    ('use sea_orm::{ConnectionTrait, DbBackend, Statement};\nuse sea_orm_migration::prelude::*;',
     'use sea_orm::{ConnectionTrait, Statement};\nuse sea_orm_migration::prelude::*;\n\nuse crate::backend::SupportedBackend;'),
    ('        let statement = match manager.get_database_backend() {\n            DbBackend::Postgres => {',
     '        let backend = SupportedBackend::new(manager.get_database_backend())?;\n'
     '        let statement = match backend {\n            SupportedBackend::Postgres => {'),
    ('            DbBackend::Sqlite => {',
     '            SupportedBackend::Sqlite => {'),
    ('''            backend => {
                return Err(DbErr::Migration(format!(
                    "registry publication idempotency migration does not support database backend {backend:?}"
                )));
            }
''', ''),
    ('                    manager.get_database_backend(),',
     '                    backend.db_backend(),'),
])
