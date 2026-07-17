use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegistryPublicationEvidence::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::Id)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::RequestId)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::Authority)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::SubjectDigestSha256)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::EvidenceReference)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::IssuerIdentity)
                            .string_len(256)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::PolicyRevision)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::EvidenceDigestSha256)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::RecordedByPrincipal)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryPublicationEvidence::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_registry_publication_evidence_request_id")
                            .from(
                                RegistryPublicationEvidence::Table,
                                RegistryPublicationEvidence::RequestId,
                            )
                            .to(RegistryPublishRequests::Table, RegistryPublishRequests::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_registry_publication_evidence_request_digest")
                    .table(RegistryPublicationEvidence::Table)
                    .col(RegistryPublicationEvidence::RequestId)
                    .col(RegistryPublicationEvidence::EvidenceDigestSha256)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RegistryPublicationEvidence::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegistryPublicationEvidence {
    Table,
    Id,
    RequestId,
    Authority,
    SubjectDigestSha256,
    EvidenceReference,
    IssuerIdentity,
    PolicyRevision,
    EvidenceDigestSha256,
    RecordedByPrincipal,
    CreatedAt,
}

#[derive(DeriveIden)]
enum RegistryPublishRequests {
    Table,
    Id,
}
