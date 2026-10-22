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
                        ColumnDef::new(RegistryPublicationEvidence::SignatureDigestSha256)
                            .string_len(64)
                            .null(),
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
                    .check(Check::named(
                        "chk_registry_publication_evidence_author_signature_digest",
                        Expr::cust(
                            "(authority <> 'author_signature' OR signature_digest_sha256 IS NOT NULL) \
                             AND (signature_digest_sha256 IS NULL OR length(signature_digest_sha256) = 64)",
                        ),
                    ))
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
    SignatureDigestSha256,
    EvidenceDigestSha256,
    RecordedByPrincipal,
    CreatedAt,
}

#[derive(DeriveIden)]
enum RegistryPublishRequests {
    Table,
    Id,
}

#[cfg(test)]
mod tests {
    use super::Migration;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    #[tokio::test]
    async fn sqlite_schema_requires_a_complete_author_signature_digest() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE registry_publish_requests (id TEXT PRIMARY KEY); \
                 INSERT INTO registry_publish_requests (id) VALUES ('request-1')"
                    .to_string(),
            ))
            .await
            .expect("migration prerequisite schema");

        let manager = SchemaManager::new(&database);
        Migration
            .up(&manager)
            .await
            .expect("publication evidence migration");

        for (id, signature_digest_sql) in [
            ("missing-digest", "NULL"),
            ("short-digest", "'not-a-sha256-digest'"),
        ] {
            let error = database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!(
                        "INSERT INTO registry_publication_evidence \
                         (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                          issuer_identity, policy_revision, signature_digest_sha256, evidence_digest_sha256, \
                          recorded_by_principal) \
                         VALUES ('{id}', 'request-1', 'author_signature', '{}', 'evidence://author', \
                                 'author', 'policy', {signature_digest_sql}, '{}', '{{}}')",
                        "a".repeat(64),
                        "b".repeat(64),
                    ),
                ))
                .await
                .expect_err("an author signature without a complete digest must be rejected");
            assert!(error.to_string().contains("CHECK"), "{error}");
        }

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                format!(
                    "INSERT INTO registry_publication_evidence \
                     (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                      issuer_identity, policy_revision, signature_digest_sha256, evidence_digest_sha256, \
                      recorded_by_principal) \
                     VALUES ('build-attestation', 'request-1', 'build_service_attestation', '{}', \
                             'evidence://build', 'build-service', 'policy', NULL, '{}', '{{}}')",
                    "c".repeat(64),
                    "d".repeat(64),
                ),
            ))
            .await
            .expect("non-author evidence may use its authority-specific payload contract");
    }
}
