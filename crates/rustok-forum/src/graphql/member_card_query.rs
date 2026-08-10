use std::collections::{HashMap, HashSet};

use async_graphql::{Context, FieldError, Object, Result, SimpleObject, dataloader::DataLoader};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use rustok_profiles::{
    ProfilePresentationService, ProfileSummaryLoader, ProfileSummaryLoaderKey,
    graphql::GqlProfileSummary,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::forum_user_stat;

const MODULE_SLUG: &str = "forum";
pub const MAX_FORUM_MEMBER_CARD_USER_IDS: usize = 100;

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

        if user_ids.len() > MAX_FORUM_MEMBER_CARD_USER_IDS {
            return Err(async_graphql::Error::new(format!(
                "Forum member-card request exceeds the {MAX_FORUM_MEMBER_CARD_USER_IDS}-user limit"
            )));
        }

        let mut seen = HashSet::with_capacity(user_ids.len());
        let mut requested_user_ids = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            if user_id.is_nil() {
                return Err(async_graphql::Error::new(
                    "Forum member-card request contains a nil user ID",
                ));
            }
            if seen.insert(user_id) {
                requested_user_ids.push(user_id);
            }
        }
        if requested_user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let requested_locale = resolve_graphql_locale(ctx, locale.as_deref());
        let mut profiles = load_visible_profiles(
            ctx,
            db,
            tenant.id,
            &requested_user_ids,
            requested_locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;

        if profiles.is_empty() {
            return Ok(Vec::new());
        }

        let visible_user_ids = requested_user_ids
            .iter()
            .copied()
            .filter(|user_id| profiles.contains_key(user_id))
            .collect::<Vec<_>>();
        let mut stats = load_forum_stats(db, tenant.id, &visible_user_ids).await?;

        let mut cards = Vec::with_capacity(visible_user_ids.len());
        for user_id in requested_user_ids {
            let Some(profile) = profiles.remove(&user_id) else {
                continue;
            };
            let forum_stats = stats.remove(&user_id).unwrap_or(GqlForumMemberStats {
                topic_count: 0,
                reply_count: 0,
                solution_count: 0,
            });
            cards.push(GqlForumMemberCard {
                user_id,
                profile,
                forum_stats,
            });
        }
        Ok(cards)
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

async fn load_visible_profiles(
    ctx: &Context<'_>,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_ids: &[Uuid],
    requested_locale: &str,
    tenant_default_locale: &str,
) -> Result<HashMap<Uuid, GqlProfileSummary>> {
    if let Some(loader) = ctx.data_opt::<DataLoader<ProfileSummaryLoader>>() {
        let keys = user_ids
            .iter()
            .map(|user_id| ProfileSummaryLoaderKey {
                tenant_id,
                user_id: *user_id,
                requested_locale: Some(requested_locale.to_string()),
                tenant_default_locale: Some(tenant_default_locale.to_string()),
            })
            .collect::<Vec<_>>();
        let profiles = loader.load_many(keys).await?;
        return Ok(profiles
            .into_iter()
            .map(|(key, summary)| (key.user_id, summary.into()))
            .collect());
    }

    let profiles = ProfilePresentationService::new(db.clone())
        .find_profile_summaries(
            tenant_id,
            user_ids,
            Some(requested_locale),
            Some(tenant_default_locale),
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(profiles
        .into_iter()
        .map(|(user_id, summary)| (user_id, summary.into()))
        .collect())
}

async fn load_forum_stats(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    visible_user_ids: &[Uuid],
) -> Result<HashMap<Uuid, GqlForumMemberStats>> {
    if visible_user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = forum_user_stat::Entity::find()
        .filter(forum_user_stat::Column::TenantId.eq(tenant_id))
        .filter(forum_user_stat::Column::UserId.is_in(visible_user_ids.to_vec()))
        .all(db)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.user_id,
                GqlForumMemberStats {
                    topic_count: row.topic_count,
                    reply_count: row.reply_count,
                    solution_count: row.solution_count,
                },
            )
        })
        .collect())
}
