use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortError, RichTextDocument, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot,
};
use rustok_comments::CommentsModule;
use rustok_content::ContentModule;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::ForumModule;
use rustok_outbox::SysEventsMigration;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

pub(crate) fn richtext(text: &str) -> RichTextDocument {
    serde_json::from_value(serde_json::json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": text}]
        }]
    }))
    .expect("test richtext")
}

struct BlogSettingsReader {
    comments_mode: &'static str,
}

#[async_trait]
impl StaticModuleSettingsReader for BlogSettingsReader {
    async fn settings(
        &self,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(
            (module_slug == "blog").then(|| StaticModuleSettingsSnapshot {
                enabled: true,
                settings: serde_json::json!({ "comments_mode": self.comments_mode }),
            }),
        )
    }
}

pub(crate) fn blog_settings_reader(
    comments_mode: &'static str,
) -> SharedStaticModuleSettingsReader {
    SharedStaticModuleSettingsReader(Arc::new(BlogSettingsReader { comments_mode }))
}

pub(crate) async fn setup_conversion_test_db() -> DatabaseConnection {
    let mut opts = ConnectOptions::new("sqlite::memory:");
    opts.max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect server content orchestration sqlite database")
}

pub(crate) async fn ensure_conversion_schema(db: &DatabaseConnection) {
    db.execute_unprepared(
        "CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
    )
    .await
    .expect("minimal platform identity relation should apply");
    let manager = SchemaManager::new(db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in ContentModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("content migration should apply");
    }
    for migration in CommentsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("comments migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in rustok_blog::BlogModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
    for migration in ForumModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("forum migration should apply");
    }
}

pub(crate) fn admin_security() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

pub(crate) async fn insert_test_actor(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: &SecurityContext,
) {
    let user_id = security
        .user_id
        .expect("test admin security should carry a user id");
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        [user_id.to_string().into(), tenant_id.to_string().into()],
    ))
    .await
    .expect("test actor should be inserted into the platform identity relation");
}
