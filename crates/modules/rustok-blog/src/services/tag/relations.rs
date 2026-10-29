use super::*;

async fn bump_posts_for_tag_relation_removal_in_tx(
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

    // Posts with an exhausted/invalid revision are not bumped: they cannot be
    // CAS-edited either, so no concurrency protection is lost, and refusing
    // here would make the tag permanently undeletable because of one such
    // post. The tag delete publishes the reindex event that repairs their
    // search projection. `relation_count` is still observed for diagnostics.
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

async fn publish_blog_reindex_in_tx(
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
    db: &DatabaseConnection,
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
        // Clearing the tag set still rewrites relations and the projection.
        return remove_post_tag_links_in_tx(txn, tenant_id, post_id).await;
    }

    let term_ids = TaxonomyService::new(db.clone())
        .ensure_module_terms_for_owner_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            &normalized_locale,
            &normalized_names,
            allow_create,
        )
        .await?;

    validate_post_tag_terms_in_tx(txn, tenant_id, &term_ids, &normalized_locale).await?;
    replace_post_tag_links_in_tx(txn, tenant_id, post_id, &term_ids).await
}

async fn validate_post_tag_terms_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_ids: &[Uuid],
    locale: &str,
) -> BlogResult<()> {
    if tag_ids.is_empty() {
        return Ok(());
    }

    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
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

    for term in terms {
        if term.scope_type == TaxonomyScopeType::Module
            && term.scope_value.as_deref() == Some(BLOG_SCOPE_VALUE)
        {
            validate_tag_name(&term.name)?;
        }
    }

    Ok(())
}

/// Recomputes the `blog_tag_usage` projection of `tag_ids` from the relation
/// table inside the caller's transaction.
///
/// The projection is derived state, so it is **recomputed**, never adjusted by
/// `+1`/`-1`: the count is read from `blog_post_tags` after the relation write
/// of the same transaction. That makes the write path self-healing — a
/// projection row lost to a crashed transaction, a manual data fix or a writer
/// outside this service (content orchestration) is repaired by the next
/// mutation instead of turning into an underflow error that blocks every
/// subsequent post save.
///
/// Rows are kept at `use_count = 0` for Blog-owned module terms because
/// `list_tags` pages over this table (a tag with no posts must remain
/// listable); projection rows of global terms are removed at zero, exactly as
/// the previous decrement path did.
async fn refresh_tag_usage_in_tx(
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
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &unique_ids,
        PLATFORM_FALLBACK_LOCALE,
        None,
    )
    .await?;
    if terms.len() != unique_ids.len() {
        return Err(BlogError::invariant(
            "Blog tag usage refresh references a missing or wrong-kind Taxonomy term",
        ));
    }
    for term in terms {
        if term.scope_type == TaxonomyScopeType::Module
            && term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
        {
            return Err(BlogError::invariant(format!(
                "Blog post tag {} references a Taxonomy term outside the Blog/module scope",
                term.id
            )));
        }

        let use_count = blog_post_tag::Entity::find()
            .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
            .filter(blog_post_tag::Column::TagId.eq(term.id))
            .count(txn)
            .await?;
        let use_count = i32::try_from(use_count).unwrap_or(i32::MAX);

        if use_count == 0 && term.scope_type == TaxonomyScopeType::Global {
            blog_tag_usage::Entity::delete_many()
                .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
                .filter(blog_tag_usage::Column::TagId.eq(term.id))
                .exec(txn)
                .await?;
            continue;
        }

        blog_tag_usage::Entity::insert(blog_tag_usage::ActiveModel {
            tenant_id: Set(tenant_id),
            tag_id: Set(term.id),
            canonical_key: Set(term.canonical_key.clone()),
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

/// Replaces the whole tag set of a post inside the caller's transaction.
///
/// This is the single write entry point for post-to-tag relations. Blog
/// itself and the content-orchestration bridge both use it, so the relation
/// rows and the `blog_tag_usage` projection can never be written separately.
/// `tag_ids` must already be resolved Taxonomy term ids of the same tenant;
/// duplicates are collapsed.
pub async fn replace_post_tag_links_in_tx(
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
    refresh_tag_usage_in_tx(txn, tenant_id, &affected).await
}

/// Removes every tag link of a post inside the caller's transaction and
/// refreshes the usage projection of the tags it pointed at.
pub async fn remove_post_tag_links_in_tx(
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

    refresh_tag_usage_in_tx(txn, tenant_id, &previous_tag_ids).await
}

pub(crate) async fn initialize_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_id: Uuid,
    canonical_key: &str,
) -> BlogResult<()> {
    // Idempotent by design: a retried create (or a projection row that already
    // exists because the tag was re-created) must not fail on the projection's
    // primary key. A zero-row upsert keeps the existing count untouched.
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
    let names = TaxonomyOwnerReader::new(db.clone())
        .load_term_names_strict_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            &term_ids,
        )
        .await?;
    let mut locale_chain = vec![locale.to_string()];
    if let Some(fallback_locale) = fallback_locale
        && fallback_locale != locale
    {
        locale_chain.push(fallback_locale.to_string());
    }

    for relation in relations {
        let Some(term_names) = names.get(&relation.tag_id) else {
            return Err(BlogError::invariant(format!(
                "Blog post {} references missing Taxonomy tag {}",
                relation.post_id, relation.tag_id
            )));
        };
        tags_by_post
            .entry(relation.post_id)
            .or_default()
            .push(term_names.resolve_name_for_locale_chain(&locale_chain));
    }

    Ok(tags_by_post)
}

pub(crate) async fn resolve_tag_id_for_posts(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    tag: &str,
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<Option<Uuid>> {
    TaxonomyService::new(db.clone())
        .resolve_term_id_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            locale,
            fallback_locale,
            tag,
        )
        .await
        .map_err(BlogError::from)
}
