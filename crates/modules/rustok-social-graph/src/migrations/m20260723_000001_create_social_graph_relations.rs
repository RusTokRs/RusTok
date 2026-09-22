use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => POSTGRES_UP,
            DatabaseBackend::Sqlite => SQLITE_UP,
            backend => {
                return Err(DbErr::Custom(format!(
                    "social graph persistence does not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute_unprepared(sql)
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS social_graph_relations;")
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity
    ON users (tenant_id, id);

CREATE TABLE IF NOT EXISTS social_graph_relations (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    source_user_id UUID NOT NULL,
    target_user_id UUID NOT NULL,
    relation_kind VARCHAR(16) NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_social_graph_source_user FOREIGN KEY (tenant_id, source_user_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_social_graph_target_user FOREIGN KEY (tenant_id, target_user_id)
        REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT ck_social_graph_distinct_users CHECK (source_user_id <> target_user_id),
    CONSTRAINT ck_social_graph_relation_kind CHECK (relation_kind IN ('block', 'mute')),
    CONSTRAINT ck_social_graph_revision CHECK (revision > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_social_graph_relation_identity
    ON social_graph_relations (tenant_id, source_user_id, target_user_id, relation_kind);
CREATE INDEX IF NOT EXISTS idx_social_graph_relation_active_source
    ON social_graph_relations (tenant_id, source_user_id, relation_kind, active, target_user_id);
CREATE INDEX IF NOT EXISTS idx_social_graph_relation_active_target
    ON social_graph_relations (tenant_id, target_user_id, relation_kind, active, source_user_id);
"#;

const SQLITE_UP: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity
    ON users (tenant_id, id);

CREATE TABLE IF NOT EXISTS social_graph_relations (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    source_user_id TEXT NOT NULL,
    target_user_id TEXT NOT NULL,
    relation_kind TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, source_user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, target_user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE,
    CHECK (source_user_id <> target_user_id),
    CHECK (relation_kind IN ('block', 'mute')),
    CHECK (active IN (0, 1)),
    CHECK (revision > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_social_graph_relation_identity
    ON social_graph_relations (tenant_id, source_user_id, target_user_id, relation_kind);
CREATE INDEX IF NOT EXISTS idx_social_graph_relation_active_source
    ON social_graph_relations (tenant_id, source_user_id, relation_kind, active, target_user_id);
CREATE INDEX IF NOT EXISTS idx_social_graph_relation_active_target
    ON social_graph_relations (tenant_id, target_user_id, relation_kind, active, source_user_id);
"#;
