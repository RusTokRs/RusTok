use chrono::Utc;
use rustok_blog::{blog_post, blog_post_tag, blog_post_translation};
use rustok_comments::{comment, comment_thread};
use rustok_content::{
    ContentError, ContentResult, DemotePostToTopicInput,
    DemotePostToTopicOutput, resolve_by_locale_with_fallback,
};
use rustok_forum::{TopicStatus, forum_topic, forum_topic_translation};
use rustok_taxonomy::TaxonomyService;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder,
};
use uuid::Uuid;

use crate::bridge::helpers::{
    adjust_forum_category_counters_in_tx, ensure_forum_category_exists_in_tx,
    normalize_locale, refresh_forum_topic_stats_in_tx, resequence_forum_topic_replies_in_tx,
};
use crate::bridge::OwnerRoutes;
use rustok_forum::services::topic_routes::forum_topic_route;
use crate::bridge::tags::{load_blog_tag_names_for_post_in_tx, sync_forum_tags_for_topic_in_tx};

pub(crate) async fn demote_post_to_topic(
    taxonomy: &TaxonomyService,
    routes: &OwnerRoutes,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    input: &DemotePostToTopicInput,
) -> ContentResult<DemotePostToTopicOutput> {
    let requested_locale = normalize_locale(&input.locale)?;
    ensure_forum_category_exists_in_tx(txn, tenant_id, input.forum_category_id).await?;
    let post = find_post_in_tx(txn, tenant_id, input.post_id).await?;
    let translations = load_post_translations_in_tx(txn, post.id).await?;
    let resolved = resolve_post_translation(&translations, &requested_locale)?;
    let tag_names = load_blog_tag_names_for_post_in_tx(
        txn,
        tenant_id,
        post.id,
        &resolved.effective_locale,
        None,
    )
    .await?;
    for translation in &translations {
        rustok_content::richtext::parse_json(
            &translation.body,
            rustok_content::richtext::RichTextProfile::Discussion,
        )
        .map_err(|_| {
            ContentError::validation(
                "Blog post content must be discussion-compatible richtext before demotion to Forum",
            )
        })?;
    }

    let topic_id = Uuid::new_v4();
    let comment_records = load_comment_records_for_post_in_tx(txn, tenant_id, post.id).await?;
    move_comments_to_forum_replies_in_tx(txn, tenant_id, topic_id, &comment_records).await?;

    let now = Utc::now();
    let topic_status_value = match post.status.as_str() {
        "archived" => TopicStatus::Archived,
        _ => TopicStatus::Open,
    };

    forum_topic::ActiveModel {
        id: Set(topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(input.forum_category_id),
        author_id: Set(Some(post.author_id)),
        status: Set(topic_status_value),
        metadata: Set(serde_json::json!({
            "orchestration": {
                "source_type": "blog_post",
                "source_id": post.id,
                "source_category_id": post.category_id,
            }
        })),
        is_pinned: Set(false),
        is_locked: Set(false),
        reply_count: Set(0),
        created_at: Set(post.created_at),
        updated_at: Set(now.into()),
        last_reply_at: Set(None),
    }
    .insert(txn)
    .await?;

    for translation in &translations {
        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(topic_id),
            tenant_id: Set(tenant_id),
            locale: Set(translation.locale.clone()),
            title: Set(translation.title.clone()),
            slug: Set(Some(post.slug.clone())),
            body: Set(translation.body.clone()),
            created_at: Set(translation.created_at),
            updated_at: Set(translation.updated_at),
        }
        .insert(txn)
        .await?;
    }

    sync_forum_tags_for_topic_in_tx(
        taxonomy,
        txn,
        tenant_id,
        topic_id,
        &tag_names,
        &resolved.effective_locale,
    )
    .await?;

    resequence_forum_topic_replies_in_tx(txn, tenant_id, topic_id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, topic_id).await?;
    adjust_forum_category_counters_in_tx(
        txn,
        tenant_id,
        input.forum_category_id,
        1,
        comment_records.len() as i32,
    )
    .await?;

    delete_comments_for_post_in_tx(txn, tenant_id, post.id).await?;
    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post.id))
        .exec(txn)
        .await?;
    blog_post::Entity::delete_by_id(post.id).exec(txn).await?;

    // Routes move from Blog to Forum. The post's canonical route and every
    // redirect that pointed at the post leave Blog and Forum. The retired Blog
    // route then redirects to the new topic.
    routes
        .blog
        .remove_post_routes_in_tx(txn, tenant_id, actor_id, post.id, &post.slug)
        .await?;
    routes
        .forum
        .remove_redirects_to_target_in_tx(txn, tenant_id, actor_id, "blog_post", post.id)
        .await?;
    routes
        .blog
        .redirect_source_route_in_tx(
            txn,
            tenant_id,
            actor_id,
            &rustok_blog::canonical_post_route(&post.slug),
            "forum_topic",
            topic_id,
            &forum_topic_route(topic_id),
        )
        .await?;

    Ok(DemotePostToTopicOutput {
        post_id: post.id,
        topic_id,
        moved_comments: comment_records.len() as u64,
        effective_locale: resolved.effective_locale,
    })
}

pub(crate) async fn find_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> ContentResult<blog_post::Model> {
    blog_post::Entity::find_by_id(post_id)
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| ContentError::node_not_found(post_id))
}

pub(crate) async fn load_post_translations_in_tx(
    txn: &DatabaseTransaction,
    post_id: Uuid,
) -> ContentResult<Vec<blog_post_translation::Model>> {
    Ok(blog_post_translation::Entity::find()
        .filter(blog_post_translation::Column::PostId.eq(post_id))
        .all(txn)
        .await?)
}

pub(crate) fn resolve_post_translation<'a>(
    translations: &'a [blog_post_translation::Model],
    locale: &str,
) -> ContentResult<rustok_content::ResolvedLocale<'a, blog_post_translation::Model>> {
    if translations.is_empty() {
        return Err(ContentError::translation_not_found(Uuid::nil(), locale));
    }
    Ok(resolve_by_locale_with_fallback(
        translations,
        locale,
        None,
        |translation| translation.locale.as_str(),
    ))
}

pub(crate) async fn load_comment_records_for_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> ContentResult<Vec<comment::Model>> {
    let thread = comment_thread::Entity::find()
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .filter(comment_thread::Column::TargetType.eq("blog_post"))
        .filter(comment_thread::Column::TargetId.eq(post_id))
        .one(txn)
        .await?;
    let Some(thread) = thread else {
        return Ok(Vec::new());
    };

    let comments = comment::Entity::find()
        .filter(comment::Column::TenantId.eq(tenant_id))
        .filter(comment::Column::ThreadId.eq(thread.id))
        .order_by_asc(comment::Column::Position)
        .all(txn)
        .await?;
    if comments.is_empty() {
        return Ok(Vec::new());
    }

    Ok(comments.into_iter().collect())
}

pub(crate) async fn move_comments_to_forum_replies_in_tx(
    _txn: &DatabaseTransaction,
    _tenant_id: Uuid,
    _topic_id: Uuid,
    comment_records: &[comment::Model],
) -> ContentResult<()> {
    if comment_records.is_empty() {
        return Ok(());
    }

    Err(ContentError::validation(
        "Blog posts with comments cannot be demoted until Forum uses the canonical richtext contract",
    ))
}

pub(crate) async fn delete_comments_for_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> ContentResult<()> {
    let thread = comment_thread::Entity::find()
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .filter(comment_thread::Column::TargetType.eq("blog_post"))
        .filter(comment_thread::Column::TargetId.eq(post_id))
        .one(txn)
        .await?;
    if let Some(thread) = thread {
        comment_thread::Entity::delete_by_id(thread.id)
            .exec(txn)
            .await?;
    }
    Ok(())
}
