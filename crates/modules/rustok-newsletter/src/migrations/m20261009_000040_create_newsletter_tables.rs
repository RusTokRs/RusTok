use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // --- newsletter_subscribers ---
        manager
            .create_table(
                Table::create()
                    .table(NewsletterSubscribers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NewsletterSubscribers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::Email)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::Name)
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::Locale)
                            .string_len(16)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::ConfirmToken)
                            .string_len(128)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::SubscribedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::ConfirmedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::UnsubscribedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscribers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique email per tenant
        manager
            .create_index(
                Index::create()
                    .name("idx_newsletter_subscribers_tenant_email")
                    .table(NewsletterSubscribers::Table)
                    .col(NewsletterSubscribers::TenantId)
                    .col(NewsletterSubscribers::Email)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Status index for filtered queries
        manager
            .create_index(
                Index::create()
                    .name("idx_newsletter_subscribers_tenant_status")
                    .table(NewsletterSubscribers::Table)
                    .col(NewsletterSubscribers::TenantId)
                    .col(NewsletterSubscribers::Status)
                    .to_owned(),
            )
            .await?;

        // Confirm token index for double opt-in lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_newsletter_subscribers_confirm_token")
                    .table(NewsletterSubscribers::Table)
                    .col(NewsletterSubscribers::ConfirmToken)
                    .to_owned(),
            )
            .await?;

        // --- newsletter_campaigns ---
        manager
            .create_table(
                Table::create()
                    .table(NewsletterCampaigns::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NewsletterCampaigns::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::Title)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::Subject)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::Preheader)
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::ContentSources)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::SegmentId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::ScheduledAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::SentAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::CreatedBy)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterCampaigns::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Tenant + status index for filtered campaign listings
        manager
            .create_index(
                Index::create()
                    .name("idx_newsletter_campaigns_tenant_status")
                    .table(NewsletterCampaigns::Table)
                    .col(NewsletterCampaigns::TenantId)
                    .col(NewsletterCampaigns::Status)
                    .to_owned(),
            )
            .await?;

        // Scheduled campaigns index for the scheduler worker
        manager
            .create_index(
                Index::create()
                    .name("idx_newsletter_campaigns_scheduled")
                    .table(NewsletterCampaigns::Table)
                    .col(NewsletterCampaigns::Status)
                    .col(NewsletterCampaigns::ScheduledAt)
                    .to_owned(),
            )
            .await?;

        // --- newsletter_subscriptions (subscriber ↔ segment) ---
        manager
            .create_table(
                Table::create()
                    .table(NewsletterSubscriptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NewsletterSubscriptions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscriptions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscriptions::SubscriberId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscriptions::SegmentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscriptions::SubscribedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NewsletterSubscriptions::UnsubscribedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique subscriber+segment pair
        manager
            .create_index(
                Index::create()
                    .name("idx_newsletter_subscriptions_unique")
                    .table(NewsletterSubscriptions::Table)
                    .col(NewsletterSubscriptions::SubscriberId)
                    .col(NewsletterSubscriptions::SegmentId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NewsletterSubscriptions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NewsletterCampaigns::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NewsletterSubscribers::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum NewsletterSubscribers {
    #[iden = "newsletter_subscribers"]
    Table,
    Id,
    TenantId,
    Email,
    Name,
    Status,
    Locale,
    ConfirmToken,
    SubscribedAt,
    ConfirmedAt,
    UnsubscribedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum NewsletterCampaigns {
    #[iden = "newsletter_campaigns"]
    Table,
    Id,
    TenantId,
    Title,
    Subject,
    Preheader,
    Status,
    ContentSources,
    SegmentId,
    ScheduledAt,
    SentAt,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum NewsletterSubscriptions {
    #[iden = "newsletter_subscriptions"]
    Table,
    Id,
    TenantId,
    SubscriberId,
    SegmentId,
    SubscribedAt,
    UnsubscribedAt,
}
