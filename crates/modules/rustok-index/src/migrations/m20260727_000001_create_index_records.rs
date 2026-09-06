use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IndexSchemas::Table)
                    .col(ColumnDef::new(IndexSchemas::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexSchemas::ModuleName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexSchemas::EntityName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexSchemas::SchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexSchemas::SchemaFingerprint)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexSchemas::SchemaJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexSchemas::Status)
                            .string_len(16)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(IndexSchemas::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(IndexSchemas::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_schemas")
                            .col(IndexSchemas::TenantId)
                            .col(IndexSchemas::ModuleName)
                            .col(IndexSchemas::EntityName)
                            .col(IndexSchemas::SchemaVersion),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_schemas_tenant")
                            .from(IndexSchemas::Table, IndexSchemas::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("schema_version > 0"))
                    .check(Expr::cust(
                        "length(module_name) BETWEEN 1 AND 128 AND module_name = trim(module_name)",
                    ))
                    .check(Expr::cust(
                        "length(entity_name) BETWEEN 1 AND 128 AND entity_name = trim(entity_name)",
                    ))
                    .check(Expr::cust(
                        "length(schema_fingerprint) = 64 AND schema_fingerprint = lower(schema_fingerprint)",
                    ))
                    .check(Expr::cust("status IN ('active', 'retired')"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_index_schemas_fingerprint")
                    .table(IndexSchemas::Table)
                    .col(IndexSchemas::TenantId)
                    .col(IndexSchemas::ModuleName)
                    .col(IndexSchemas::EntityName)
                    .col(IndexSchemas::SchemaVersion)
                    .col(IndexSchemas::SchemaFingerprint)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_schemas_status")
                    .table(IndexSchemas::Table)
                    .col(IndexSchemas::TenantId)
                    .col(IndexSchemas::ModuleName)
                    .col(IndexSchemas::EntityName)
                    .col(IndexSchemas::Status)
                    .col(IndexSchemas::SchemaVersion)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(IndexEntities::Table)
                    .col(ColumnDef::new(IndexEntities::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexEntities::ModuleName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexEntities::EntityName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexEntities::SchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexEntities::EntityId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexEntities::LocaleKey)
                            .string_len(32)
                            .not_null()
                            .default(""),
                    )
                    .col(super::source_version_column(
                        manager.get_database_backend(),
                        IndexEntities::SourceVersion,
                        false,
                    ))
                    .col(
                        ColumnDef::new(IndexEntities::SchemaFingerprint)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexEntities::Payload).json_binary())
                    .col(
                        ColumnDef::new(IndexEntities::IsDeleted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(IndexEntities::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(IndexEntities::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_entities")
                            .col(IndexEntities::TenantId)
                            .col(IndexEntities::ModuleName)
                            .col(IndexEntities::EntityName)
                            .col(IndexEntities::SchemaVersion)
                            .col(IndexEntities::EntityId)
                            .col(IndexEntities::LocaleKey),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_entities_schema")
                            .from(IndexEntities::Table, IndexEntities::TenantId)
                            .from_col(IndexEntities::ModuleName)
                            .from_col(IndexEntities::EntityName)
                            .from_col(IndexEntities::SchemaVersion)
                            .from_col(IndexEntities::SchemaFingerprint)
                            .to(IndexSchemas::Table, IndexSchemas::TenantId)
                            .to_col(IndexSchemas::ModuleName)
                            .to_col(IndexSchemas::EntityName)
                            .to_col(IndexSchemas::SchemaVersion)
                            .to_col(IndexSchemas::SchemaFingerprint)
                            .on_update(ForeignKeyAction::Restrict)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("schema_version > 0"))
                    .check(Expr::cust("source_version >= 0"))
                    .check(Expr::cust(
                        "length(module_name) BETWEEN 1 AND 128 AND module_name = trim(module_name)",
                    ))
                    .check(Expr::cust(
                        "length(entity_name) BETWEEN 1 AND 128 AND entity_name = trim(entity_name)",
                    ))
                    .check(Expr::cust(
                        "length(locale_key) <= 32 AND locale_key = trim(locale_key)",
                    ))
                    .check(Expr::cust(
                        "length(schema_fingerprint) = 64 AND schema_fingerprint = lower(schema_fingerprint)",
                    ))
                    .check(Expr::cust(
                        "(is_deleted = FALSE AND payload IS NOT NULL) OR (is_deleted = TRUE AND payload IS NULL)",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_index_entities_source_version")
                    .table(IndexEntities::Table)
                    .col(IndexEntities::TenantId)
                    .col(IndexEntities::ModuleName)
                    .col(IndexEntities::EntityName)
                    .col(IndexEntities::SchemaVersion)
                    .col(IndexEntities::EntityId)
                    .col(IndexEntities::LocaleKey)
                    .col(IndexEntities::SourceVersion)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_entities_scope")
                    .table(IndexEntities::Table)
                    .col(IndexEntities::TenantId)
                    .col(IndexEntities::ModuleName)
                    .col(IndexEntities::EntityName)
                    .col(IndexEntities::SchemaVersion)
                    .col(IndexEntities::LocaleKey)
                    .col(IndexEntities::IsDeleted)
                    .col(IndexEntities::EntityId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(IndexLinks::Table)
                    .col(ColumnDef::new(IndexLinks::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexLinks::SourceModule)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexLinks::SourceEntity)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexLinks::SourceSchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexLinks::SourceEntityId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexLinks::SourceLocaleKey)
                            .string_len(32)
                            .not_null()
                            .default(""),
                    )
                    .col(super::source_version_column(
                        manager.get_database_backend(),
                        IndexLinks::SourceVersion,
                        false,
                    ))
                    .col(
                        ColumnDef::new(IndexLinks::LinkName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexLinks::Ordinal).integer().not_null())
                    .col(
                        ColumnDef::new(IndexLinks::TargetModule)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexLinks::TargetEntity)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexLinks::TargetSchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexLinks::TargetEntityId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexLinks::TargetLocaleKey)
                            .string_len(32)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(IndexLinks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_links")
                            .col(IndexLinks::TenantId)
                            .col(IndexLinks::SourceModule)
                            .col(IndexLinks::SourceEntity)
                            .col(IndexLinks::SourceSchemaVersion)
                            .col(IndexLinks::SourceEntityId)
                            .col(IndexLinks::SourceLocaleKey)
                            .col(IndexLinks::SourceVersion)
                            .col(IndexLinks::LinkName)
                            .col(IndexLinks::Ordinal),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_links_source_version")
                            .from(IndexLinks::Table, IndexLinks::TenantId)
                            .from_col(IndexLinks::SourceModule)
                            .from_col(IndexLinks::SourceEntity)
                            .from_col(IndexLinks::SourceSchemaVersion)
                            .from_col(IndexLinks::SourceEntityId)
                            .from_col(IndexLinks::SourceLocaleKey)
                            .from_col(IndexLinks::SourceVersion)
                            .to(IndexEntities::Table, IndexEntities::TenantId)
                            .to_col(IndexEntities::ModuleName)
                            .to_col(IndexEntities::EntityName)
                            .to_col(IndexEntities::SchemaVersion)
                            .to_col(IndexEntities::EntityId)
                            .to_col(IndexEntities::LocaleKey)
                            .to_col(IndexEntities::SourceVersion)
                            .on_update(ForeignKeyAction::Restrict)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("source_schema_version > 0"))
                    .check(Expr::cust("target_schema_version > 0"))
                    .check(Expr::cust("source_version >= 0"))
                    .check(Expr::cust("ordinal >= 0"))
                    .check(Expr::cust(
                        "length(source_module) BETWEEN 1 AND 128 AND source_module = trim(source_module)",
                    ))
                    .check(Expr::cust(
                        "length(source_entity) BETWEEN 1 AND 128 AND source_entity = trim(source_entity)",
                    ))
                    .check(Expr::cust(
                        "length(source_locale_key) <= 32 AND source_locale_key = trim(source_locale_key)",
                    ))
                    .check(Expr::cust(
                        "length(link_name) BETWEEN 1 AND 128 AND link_name = trim(link_name)",
                    ))
                    .check(Expr::cust(
                        "length(target_module) BETWEEN 1 AND 128 AND target_module = trim(target_module)",
                    ))
                    .check(Expr::cust(
                        "length(target_entity) BETWEEN 1 AND 128 AND target_entity = trim(target_entity)",
                    ))
                    .check(Expr::cust(
                        "length(target_locale_key) <= 32 AND target_locale_key = trim(target_locale_key)",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_links_source")
                    .table(IndexLinks::Table)
                    .col(IndexLinks::TenantId)
                    .col(IndexLinks::SourceModule)
                    .col(IndexLinks::SourceEntity)
                    .col(IndexLinks::SourceSchemaVersion)
                    .col(IndexLinks::SourceEntityId)
                    .col(IndexLinks::SourceLocaleKey)
                    .col(IndexLinks::LinkName)
                    .col(IndexLinks::Ordinal)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_links_target")
                    .table(IndexLinks::Table)
                    .col(IndexLinks::TenantId)
                    .col(IndexLinks::TargetModule)
                    .col(IndexLinks::TargetEntity)
                    .col(IndexLinks::TargetSchemaVersion)
                    .col(IndexLinks::TargetEntityId)
                    .col(IndexLinks::TargetLocaleKey)
                    .col(IndexLinks::LinkName)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IndexLinks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(IndexEntities::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(IndexSchemas::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum IndexSchemas {
    Table,
    TenantId,
    ModuleName,
    EntityName,
    SchemaVersion,
    SchemaFingerprint,
    SchemaJson,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum IndexEntities {
    Table,
    TenantId,
    ModuleName,
    EntityName,
    SchemaVersion,
    EntityId,
    LocaleKey,
    SourceVersion,
    SchemaFingerprint,
    Payload,
    IsDeleted,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum IndexLinks {
    Table,
    TenantId,
    SourceModule,
    SourceEntity,
    SourceSchemaVersion,
    SourceEntityId,
    SourceLocaleKey,
    SourceVersion,
    LinkName,
    Ordinal,
    TargetModule,
    TargetEntity,
    TargetSchemaVersion,
    TargetEntityId,
    TargetLocaleKey,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Tenants {
    Table,
    Id,
}
