use rustok_content::richtext::{RichTextProfile, parse_json};
use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("comment_bodies", "body_format").await? {
            return Ok(());
        }

        let db = manager.get_connection();
        let backend = db.get_database_backend();
        let rows = db
            .query_all(Statement::from_string(
                backend,
                "SELECT id, body FROM comment_bodies".to_string(),
            ))
            .await?;

        for row in rows {
            let id: Uuid = row.try_get("", "id")?;
            let body: String = row.try_get("", "body")?;
            if let Err(error) = parse_json(&body, RichTextProfile::Comment) {
                return Err(DbErr::Migration(format!(
                    "comment body {id} is not canonical comment richtext ({:?}); convert stored comment bodies before retrying the migration",
                    error.code()
                )));
            }
        }

        manager
            .alter_table(
                Table::alter()
                    .table(CommentBodies::Table)
                    .drop_column(CommentBodies::BodyFormat)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "the comment richtext cutover is irreversible because the removed format selector cannot be reconstructed"
                .to_string(),
        ))
    }
}

#[derive(DeriveIden)]
enum CommentBodies {
    Table,
    BodyFormat,
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, DbBackend};

    use super::*;

    async fn legacy_database(body: &str) -> sea_orm::DatabaseConnection {
        let database = Database::connect(format!(
            "sqlite:file:comments_richtext_cutover_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        ))
        .await
        .expect("SQLite connection");
        let manager = SchemaManager::new(&database);
        manager
            .create_table(
                Table::create()
                    .table(CommentBodies::Table)
                    .col(
                        ColumnDef::new(CommentBodyFixture::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CommentBodyFixture::Body).text().not_null())
                    .col(
                        ColumnDef::new(CommentBodies::BodyFormat)
                            .string_len(32)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("legacy table");
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO comment_bodies (id, body, body_format) VALUES (?, ?, ?)",
                vec![Uuid::new_v4().into(), body.into(), "markdown".into()],
            ))
            .await
            .expect("legacy body");
        database
    }

    #[tokio::test]
    async fn drops_the_selector_after_canonical_rows_pass_preflight() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "Hello"}]
            }]
        })
        .to_string();
        let database = legacy_database(&body).await;
        let manager = SchemaManager::new(&database);

        Migration
            .up(&manager)
            .await
            .expect("canonical rows should migrate");

        assert!(
            !manager
                .has_column("comment_bodies", "body_format")
                .await
                .expect("column lookup")
        );
    }

    #[tokio::test]
    async fn preserves_the_selector_when_a_row_needs_offline_conversion() {
        let database = legacy_database("legacy Markdown").await;
        let manager = SchemaManager::new(&database);

        let error = Migration
            .up(&manager)
            .await
            .expect_err("legacy Markdown must fail closed");

        assert!(error.to_string().contains("convert stored comment bodies"));
        assert!(
            manager
                .has_column("comment_bodies", "body_format")
                .await
                .expect("column lookup")
        );
    }

    #[derive(DeriveIden)]
    enum CommentBodyFixture {
        Id,
        Body,
    }
}
