use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::Utc;
use rustok_content::{
    CanonicalUrlMutation, ContentError, ContentResult, MergeTopicsInput, MergeTopicsOutput,
    RetiredCanonicalTarget,
};
use rustok_forum::{forum_reply, forum_topic};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseTransaction, EntityTrait};
use uuid::Uuid;

use crate::bridge::helpers::{
    adjust_forum_category_counters_in_tx, find_topic_in_tx, forum_topic_route,
    load_forum_reply_records_in_tx, load_topic_translations_in_tx, locales_from_topic_translations,
    next_forum_reply_position_in_tx, refresh_forum_topic_stats_in_tx,
    resequence_forum_topic_replies_in_tx,
};

pub(crate) async fn merge_topics(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    _actor_id: Option<Uuid>,
    input: &MergeTopicsInput,
) -> ContentResult<MergeTopicsOutput> {
    let target_topic = find_topic_in_tx(txn, tenant_id, input.target_topic_id).await?;
    let source_ids = unique_source_ids(input.target_topic_id, &input.source_topic_ids)?;
    let target_translations =
        load_topic_translations_in_tx(txn, tenant_id, target_topic.id).await?;
    let mut next_position = next_forum_reply_position_in_tx(txn, target_topic.id).await?;
    let mut moved_count = 0_u64;
    let now = Utc::now();
    let mut source_topics = Vec::new();
    let mut merge_locales = locales_from_topic_translations(&target_translations)?
        .into_iter()
        .collect::<BTreeSet<_>>();

    for source_topic_id in &source_ids {
        let source_topic = find_topic_in_tx(txn, tenant_id, *source_topic_id).await?;
        let source_translations =
            load_topic_translations_in_tx(txn, tenant_id, source_topic.id).await?;
        merge_locales.extend(locales_from_topic_translations(&source_translations)?);
        let replies = load_forum_reply_records_in_tx(txn, tenant_id, source_topic.id).await?;
        for record in &replies {
            let mut active: forum_reply::ActiveModel = record.reply.clone().into();
            active.topic_id = Set(target_topic.id);
            active.position = Set(next_position);
            active.updated_at = Set(now.into());
            active.update(txn).await?;
            next_position += 1;
        }
        moved_count += replies.len() as u64;
        source_topics.push(source_topic);
    }

    resequence_forum_topic_replies_in_tx(txn, tenant_id, target_topic.id).await?;
    refresh_forum_topic_stats_in_tx(txn, tenant_id, target_topic.id).await?;

    let mut category_topic_delta: HashMap<Uuid, i32> = HashMap::new();
    let mut category_reply_delta: HashMap<Uuid, i32> = HashMap::new();
    *category_reply_delta
        .entry(target_topic.category_id)
        .or_default() += moved_count as i32;
    for source_topic in &source_topics {
        *category_topic_delta
            .entry(source_topic.category_id)
            .or_default() -= 1;
        *category_reply_delta
            .entry(source_topic.category_id)
            .or_default() -= source_topic.reply_count;
        forum_topic::Entity::delete_by_id(source_topic.id)
            .exec(txn)
            .await?;
    }
    for (category_id, topic_delta) in category_topic_delta {
        let reply_delta = category_reply_delta
            .remove(&category_id)
            .unwrap_or_default();
        adjust_forum_category_counters_in_tx(txn, tenant_id, category_id, topic_delta, reply_delta)
            .await?;
    }
    for (category_id, reply_delta) in category_reply_delta {
        adjust_forum_category_counters_in_tx(txn, tenant_id, category_id, 0, reply_delta).await?;
    }

    let alias_urls = source_topics
        .iter()
        .map(|source_topic| forum_topic_route(source_topic.id))
        .collect::<Vec<_>>();
    let retired_target_ids = source_topics
        .iter()
        .map(|source_topic| source_topic.id)
        .collect::<Vec<_>>();
    let url_updates = merge_locales
        .into_iter()
        .map(|locale| CanonicalUrlMutation {
            target_kind: "forum_topic".to_string(),
            target_id: target_topic.id,
            locale: locale.clone(),
            canonical_url: forum_topic_route(target_topic.id),
            alias_urls: alias_urls.clone(),
            retired_targets: retired_target_ids
                .iter()
                .copied()
                .map(|target_id| RetiredCanonicalTarget {
                    target_kind: "forum_topic".to_string(),
                    target_id,
                    locale: locale.clone(),
                })
                .collect(),
        })
        .collect();

    Ok(MergeTopicsOutput {
        target_topic_id: target_topic.id,
        source_topic_ids: source_ids,
        moved_comments: moved_count,
        url_updates,
    })
}

pub(crate) fn unique_source_ids(
    target_topic_id: Uuid,
    source_ids: &[Uuid],
) -> ContentResult<Vec<Uuid>> {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for source_id in source_ids {
        if *source_id == target_topic_id {
            return Err(ContentError::validation(
                "merge_topics source topics cannot include the target topic",
            ));
        }
        if seen.insert(*source_id) {
            unique.push(*source_id);
        }
    }
    if unique.is_empty() {
        return Err(ContentError::validation(
            "merge_topics requires at least one source topic",
        ));
    }
    Ok(unique)
}
