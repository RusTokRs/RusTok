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
                    "social graph follow migration does not support database backend {backend:?}"
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
        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => POSTGRES_DOWN,
            DatabaseBackend::Sqlite => SQLITE_DOWN,
            backend => {
                return Err(DbErr::Custom(format!(
                    "social graph follow migration does not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute_unprepared(sql)
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
ALTER TABLE social_graph_relations
    DROP CONSTRAINT IF EXISTS ck_social_graph_relation_kind;
ALTER TABLE social_graph_relations
    ADD CONSTRAINT ck_social_graph_relation_kind
    CHECK (relation_kind IN ('block', 'mute', 'follow'));
"#;

const POSTGRES_DOWN: &str = r#"
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM social_graph_relations
        WHERE relation_kind = 'follow'
        LIMIT 1
    ) THEN
        RAISE EXCEPTION 'cannot remove social graph follow relation kind while follow rows exist';
    END IF;
END
$$;
ALTER TABLE social_graph_relations
    DROP CONSTRAINT IF EXISTS ck_social_graph_relation_kind;
ALTER TABLE social_graph_relations
    ADD CONSTRAINT ck_social_graph_relation_kind
    CHECK (relation_kind IN ('block', 'mute'));
"#;

const SQLITE_UP: &str = r#"
ALTER TABLE social_graph_relations RENAME TO social_graph_relations_before_follow;

CREATE TABLE social_graph_relations (
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
    CHECK (relation_kind IN ('block', 'mute', 'follow')),
    CHECK (active IN (0, 1)),
    CHECK (revision > 0)
);

INSERT INTO social_graph_relations (
    id,
    tenant_id,
    source_user_id,
    target_user_id,
    relation_kind,
    active,
    revision,
    created_at,
    updated_at
)
SELECT
    id,
    tenant_id,
    source_user_id,
    target_user_id,
    relation_kind,
    active,
    revision,
    created_at,
    updated_at
FROM social_graph_relations_before_follow;

DROP TABLE social_graph_relations_before_follow;

CREATE UNIQUE INDEX ux_social_graph_relation_identity
    ON social_graph_relations (tenant_id, source_user_id, target_user_id, relation_kind);
CREATE INDEX idx_social_graph_relation_active_source
    ON social_graph_relations (tenant_id, source_user_id, relation_kind, active, target_user_id);
CREATE INDEX idx_social_graph_relation_active_target
    ON social_graph_relations (tenant_id, target_user_id, relation_kind, active, source_user_id);
"#;

const SQLITE_DOWN: &str = r#"
CREATE TEMP TABLE social_graph_follow_downgrade_guard (
    follow_count INTEGER NOT NULL CHECK (follow_count = 0)
);
INSERT INTO social_graph_follow_downgrade_guard (follow_count)
SELECT COUNT(*) FROM social_graph_relations WHERE relation_kind = 'follow';
DROP TABLE social_graph_follow_downgrade_guard;

ALTER TABLE social_graph_relations RENAME TO social_graph_relations_with_follow;

CREATE TABLE social_graph_relations (
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

INSERT INTO social_graph_relations (
    id,
    tenant_id,
    source_user_id,
    target_user_id,
    relation_kind,
    active,
    revision,
    created_at,
    updated_at
)
SELECT
    id,
    tenant_id,
    source_user_id,
    target_user_id,
    relation_kind,
    active,
    revision,
    created_at,
    updated_at
FROM social_graph_relations_with_follow;

DROP TABLE social_graph_relations_with_follow;

CREATE UNIQUE INDEX ux_social_graph_relation_identity
    ON social_graph_relations (tenant_id, source_user_id, target_user_id, relation_kind);
CREATE INDEX idx_social_graph_relation_active_source
    ON social_graph_relations (tenant_id, source_user_id, relation_kind, active, target_user_id);
CREATE INDEX idx_social_graph_relation_active_target
    ON social_graph_relations (tenant_id, target_user_id, relation_kind, active, source_user_id);
"#;
