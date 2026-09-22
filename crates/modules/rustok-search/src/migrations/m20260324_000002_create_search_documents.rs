use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SearchDocuments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SearchDocuments::DocumentKey)
                            .string_len(200)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SearchDocuments::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(SearchDocuments::DocumentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::SourceModule)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::EntityType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::IsPublic)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(SearchDocuments::Title).text().not_null())
                    .col(ColumnDef::new(SearchDocuments::Subtitle).text())
                    .col(ColumnDef::new(SearchDocuments::Slug).string_len(255))
                    .col(ColumnDef::new(SearchDocuments::Handle).string_len(255))
                    .col(
                        ColumnDef::new(SearchDocuments::Body)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::KeywordsText)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::Facets)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::Payload)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::SearchVector)
                            .custom(Alias::new("tsvector")),
                    )
                    .col(ColumnDef::new(SearchDocuments::PublishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(SearchDocuments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchDocuments::IndexedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_documents_lookup")
                    .table(SearchDocuments::Table)
                    .col(SearchDocuments::TenantId)
                    .col(SearchDocuments::EntityType)
                    .col(SearchDocuments::SourceModule)
                    .col(SearchDocuments::Status)
                    .col(SearchDocuments::Locale)
                    .col(SearchDocuments::IsPublic)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_documents_entity")
                    .table(SearchDocuments::Table)
                    .col(SearchDocuments::TenantId)
                    .col(SearchDocuments::DocumentId)
                    .col(SearchDocuments::Locale)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_search_documents_fts ON search_documents USING GIN (search_vector)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                CREATE OR REPLACE FUNCTION search_documents_search_trigger() RETURNS trigger AS $$
                BEGIN
                    NEW.search_vector :=
                        setweight(to_tsvector('simple', COALESCE(NEW.title, '')), 'A') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.subtitle, '')), 'B') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.keywords_text, '')), 'B') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.body, '')), 'C');
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;

                CREATE TRIGGER search_documents_search_update
                    BEFORE INSERT OR UPDATE ON search_documents
                    FOR EACH ROW
                    EXECUTE FUNCTION search_documents_search_trigger();
            "#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "DROP TRIGGER IF EXISTS search_documents_search_update ON search_documents",
                )
                .await?;
            manager
                .get_connection()
                .execute_unprepared("DROP FUNCTION IF EXISTS search_documents_search_trigger")
                .await?;
        }

        manager
            .drop_table(Table::drop().table(SearchDocuments::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum SearchDocuments {
    Table,
    DocumentKey,
    TenantId,
    DocumentId,
    SourceModule,
    EntityType,
    Locale,
    Status,
    IsPublic,
    Title,
    Subtitle,
    Slug,
    Handle,
    Body,
    KeywordsText,
    Facets,
    Payload,
    SearchVector,
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    IndexedAt,
}
