use std::collections::HashMap;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject, dataloader::DataLoader};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use rustok_profiles::{ProfileSummaryLoader, ProfileSummaryLoaderKey, graphql::GqlProfileSummary};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::services::user_stats::{
    ForumMemberCard, ForumMemberCardAudience, ForumMemberCardService, ForumMemberStats,
};

pub use crate::services::user_stats::MAX_FORUM_MEMBER_CARD_USER_IDS;

const MODULE_SLUG: &str = "forum";

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumMemberStats {
    pub topic_count: i32,
    pub reply_count: i32,
    pub solution_count: i32,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumMemberCard {
    pub user_id: Uuid,
    pub profile: GqlProfileSummary,
    pub forum_stats: GqlForumMemberStats,
}

#[derive(Default)]
pub struct ForumMemberCardQuery;

#[Object]
impl ForumMemberCardQuery {
    async fn forum_member_cards(
        &self,
        ctx: &Context<'_>,
        user_ids: Vec<Uuid>,
        locale: Option<String>,
    ) -> Result<Vec<GqlForumMemberCard>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_member_card_permission(ctx)?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let requested_locale = resolve_graphql_locale(ctx, locale.as_deref());
        let service = ForumMemberCardService::new(db.clone());
        let requested_user_ids = ForumMemberCardService::normalize_user_ids(&user_ids)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        if requested_user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let cards = if let Some(loader) = ctx.data_opt::<DataLoader<ProfileSummaryLoader>>() {
            let keys = requested_user_ids
                .iter()
                .map(|user_id| ProfileSummaryLoaderKey {
                    tenant_id: tenant.id,
                    user_id: *user_id,
                    requested_locale: Some(requested_locale.clone()),
                    tenant_default_locale: Some(tenant.default_locale.clone()),
                })
                .collect::<Vec<_>>();
            let profiles = loader
                .load_many(keys)
                .await?
                .into_iter()
                .map(|(key, summary)| (key.user_id, summary))
                .collect::<HashMap<_, _>>();
            service
                .compose_admitted_profiles(tenant.id, &requested_user_ids, profiles)
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?
        } else {
            service
                .read_for_audience(
                    tenant.id,
                    ForumMemberCardAudience::Anonymous,
                    &requested_user_ids,
                    Some(requested_locale.as_str()),
                    Some(tenant.default_locale.as_str()),
                )
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?
        };

        Ok(cards.into_iter().map(map_member_card).collect())
    }
}

fn require_member_card_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_READ]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: forum_topics:read required for member cards",
        ));
    }
    Ok(())
}

fn map_member_card(card: ForumMemberCard) -> GqlForumMemberCard {
    GqlForumMemberCard {
        user_id: card.user_id,
        profile: card.profile.into(),
        forum_stats: map_member_stats(card.forum_stats),
    }
}

fn map_member_stats(stats: ForumMemberStats) -> GqlForumMemberStats {
    GqlForumMemberStats {
        topic_count: stats.topic_count,
        reply_count: stats.reply_count,
        solution_count: stats.solution_count,
    }
}
