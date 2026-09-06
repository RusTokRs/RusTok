use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChannelResolutionPolicyRules::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::PolicySetId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::Priority)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::ActionChannelId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::Definition)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicyRules::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channel_resolution_policy_rules_policy_set_id")
                            .from(
                                ChannelResolutionPolicyRules::Table,
                                ChannelResolutionPolicyRules::PolicySetId,
                            )
                            .to(
                                ChannelResolutionPolicySets::Table,
                                ChannelResolutionPolicySets::Id,
                            )
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channel_resolution_policy_rules_action_channel_id")
                            .from(
                                ChannelResolutionPolicyRules::Table,
                                ChannelResolutionPolicyRules::ActionChannelId,
                            )
                            .to(Channels::Table, Channels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channel_resolution_policy_rules_set_priority")
                    .table(ChannelResolutionPolicyRules::Table)
                    .col(ChannelResolutionPolicyRules::PolicySetId)
                    .col(ChannelResolutionPolicyRules::Priority)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channel_resolution_policy_rules_action_channel")
                    .table(ChannelResolutionPolicyRules::Table)
                    .col(ChannelResolutionPolicyRules::ActionChannelId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ChannelResolutionPolicyRules::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ChannelResolutionPolicySets {
    Table,
    Id,
}

#[derive(Iden)]
enum Channels {
    Table,
    Id,
}

#[derive(Iden)]
enum ChannelResolutionPolicyRules {
    Table,
    Id,
    PolicySetId,
    Priority,
    IsActive,
    ActionChannelId,
    Definition,
    CreatedAt,
    UpdatedAt,
}
