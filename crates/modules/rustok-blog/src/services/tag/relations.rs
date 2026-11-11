/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use super::*;
use crate::ports::taxonomy::BlogTaxonomyPort;

pub(super) async fn bump_posts_for_tag_relation_removal_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_id: Uuid,
) -> BlogResult<()> {
    use sea_orm::{ExprTrait, QueryTrait};

    let valid_post_ids = blog_post::Entity::find()
        .select_only()
        .column(blog_post::Column::Id)
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Version.gt(0))
        .filter(blog_post::Column::Version.ne(i32::MAX))
        .into_query();

    let relation_filter = blog_post_tag::Entity::find()
        .select_only()
        .column(blog_post_tag::Column::PostId)
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.eq(tag_id))
        .into_query();

    let relation_count = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.eq(tag_id))
        .count(txn)
        .await?;

    let valid_relation_count = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.eq(tag_id))
        .filter(blog_post_tag::Column::PostId.in_subquery(valid_post_ids))
        .count(txn)
        .await?;

    if relation_count != valid_relation_count {
        tracing::warn!(
            tenant_id = %tenant_id,
            tag_id = %tag_id,
            relation_count,
            valid_relation_count,
            "Blog tag removal skips the revision bump for posts without a valid persisted version"
        );
    }

    let now = Utc::now();
    let updated = blog_post::Entity::update_many()
        .col_expr(
            blog_post::Column::Version,
            Expr::col(blog_post::Column::Version).add(1),
        )
        .col_expr(blog_post::Column::UpdatedAt, Expr::value(now))
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Id.in_subquery(relation_filter))
        .filter(blog_post::Column::Version.gt(0))
        .filter(blog_post::Column::Version.ne(i32::MAX))
        .exec(txn)
        .await?;

    if updated.rows_affected as u64 != valid_relation_count {
        return Err(BlogError::conflict(format!(
            "Blog tag {tag_id} invalidated {} post(s), expected {valid_relation_count}",
            updated.rows_affected
        )));
    }

    Ok(())
}

pub(super) async fn publish_blog_reindex_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
) -> BlogResult<()> {
    TransactionalEventBus::publish_root_in_tx(
        txn,
        tenant_id,
        actor_id,
        DomainEvent::ReindexRequested {
            target_type: "blog".to_string(),
            target_id: None,
        },
    )
    .await
    .map_err(BlogError::from)
}

pub(crate) async fn sync_post_tags_in_tx(
    port: &dyn BlogTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_names: &[String],
    locale: &str,
    allow_create: bool,
) -> BlogResult<()> {
    let normalized_locale = normalize_locale(locale)?;
    let normalized_names = normalize_tag_names(tag_names);
    for name in &normalized_names {
        validate_tag_name(name)?;
    }

    if normalized_names.is_empty() {
        return remove_post_tag_links_with_port_in_tx(Some(port), txn, tenant_id, post_id).await;
    }

    let term_ids = port
        .ensure_module_tags_in_tx(
            txn,
            tenant_id,
            &normalized_locale,
            &normalized_names,
            allow_create,
        )
        .await?;

    validate_post_tag_terms_in_tx(port, txn, tenant_id, &term_ids, &normalized_locale).await?;
    replace_post_tag_links_with_port_in_tx(Some(port), txn, tenant_id, post_id, &term_ids).await
}

async fn validate_post_tag_terms_in_tx(
    port: &dyn BlogTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_ids: &[Uuid],
    locale: &str,
) -> BlogResult<()> {
    if tag_ids.is_empty() {
        return Ok(());
    }

    let terms = port
        .load_tags_by_ids(
            txn.into(),
            tenant_id,
            tag_ids,
            locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        )
        .await?;

    if terms.len() != tag_ids.iter().copied().collect::<HashSet<_>>().len() {
        return Err(BlogError::invariant(
            "Blog post tag mutation references missing or duplicate Taxonomy terms",
        ));
    }

    for term in terms.values() {
        if term.scope_type == "module"
            && term.scope_value.as_deref() == Some(BLOG_SCOPE_VALUE)
        {
            validate_tag_name(&term.name)?;
        }
    }

    Ok(())
}

/// Recomputes the `blog_tag_usage` projection of `tag_ids` from the relation
/// table inside the caller's transaction.
pub(super) async fn refresh_tag_usage_in_tx(
    port: Option<&dyn BlogTaxonomyPort>,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_ids: &[Uuid],
) -> BlogResult<()> {
    if tag_ids.is_empty() {
        return Ok(());
    }
    let mut unique_ids = tag_ids.to_vec();
    unique_ids.sort_unstable();
    unique_ids.dedup();

    let terms = if let Some(port) = port {
        port.load_tags_by_ids(
            txn.into(),
            tenant_id,
            &unique_ids,
            PLATFORM_FALLBACK_LOCALE,
            None,
        )
        .await?
    } else {
        HashMap::new()
    };

    if !terms.is_empty() && terms.len() != unique_ids.len() {
        return Err(BlogError::invariant(
            "Blog tag usage refresh references a missing or wrong-kind Taxonomy term",
        ));
    }

    for tag_id in unique_ids {
        let term = terms.get(&tag_id);
        if let Some(term) = term {
            if term.scope_type == "module"
                && term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
            {
                return Err(BlogError::invariant(format!(
                    "Blog post tag {} references a Taxonomy term outside the Blog/module scope",
                    term.id
                )));
            }
        }

        let use_count = blog_post_tag::Entity::find()
            .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
            .filter(blog_post_tag::Column::TagId.eq(tag_id))
            .count(txn)
            .await?;
        let use_count = i32::try_from(use_count).unwrap_or(i32::MAX);

        let is_global = term.map(|t| t.scope_type == "global").unwrap_or(false);
        if use_count == 0 && is_global {
            blog_tag_usage::Entity::delete_many()
                .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
                .filter(blog_tag_usage::Column::TagId.eq(tag_id))
                .exec(txn)
                .await?;
            continue;
        }

        let canonical_key = term
            .map(|t| t.canonical_key.clone())
            .unwrap_or_else(|| tag_id.to_string());

        blog_tag_usage::Entity::insert(blog_tag_usage::ActiveModel {
            tenant_id: Set(tenant_id),
            tag_id: Set(tag_id),
            canonical_key: Set(canonical_key),
            use_count: Set(use_count),
        })
        .on_conflict(
            OnConflict::columns([
                blog_tag_usage::Column::TenantId,
                blog_tag_usage::Column::TagId,
            ])
            .update_columns([
                blog_tag_usage::Column::UseCount,
                blog_tag_usage::Column::CanonicalKey,
            ])
            .to_owned(),
        )
        .exec(txn)
        .await
        .map_err(BlogError::from)?;
    }
    Ok(())
}

/// Replaces the whole tag set of a post inside the caller's transaction with optional taxonomy port.
pub async fn replace_post_tag_links_with_port_in_tx(
    port: Option<&dyn BlogTaxonomyPort>,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_ids: &[Uuid],
) -> BlogResult<()> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("Tenant ID must not be nil"));
    }
    if post_id.is_nil() {
        return Err(BlogError::validation("Post ID must not be nil"));
    }

    let previous_relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(txn)
        .await?;
    let previous_tag_ids = previous_relations
        .into_iter()
        .map(|relation| relation.tag_id)
        .collect::<Vec<_>>();

    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .exec(txn)
        .await?;

    let mut unique_tag_ids = tag_ids.to_vec();
    unique_tag_ids.sort_unstable();
    unique_tag_ids.dedup();
    let now = Utc::now();
    for tag_id in &unique_tag_ids {
        blog_post_tag::ActiveModel {
            post_id: Set(post_id),
            tag_id: Set(*tag_id),
            tenant_id: Set(tenant_id),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;
    }

    let mut affected = previous_tag_ids;
    affected.extend(unique_tag_ids);
    refresh_tag_usage_in_tx(port, txn, tenant_id, &affected).await
}

pub async fn replace_post_tag_links_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_ids: &[Uuid],
) -> BlogResult<()> {
    replace_post_tag_links_with_port_in_tx(None, txn, tenant_id, post_id, tag_ids).await
}

/// Removes every tag link of a post inside the caller's transaction and
/// refreshes the usage projection of the tags it pointed at.
pub async fn remove_post_tag_links_with_port_in_tx(
    port: Option<&dyn BlogTaxonomyPort>,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<()> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("Tenant ID must not be nil"));
    }
    if post_id.is_nil() {
        return Err(BlogError::validation("Post ID must not be nil"));
    }

    let previous_relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(txn)
        .await?;
    let previous_tag_ids = previous_relations
        .into_iter()
        .map(|relation| relation.tag_id)
        .collect::<Vec<_>>();

    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .exec(txn)
        .await?;

    refresh_tag_usage_in_tx(port, txn, tenant_id, &previous_tag_ids).await
}

pub async fn remove_post_tag_links_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<()> {
    remove_post_tag_links_with_port_in_tx(None, txn, tenant_id, post_id).await
}

pub(crate) async fn initialize_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_id: Uuid,
    canonical_key: &str,
) -> BlogResult<()> {
    blog_tag_usage::Entity::insert(blog_tag_usage::ActiveModel {
        tenant_id: Set(tenant_id),
        tag_id: Set(tag_id),
        canonical_key: Set(canonical_key.to_string()),
        use_count: Set(0),
    })
    .on_conflict(
        OnConflict::columns([
            blog_tag_usage::Column::TenantId,
            blog_tag_usage::Column::TagId,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec(txn)
    .await
    .map_err(BlogError::from)?;
    Ok(())
}

pub(crate) async fn load_post_tags_map(
    port: &dyn BlogTaxonomyPort,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_ids: &[Uuid],
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<HashMap<Uuid, Vec<String>>> {
    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut tags_by_post = post_ids
        .iter()
        .copied()
        .map(|post_id| (post_id, Vec::new()))
        .collect::<HashMap<_, _>>();

    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.is_in(post_ids.to_vec()))
        .order_by_asc(blog_post_tag::Column::CreatedAt)
        .all(db)
        .await?;

    if relations.is_empty() {
        return Ok(tags_by_post);
    }

    let term_ids = relations.iter().map(|item| item.tag_id).collect::<Vec<_>>();
    let names = port
        .load_tags_by_ids(
            db.into(),
            tenant_id,
            &term_ids,
            locale,
            fallback_locale,
        )
        .await?;

    for relation in relations {
        let Some(term) = names.get(&relation.tag_id) else {
            return Err(BlogError::invariant(format!(
                "Blog post {} references missing Taxonomy tag {}",
                relation.post_id, relation.tag_id
            )));
        };
        tags_by_post
            .entry(relation.post_id)
            .or_default()
            .push(term.name.clone());
    }

    Ok(tags_by_post)
}

pub(crate) async fn resolve_tag_id_for_posts(
    port: &dyn BlogTaxonomyPort,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    tag: &str,
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<Option<Uuid>> {
    port.resolve_tag_id_for_module(
        db.into(),
        tenant_id,
        tag,
        locale,
        fallback_locale,
    )
    .await
}
