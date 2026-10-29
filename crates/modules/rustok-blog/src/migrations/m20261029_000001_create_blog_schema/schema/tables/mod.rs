//! Table creation and teardown for the canonical Blog schema migration.

use super::*;

mod categories;
mod channel_visibility;
mod comment_projection_deliveries;
mod post_tags;
mod posts;
mod preview_tokens;
mod routes;
mod schedule_audit_outbox;
mod schedule_audit_recovery;
mod schedule_state;
mod tag_usage;
mod translations;

pub(super) async fn up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    posts::create(manager).await?;
    translations::create(manager).await?;
    channel_visibility::create(manager).await?;
    categories::create(manager).await?;
    post_tags::create(manager).await?;
    tag_usage::create(manager).await?;
    routes::create(manager).await?;
    preview_tokens::create(manager).await?;
    comment_projection_deliveries::create(manager).await?;
    schedule_state::create(manager).await?;
    schedule_audit_outbox::create(manager).await?;
    schedule_audit_recovery::create(manager).await?;
    Ok(())
}

pub(super) async fn down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for table in [
        BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::Table.into_table_ref(),
        BlogCommentsTcpDelegationScheduleAuditOutbox::Table.into_table_ref(),
        BlogCommentsTcpDelegationScheduleState::Table.into_table_ref(),
        BlogCommentProjectionDeliveries::Table.into_table_ref(),
        BlogPreviewTokens::Table.into_table_ref(),
        BlogPostRoutes::Table.into_table_ref(),
        BlogTagUsage::Table.into_table_ref(),
        BlogPostTags::Table.into_table_ref(),
        BlogPostChannelVisibility::Table.into_table_ref(),
        BlogPostTranslations::Table.into_table_ref(),
        BlogPosts::Table.into_table_ref(),
        BlogCategories::Table.into_table_ref(),
    ] {
        manager
            .drop_table(Table::drop().table(table).if_exists().to_owned())
            .await?;
    }
    Ok(())
}
