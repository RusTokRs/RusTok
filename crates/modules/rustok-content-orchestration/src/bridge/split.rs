use std::collections::HashSet;

use chrono::Utc;
use rustok_content::{ContentError, ContentResult, SplitTopicInput, SplitTopicOutput};
use rustok_forum::{forum_reply, forum_topic, forum_topic_translation};
use rustok_taxonomy::TaxonomyService;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseTransaction};
use uuid::Uuid;

use crate::bridge::helpers::{
    adjust_forum_category_counters_in_tx, find_topic_in_tx, load_forum_reply_records_in_tx,
    load_topic_translations_in_tx, normalize_locale, normalize_slug,
    refresh_forum_topic_stats_in_tx, resequence_forum_topic_replies_in_tx,
    resolve_topic_translation,
};
use crate::bridge::tags::{load_forum_tag_names_for_topic_in_tx, sync_forum_tags_for_topic_in_tx};

pub(crate) async fn split_topic(
    taxonomy: &TaxonomyService,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    _actor_id: Option<Uuid>,
    input: &SplitTopicInput,
) -> ContentResult<SplitTopicOutput> {
    let requested_locale = normalize_locale(&input.locale)?;
    let source_topic = find_topic_in_tx(txn, tenant_id, input.topic_id).await?;
    let source_translations =
        load_topic_translations_in_tx(txn, tenant_id, source_topic.id).await?;
    let resolved = resolve_topic_translation(&source_translations, &requested_locale)?;
    if input.reply_ids.is_empty() {
        return Err(ContentError::validation(
            "split_topic requires at least one reply/comment id",
        ));
    }
    let moved_set: HashSet<Uuid> = input.reply_ids.iter().copied().collect();
    if moved_set.len() != input.reply_ids.len() {
        return Err(ContentError::validation(
            "split_topic reply_ids must be unique",
        ));
    }

    let reply_records = load_forum_reply_records_in_tx(txn, tenant_id, source_topic.id).await?;
    let moved_records = reply_records
        .iter()
        .filter(|record| moved_set.contains(&record.reply.id))
        .cloned()
        .collect::<Vec<_>>();
    if moved_records.len() != moved_set.len() {
        return Err(ContentError::validation(
            "split_topic reply_ids must belong to the source topic",
        ));
    }

    let target_topic_id = Uuid::new_v4();
    let now = Utc::now();
    let source_tags = load_forum_tag_names_for_topic_in_tx(
        txn,
        tenant_id,
        source_topic.id,
        &resolved.effective_locale,
        None,
    )
    .await?;
    forum_topic::ActiveModel {
        id: Set(target_topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(source_topic.category_id),
        author_id: Set(source_topic.author_id),
        status: Set(source_topic.status),
        metadata: Set(source_topic.metadata.clone()),
        is_pinned: Set(false),
        is_locked: Set(source_topic.is_locked),
        reply_count: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        last_reply_at: Set(None),
    }
    .insert(txn)
    .await?;

    let mut requested_translation_written = false;
    for translation in &source_translations {
        let (title, slug) = if translation.locale == requested_locale {
            requested_translation_written = true;
            (
                input.new_title.clone(),
                Some(normalize_slug(&input.new_title)),
            )
        } else {
            (translation.title.clone(), translation.slug.clone())
        };
        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(target_topic_id),
            tenant_id: Set(tenant_id),
            locale: Set(translation.locale.clone()),
            title: Set(title),
            slug: Set(slug),
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
        target_topic_id,
        &source_tags,
        &resolved.effective_locale,
    )
    .await?;

    if !requested_translation_written {
        let translation = resolved.item.ok_or_else(|| {
            ContentError::translation_not_found(source_topic.id, requested_locale.clone())
        })?;
        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(target_topic_id),
            tenant_id: Set(tenant_id),
            locale: Set(requested_locale.clone()),
            title: Set(input.new_title.clone()),
            slug: Set(Some(normalize_slug(&input.new_title))),
            body: Set(translation.body.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(txn)
        .await?;
    }

    for (index, record) in moved_records.iter().enumerate() {
        let mut active: forum_reply::ActiveModel = record.reply.clone().into();
        active.topic_id = Set(target_topic_id);
        active.parent_reply_id = Set(record
            .reply
            .parent_reply_id
            .filter(|parent_id| moved_set.contains(parent_id)));
        active.position = Set(index as i64 + 1);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }

    resequence_forum_topic_replies_in_tx(txn, tenant_id, source_topic.id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, source_topic.id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, target_topic_id).await?;
    adjust_forum_category_counters_in_tx(txn, tenant_id, source_topic.category_id, 1, 0).await?;

    Ok(SplitTopicOutput {
        source_topic_id: source_topic.id,
        target_topic_id,
        moved_reply_ids: input.reply_ids.clone(),
        moved_comments: moved_records.len() as u64,
    })
}
