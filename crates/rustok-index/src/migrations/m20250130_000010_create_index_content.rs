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
                    .table(IndexContent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IndexContent::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(IndexContent::TenantId).uuid().not_null())
                    .col(ColumnDef::new(IndexContent::NodeId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexContent::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexContent::Kind).string_len(32).not_null())
                    .col(
                        ColumnDef::new(IndexContent::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexContent::Title).string_len(255))
                    .col(ColumnDef::new(IndexContent::Slug).string_len(255))
                    .col(ColumnDef::new(IndexContent::Excerpt).text())
                    .col(ColumnDef::new(IndexContent::Body).text())
                    .col(ColumnDef::new(IndexContent::BodyFormat).string_len(16))
                    .col(ColumnDef::new(IndexContent::AuthorId).uuid())
                    .col(ColumnDef::new(IndexContent::AuthorName).string_len(255))
                    .col(ColumnDef::new(IndexContent::AuthorAvatar).string_len(500))
                    .col(ColumnDef::new(IndexContent::CategoryId).uuid())
                    .col(ColumnDef::new(IndexContent::CategoryName).string_len(255))
                    .col(ColumnDef::new(IndexContent::CategorySlug).string_len(255))
                    .col(
                        ColumnDef::new(IndexContent::Tags)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(ColumnDef::new(IndexContent::MetaTitle).string_len(255))
                    .col(ColumnDef::new(IndexContent::MetaDescription).string_len(500))
                    .col(ColumnDef::new(IndexContent::OgImage).string_len(500))
                    .col(ColumnDef::new(IndexContent::FeaturedImageUrl).string_len(500))
                    .col(ColumnDef::new(IndexContent::FeaturedImageAlt).string_len(255))
                    .col(ColumnDef::new(IndexContent::ParentId).uuid())
                    .col(
                        ColumnDef::new(IndexContent::Depth)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexContent::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexContent::ReplyCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexContent::ViewCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(IndexContent::SearchVector).custom(Alias::new("tsvector")))
                    .col(ColumnDef::new(IndexContent::PublishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(IndexContent::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexContent::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexContent::IndexedAt)
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
                    .name("idx_index_content_unique")
                    .table(IndexContent::Table)
                    .col(IndexContent::NodeId)
                    .col(IndexContent::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_content_list")
                    .table(IndexContent::Table)
                    .col(IndexContent::TenantId)
                    .col(IndexContent::Kind)
                    .col(IndexContent::Status)
                    .col(IndexContent::Locale)
                    .col(IndexContent::PublishedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_content_slug")
                    .table(IndexContent::Table)
                    .col(IndexContent::TenantId)
                    .col(IndexContent::Locale)
                    .col(IndexContent::Kind)
                    .col(IndexContent::Slug)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_content_category")
                    .table(IndexContent::Table)
                    .col(IndexContent::TenantId)
                    .col(IndexContent::CategoryId)
                    .col(IndexContent::Locale)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_content_author")
                    .table(IndexContent::Table)
                    .col(IndexContent::TenantId)
                    .col(IndexContent::AuthorId)
                    .col(IndexContent::Locale)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_index_content_search ON index_content USING GIN (search_vector)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                CREATE OR REPLACE FUNCTION index_content_search_trigger() RETURNS trigger AS $$
                BEGIN
                    NEW.search_vector :=
                        setweight(to_tsvector('simple', COALESCE(NEW.title, '')), 'A') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.excerpt, '')), 'B') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.body, '')), 'C') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.category_name, '')), 'D') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.author_name, '')), 'D');
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;

                CREATE TRIGGER index_content_search_update
                    BEFORE INSERT OR UPDATE ON index_content
                    FOR EACH ROW
                    EXECUTE FUNCTION index_content_search_trigger();
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
                    "DROP TRIGGER IF EXISTS index_content_search_update ON index_content",
                )
                .await?;
            manager
                .get_connection()
                .execute_unprepared("DROP FUNCTION IF EXISTS index_content_search_trigger")
                .await?;
        }
        manager
            .drop_table(Table::drop().table(IndexContent::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum IndexContent {
    Table,
    Id,
    TenantId,
    NodeId,
    Locale,
    Kind,
    Status,
    Title,
    Slug,
    Excerpt,
    Body,
    BodyFormat,
    AuthorId,
    AuthorName,
    AuthorAvatar,
    CategoryId,
    CategoryName,
    CategorySlug,
    Tags,
    MetaTitle,
    MetaDescription,
    OgImage,
    FeaturedImageUrl,
    FeaturedImageAlt,
    ParentId,
    Depth,
    Position,
    ReplyCount,
    ViewCount,
    SearchVector,
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    IndexedAt,
}
