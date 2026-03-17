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
                    .table(IndexProducts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IndexProducts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(IndexProducts::TenantId).uuid().not_null())
                    .col(ColumnDef::new(IndexProducts::ProductId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexProducts::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::IsPublished)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexProducts::Subtitle).string_len(255))
                    .col(
                        ColumnDef::new(IndexProducts::Handle)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexProducts::Description).text())
                    .col(ColumnDef::new(IndexProducts::CategoryId).uuid())
                    .col(ColumnDef::new(IndexProducts::CategoryName).string_len(255))
                    .col(ColumnDef::new(IndexProducts::CategoryPath).string_len(1000))
                    .col(
                        ColumnDef::new(IndexProducts::Tags)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(ColumnDef::new(IndexProducts::Brand).string_len(100))
                    .col(ColumnDef::new(IndexProducts::Currency).string_len(3))
                    .col(ColumnDef::new(IndexProducts::PriceMin).big_integer())
                    .col(ColumnDef::new(IndexProducts::PriceMax).big_integer())
                    .col(ColumnDef::new(IndexProducts::CompareAtPriceMin).big_integer())
                    .col(ColumnDef::new(IndexProducts::CompareAtPriceMax).big_integer())
                    .col(
                        ColumnDef::new(IndexProducts::OnSale)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::InStock)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::TotalInventory)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::VariantCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::Options)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(ColumnDef::new(IndexProducts::ThumbnailUrl).string_len(500))
                    .col(
                        ColumnDef::new(IndexProducts::Images)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(ColumnDef::new(IndexProducts::MetaTitle).string_len(255))
                    .col(ColumnDef::new(IndexProducts::MetaDescription).string_len(500))
                    .col(
                        ColumnDef::new(IndexProducts::Attributes)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::SalesCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::ViewCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(IndexProducts::Rating).decimal_len(3, 2))
                    .col(
                        ColumnDef::new(IndexProducts::ReviewCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(IndexProducts::SearchVector).custom(Alias::new("tsvector")))
                    .col(ColumnDef::new(IndexProducts::PublishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(IndexProducts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexProducts::IndexedAt)
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
                    .name("idx_index_products_unique")
                    .table(IndexProducts::Table)
                    .col(IndexProducts::ProductId)
                    .col(IndexProducts::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_products_list")
                    .table(IndexProducts::Table)
                    .col(IndexProducts::TenantId)
                    .col(IndexProducts::Locale)
                    .col(IndexProducts::IsPublished)
                    .col(IndexProducts::InStock)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_products_handle")
                    .table(IndexProducts::Table)
                    .col(IndexProducts::TenantId)
                    .col(IndexProducts::Locale)
                    .col(IndexProducts::Handle)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_products_category")
                    .table(IndexProducts::Table)
                    .col(IndexProducts::TenantId)
                    .col(IndexProducts::CategoryId)
                    .col(IndexProducts::Locale)
                    .col(IndexProducts::IsPublished)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_products_price")
                    .table(IndexProducts::Table)
                    .col(IndexProducts::TenantId)
                    .col(IndexProducts::Locale)
                    .col(IndexProducts::PriceMin)
                    .col(IndexProducts::PriceMax)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_index_products_search ON index_products USING GIN (search_vector)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_index_products_attrs ON index_products USING GIN (attributes jsonb_path_ops)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                CREATE OR REPLACE FUNCTION index_products_search_trigger() RETURNS trigger AS $$
                BEGIN
                    NEW.search_vector :=
                        setweight(to_tsvector('simple', COALESCE(NEW.title, '')), 'A') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.subtitle, '')), 'B') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.description, '')), 'C') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.brand, '')), 'B') ||
                        setweight(to_tsvector('simple', COALESCE(NEW.category_name, '')), 'C');
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;

                CREATE TRIGGER index_products_search_update
                    BEFORE INSERT OR UPDATE ON index_products
                    FOR EACH ROW
                    EXECUTE FUNCTION index_products_search_trigger();
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
                    "DROP TRIGGER IF EXISTS index_products_search_update ON index_products",
                )
                .await?;
            manager
                .get_connection()
                .execute_unprepared("DROP FUNCTION IF EXISTS index_products_search_trigger")
                .await?;
        }
        manager
            .drop_table(Table::drop().table(IndexProducts::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum IndexProducts {
    Table,
    Id,
    TenantId,
    ProductId,
    Locale,
    Status,
    IsPublished,
    Title,
    Subtitle,
    Handle,
    Description,
    CategoryId,
    CategoryName,
    CategoryPath,
    Tags,
    Brand,
    Currency,
    PriceMin,
    PriceMax,
    CompareAtPriceMin,
    CompareAtPriceMax,
    OnSale,
    InStock,
    TotalInventory,
    VariantCount,
    Options,
    ThumbnailUrl,
    Images,
    MetaTitle,
    MetaDescription,
    Attributes,
    SalesCount,
    ViewCount,
    Rating,
    ReviewCount,
    SearchVector,
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    IndexedAt,
}
