use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("ux_moderation_decisions_tenant_id")
                    .table(ModerationDecisions::Table)
                    .col(ModerationDecisions::TenantId)
                    .col(ModerationDecisions::Id)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ModerationDecisionEffects::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModerationDecisionEffects::DecisionId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ModerationDecisionEffects::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationDecisionEffects::SchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationDecisionEffects::EffectKind)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationDecisionEffects::EffectPayload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationDecisionEffects::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::cust("schema_version >= 1"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_moderation_decision_effects_tenant_decision")
                            .from(
                                ModerationDecisionEffects::Table,
                                ModerationDecisionEffects::TenantId,
                            )
                            .from_col(ModerationDecisionEffects::DecisionId)
                            .to(ModerationDecisions::Table, ModerationDecisions::TenantId)
                            .to_col(ModerationDecisions::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_moderation_decision_effects_tenant_kind")
                    .table(ModerationDecisionEffects::Table)
                    .col(ModerationDecisionEffects::TenantId)
                    .col(ModerationDecisionEffects::EffectKind)
                    .col(ModerationDecisionEffects::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ModerationDecisionEffects::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("ux_moderation_decisions_tenant_id")
                    .table(ModerationDecisions::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ModerationDecisions {
    Table,
    Id,
    TenantId,
}

#[derive(DeriveIden)]
enum ModerationDecisionEffects {
    Table,
    DecisionId,
    TenantId,
    SchemaVersion,
    EffectKind,
    EffectPayload,
    CreatedAt,
}
