#[cfg(test)]
mod tests {
    use crate::{
        CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
        ListRepliesFilter, ReplyService, TopicService, migrations,
    };
    use rustok_core::SecurityContext;
    use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use std::sync::Arc;
    use uuid::Uuid;

    async fn setup_forum_test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:forum_reply_service_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);

        Database::connect(opts)
            .await
            .expect("failed to connect forum reply test sqlite database")
    }

    async fn ensure_forum_schema(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
        db.execute_unprepared(
            "CREATE TABLE users (
                id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                PRIMARY KEY (id),
                UNIQUE (tenant_id, id)
            )",
        )
        .await
        .expect("identity owner table should exist for forum tests");

        for migration in rustok_taxonomy::migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("taxonomy migration should apply");
        }

        for migration in migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("forum migration should apply");
        }
    }

    #[tokio::test]
    async fn list_response_preserves_reply_order_by_position() {
        let db = setup_forum_test_db().await;
        ensure_forum_schema(&db).await;

        let transport = OutboxTransport::new(db.clone());
        let event_bus = TransactionalEventBus::new(Arc::new(transport));

        let tenant_id = Uuid::new_v4();
        let security = SecurityContext::system();

        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "General".to_string(),
                    slug: "general".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("category should be created");

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Ordered topic".to_string(),
                    slug: Some("ordered-topic".to_string()),
                    body: rustok_api::RichTextDocument::single_paragraph("Body"),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: None,
                },
            )
            .await
            .expect("topic should be created");

        let service = ReplyService::new(db.clone(), event_bus.clone());
        for content in ["first", "second", "third"] {
            service
                .create(
                    tenant_id,
                    security.clone(),
                    topic.id,
                    CreateReplyInput {
                        locale: "en".to_string(),
                        content: rustok_api::RichTextDocument::single_paragraph(content),
                        parent_reply_id: None,
                    },
                )
                .await
                .expect("reply should be created");
        }

        let (replies, total) = service
            .list_response_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic.id,
                ListRepliesFilter {
                    locale: Some("en".to_string()),
                    page: 1,
                    per_page: 20,
                },
                None,
            )
            .await
            .expect("reply list should load");

        assert_eq!(total, 3);
        assert_eq!(replies.len(), 3);
        let contents = replies
            .into_iter()
            .map(|reply| reply.content.document)
            .collect::<Vec<_>>();
        assert_eq!(
            contents,
            vec![
                rustok_api::RichTextDocument::single_paragraph("first"),
                rustok_api::RichTextDocument::single_paragraph("second"),
                rustok_api::RichTextDocument::single_paragraph("third"),
            ]
        );
    }
}
