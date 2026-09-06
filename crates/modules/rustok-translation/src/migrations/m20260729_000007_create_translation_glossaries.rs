use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_glossaries(manager).await?;
        create_terms(manager).await?;
        create_receipts(manager).await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Jobs::Table)
                    .add_column(ColumnDef::new(Jobs::GlossaryId).uuid().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Jobs::Table)
                    .add_column(ColumnDef::new(Jobs::GlossaryRevision).big_integer().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: glossary revisions and job bindings are durable workflow evidence.
        Ok(())
    }
}

async fn create_glossaries(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Glossaries::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Glossaries::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Glossaries::TenantId).uuid().not_null())
                .col(ColumnDef::new(Glossaries::Name).string_len(191).not_null())
                .col(
                    ColumnDef::new(Glossaries::NameKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(ColumnDef::new(Glossaries::Description).text().not_null())
                .col(
                    ColumnDef::new(Glossaries::SourceLocale)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::TargetLocale)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::OwnerSlug)
                        .string_len(191)
                        .not_null()
                        .default(""),
                )
                .col(
                    ColumnDef::new(Glossaries::ResourceKind)
                        .string_len(191)
                        .not_null()
                        .default(""),
                )
                .col(
                    ColumnDef::new(Glossaries::FieldKey)
                        .string_len(191)
                        .not_null()
                        .default(""),
                )
                .col(
                    ColumnDef::new(Glossaries::IsActive)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(
                    ColumnDef::new(Glossaries::Revision)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::LastIdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::LastRequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::CreatedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::CreatedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::UpdatedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::UpdatedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Glossaries::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Glossaries::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_glossaries_tenant")
                        .from(Glossaries::Table, Glossaries::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust("source_locale <> target_locale"))
                .check(Expr::cust("revision > 0"))
                .check(Expr::cust("length(name) > 0"))
                .check(Expr::cust("length(description) <= 4096"))
                .check(Expr::cust(
                    "(owner_slug <> '') OR (resource_kind = '' AND field_key = '')",
                ))
                .check(Expr::cust("(resource_kind <> '') OR field_key = ''"))
                .check(Expr::cust(
                    "created_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust(
                    "updated_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_glossaries_tenant_id")
                .table(Glossaries::Table)
                .col(Glossaries::TenantId)
                .col(Glossaries::Id)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_glossaries_name")
                .table(Glossaries::Table)
                .col(Glossaries::TenantId)
                .col(Glossaries::NameKey)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_translation_glossaries_scope")
                .table(Glossaries::Table)
                .col(Glossaries::TenantId)
                .col(Glossaries::IsActive)
                .col(Glossaries::OwnerSlug)
                .col(Glossaries::ResourceKind)
                .col(Glossaries::FieldKey)
                .to_owned(),
        )
        .await
}

async fn create_terms(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Terms::Table)
                .if_not_exists()
                .col(ColumnDef::new(Terms::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Terms::TenantId).uuid().not_null())
                .col(ColumnDef::new(Terms::GlossaryId).uuid().not_null())
                .col(ColumnDef::new(Terms::ConceptKey).string_len(191).not_null())
                .col(ColumnDef::new(Terms::SourceTerm).text().not_null())
                .col(ColumnDef::new(Terms::TargetTerm).text().not_null())
                .col(ColumnDef::new(Terms::Policy).string_len(32).not_null())
                .col(ColumnDef::new(Terms::MatchKind).string_len(32).not_null())
                .col(ColumnDef::new(Terms::CaseSensitive).boolean().not_null())
                .col(ColumnDef::new(Terms::Notes).text().not_null())
                .col(
                    ColumnDef::new(Terms::ValidFromRevision)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Terms::ValidToRevision).big_integer().null())
                .col(
                    ColumnDef::new(Terms::CreatedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Terms::CreatedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Terms::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_glossary_terms_tenant")
                        .from(Terms::Table, Terms::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_glossary_terms_glossary")
                        .from(Terms::Table, Terms::GlossaryId)
                        .to(Glossaries::Table, Glossaries::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust(
                    "policy IN ('preferred', 'allowed', 'forbidden', 'do_not_translate')",
                ))
                .check(Expr::cust(
                    "match_kind IN ('exact', 'whole_word', 'substring')",
                ))
                .check(Expr::cust("valid_from_revision > 0"))
                .check(Expr::cust(
                    "valid_to_revision IS NULL OR valid_to_revision > valid_from_revision",
                ))
                .check(Expr::cust("length(concept_key) > 0"))
                .check(Expr::cust(
                    "length(source_term) > 0 AND length(source_term) <= 2048",
                ))
                .check(Expr::cust(
                    "length(target_term) > 0 AND length(target_term) <= 2048",
                ))
                .check(Expr::cust("length(notes) <= 4096"))
                .check(Expr::cust(
                    "created_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_glossary_term_revision")
                .table(Terms::Table)
                .col(Terms::TenantId)
                .col(Terms::GlossaryId)
                .col(Terms::ConceptKey)
                .col(Terms::TargetTerm)
                .col(Terms::ValidFromRevision)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_translation_glossary_terms_snapshot")
                .table(Terms::Table)
                .col(Terms::TenantId)
                .col(Terms::GlossaryId)
                .col(Terms::ValidFromRevision)
                .col(Terms::ValidToRevision)
                .to_owned(),
        )
        .await
}

async fn create_receipts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Receipts::Table)
                .if_not_exists()
                .col(ColumnDef::new(Receipts::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Receipts::TenantId).uuid().not_null())
                .col(ColumnDef::new(Receipts::GlossaryId).uuid().not_null())
                .col(
                    ColumnDef::new(Receipts::Operation)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Receipts::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Receipts::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Receipts::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Receipts::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Receipts::ResultingGlossaryRevision)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Receipts::Response).json_binary().not_null())
                .col(
                    ColumnDef::new(Receipts::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_glossary_receipts_tenant")
                        .from(Receipts::Table, Receipts::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_glossary_receipts_glossary")
                        .from(Receipts::Table, Receipts::GlossaryId)
                        .to(Glossaries::Table, Glossaries::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust(
                    "operation IN ('create', 'update', 'replace_terms', 'set_active')",
                ))
                .check(Expr::cust("resulting_glossary_revision > 0"))
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_glossary_receipts_idempotency")
                .table(Receipts::Table)
                .col(Receipts::TenantId)
                .col(Receipts::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await
}

#[derive(Iden)]
enum Tenants {
    #[iden = "tenants"]
    Table,
    Id,
}

#[derive(Iden)]
enum Jobs {
    #[iden = "translation_jobs"]
    Table,
    GlossaryId,
    GlossaryRevision,
}

#[derive(Iden)]
enum Glossaries {
    #[iden = "translation_glossaries"]
    Table,
    Id,
    TenantId,
    Name,
    NameKey,
    Description,
    SourceLocale,
    TargetLocale,
    OwnerSlug,
    ResourceKind,
    FieldKey,
    IsActive,
    Revision,
    LastIdempotencyKey,
    LastRequestHash,
    CreatedByActorKind,
    CreatedByActorId,
    UpdatedByActorKind,
    UpdatedByActorId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Terms {
    #[iden = "translation_glossary_terms"]
    Table,
    Id,
    TenantId,
    GlossaryId,
    ConceptKey,
    SourceTerm,
    TargetTerm,
    Policy,
    MatchKind,
    CaseSensitive,
    Notes,
    ValidFromRevision,
    ValidToRevision,
    CreatedByActorKind,
    CreatedByActorId,
    CreatedAt,
}

#[derive(Iden)]
enum Receipts {
    #[iden = "translation_glossary_receipts"]
    Table,
    Id,
    TenantId,
    GlossaryId,
    Operation,
    IdempotencyKey,
    RequestHash,
    RequestedByActorKind,
    RequestedByActorId,
    ResultingGlossaryRevision,
    Response,
    CreatedAt,
}
