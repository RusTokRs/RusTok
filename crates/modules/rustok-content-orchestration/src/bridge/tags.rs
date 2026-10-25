use std::collections::HashMap;

use chrono::Utc;
use rustok_blog::blog_post_tag;
use rustok_blog::services::replace_post_tag_links_in_tx;
use rustok_content::ContentResult;
use rustok_forum::forum_topic_tag;
use rustok_taxonomy::{TaxonomyOwnerReader, TaxonomyService, TaxonomyTermKind};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder,
};
use uuid::Uuid;

use crate::bridge::helpers::{normalize_locale, taxonomy_error_to_content_error};

pub(crate) async fn load_blog_tag_names_for_post_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ContentResult<Vec<String>> {
    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .order_by_asc(blog_post_tag::Column::CreatedAt)
        .all(txn)
        .await?;
    if relations.is_empty() {
        return Ok(Vec::new());
    }

    let term_ids = relations.iter().map(|item| item.tag_id).collect::<Vec<_>>();
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &term_ids,
        locale,
        fallback_locale,
    )
    .await
    .map_err(taxonomy_error_to_content_error)?;
    let terms_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();

    Ok(relations
        .into_iter()
        .filter_map(|relation| {
            terms_by_id
                .get(&relation.tag_id)
                .map(|term| term.name.clone())
        })
        .collect())
}

pub(crate) async fn load_forum_tag_names_for_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ContentResult<Vec<String>> {
    let relations = forum_topic_tag::Entity::find()
        .filter(forum_topic_tag::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_tag::Column::TopicId.eq(topic_id))
        .order_by_asc(forum_topic_tag::Column::CreatedAt)
        .all(txn)
        .await?;
    if relations.is_empty() {
        return Ok(Vec::new());
    }

    let term_ids = relations
        .iter()
        .map(|item| item.term_id)
        .collect::<Vec<_>>();
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &term_ids,
        locale,
        fallback_locale,
    )
    .await
    .map_err(taxonomy_error_to_content_error)?;
    let terms_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();

    Ok(relations
        .into_iter()
        .filter_map(|relation| {
            terms_by_id
                .get(&relation.term_id)
                .map(|term| term.name.clone())
        })
        .collect())
}

pub(crate) async fn sync_blog_tags_for_post_in_tx(
    taxonomy: &TaxonomyService,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_names: &[String],
    locale: &str,
) -> ContentResult<()> {
    let locale = normalize_locale(locale)?;
    let mut names = tag_names
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();

    let tag_ids = taxonomy
        .ensure_terms_for_module_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            &locale,
            &names,
        )
        .await
        .map_err(taxonomy_error_to_content_error)?;

    // The relation rows belong to Blog, and Blog's `blog_tag_usage`
    // projection is derived from them. Writing the rows here directly (as the
    // bridge used to) left the projection stale, so the tags kept their old
    // counts and the next Blog `update_post` failed on an underflow. The owner
    // command rewrites both in one step.
    replace_post_tag_links_in_tx(txn, tenant_id, post_id, &tag_ids)
        .await
        .map_err(crate::errors::blog_error_to_content_error)?;

    Ok(())
}

pub(crate) async fn sync_forum_tags_for_topic_in_tx(
    taxonomy: &TaxonomyService,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    tag_names: &[String],
    locale: &str,
) -> ContentResult<()> {
    let locale = normalize_locale(locale)?;
    let mut names = tag_names
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();

    forum_topic_tag::Entity::delete_many()
        .filter(forum_topic_tag::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_tag::Column::TopicId.eq(topic_id))
        .exec(txn)
        .await?;

    let term_ids = taxonomy
        .ensure_terms_for_module_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "forum",
            &locale,
            &names,
        )
        .await
        .map_err(taxonomy_error_to_content_error)?;

    for term_id in term_ids {
        forum_topic_tag::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(topic_id),
            term_id: Set(term_id),
            tenant_id: Set(tenant_id),
            created_at: Set(Utc::now().into()),
        }
        .insert(txn)
        .await?;
    }

    Ok(())
}
