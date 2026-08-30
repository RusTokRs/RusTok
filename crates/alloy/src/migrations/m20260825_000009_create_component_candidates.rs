use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AlloyComponentCandidates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::IdempotencyKey)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::RequestDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ScriptId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ParentRevision)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ParentSourceDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ParentReleaseSlug)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ParentReleaseVersion)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ParentReleaseDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::Workspace)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::SourceDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ScenarioDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::ActorId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_alloy_component_candidates_tenant_idempotency")
                            .col(AlloyComponentCandidates::TenantId)
                            .col(AlloyComponentCandidates::IdempotencyKey),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_alloy_component_candidates_script_revision")
                    .table(AlloyComponentCandidates::Table)
                    .col(AlloyComponentCandidates::TenantId)
                    .col(AlloyComponentCandidates::ScriptId)
                    .col(AlloyComponentCandidates::ParentRevision)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AlloyComponentCandidateReviews::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::CandidateId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::SourceDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::ScenarioDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::PolicyRevision)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::ActorId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AlloyComponentCandidateReviews::Reason).text())
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::IdempotencyKey)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::RequestDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateReviews::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_alloy_component_candidate_reviews_idempotency")
                            .col(AlloyComponentCandidateReviews::CandidateId)
                            .col(AlloyComponentCandidateReviews::IdempotencyKey),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_alloy_component_candidate_reviews_tenant_candidate")
                    .table(AlloyComponentCandidateReviews::Table)
                    .col(AlloyComponentCandidateReviews::TenantId)
                    .col(AlloyComponentCandidateReviews::CandidateId)
                    .col(AlloyComponentCandidateReviews::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AlloyComponentCandidateBuilds::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::CandidateId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::CandidateSourceDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::ScenarioDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::ArchiveSourceDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::BuildRequestId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::SourceReference)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::ActorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::IdempotencyKey)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::RequestDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyComponentCandidateBuilds::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_alloy_component_candidate_builds_idempotency")
                            .col(AlloyComponentCandidateBuilds::CandidateId)
                            .col(AlloyComponentCandidateBuilds::IdempotencyKey),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_alloy_component_candidate_builds_build_request")
                            .col(AlloyComponentCandidateBuilds::BuildRequestId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_alloy_component_candidate_builds_tenant_candidate")
                    .table(AlloyComponentCandidateBuilds::Table)
                    .col(AlloyComponentCandidateBuilds::TenantId)
                    .col(AlloyComponentCandidateBuilds::CandidateId)
                    .col(AlloyComponentCandidateBuilds::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(AlloyComponentCandidateBuilds::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(AlloyComponentCandidateReviews::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(AlloyComponentCandidates::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AlloyComponentCandidates {
    Table,
    Id,
    TenantId,
    IdempotencyKey,
    RequestDigest,
    ScriptId,
    ParentRevision,
    ParentSourceDigest,
    ParentReleaseSlug,
    ParentReleaseVersion,
    ParentReleaseDigest,
    Workspace,
    SourceDigest,
    ScenarioDigest,
    ActorId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum AlloyComponentCandidateReviews {
    Table,
    Id,
    CandidateId,
    TenantId,
    SourceDigest,
    ScenarioDigest,
    Status,
    PolicyRevision,
    ActorId,
    Reason,
    IdempotencyKey,
    RequestDigest,
    CreatedAt,
}

#[derive(DeriveIden)]
enum AlloyComponentCandidateBuilds {
    Table,
    Id,
    CandidateId,
    TenantId,
    CandidateSourceDigest,
    ScenarioDigest,
    ArchiveSourceDigest,
    BuildRequestId,
    SourceReference,
    ActorId,
    IdempotencyKey,
    RequestDigest,
    CreatedAt,
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};

    use super::*;

    async fn table_exists(connection: &sea_orm::DatabaseConnection, table: &str) -> bool {
        connection
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
                vec![table.into()],
            ))
            .await
            .expect("SQLite table lookup")
            .is_some()
    }

    #[tokio::test]
    async fn component_candidate_migration_reverts_every_owned_table() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite memory database");
        let manager = SchemaManager::new(&database);
        let migration = Migration;
        migration.up(&manager).await.expect("migration up");
        for table in [
            "alloy_component_candidates",
            "alloy_component_candidate_reviews",
            "alloy_component_candidate_builds",
        ] {
            assert!(table_exists(&database, table).await, "{table} should exist");
        }
        migration.down(&manager).await.expect("migration down");
        for table in [
            "alloy_component_candidates",
            "alloy_component_candidate_reviews",
            "alloy_component_candidate_builds",
        ] {
            assert!(
                !table_exists(&database, table).await,
                "{table} should be removed"
            );
        }
    }
}
