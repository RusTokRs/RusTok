/// Verify that a category and every ancestor are active.
///
/// Structural topic commands and restore use the same category-tree lifecycle invariant:
/// a child of an archived ancestor is not an active placement target even when its own
/// lifecycle row is absent.
pub(super) async fn ensure_category_tree_target_is_active_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    let categories = load_categories_in_tx(txn, tenant_id).await?;
    let category_ids = categories.iter().map(|category| category.id).collect::<Vec<_>>();
    let parent_by_id = load_category_parents_in_tx(txn, tenant_id, &category_ids).await?;
    validate_parent_map(&parent_by_id)?;

    let lifecycle_rows = forum_category_lifecycle::Entity::find()
        .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?;
    let lifecycle_by_category = lifecycle_rows
        .into_iter()
        .map(|lifecycle| (lifecycle.category_id, lifecycle))
        .collect::<HashMap<_, _>>();

    if !parent_by_id.contains_key(&category_id) || lifecycle_by_category.contains_key(&category_id) {
        return Err(ForumError::Validation(
            "Forum content cannot be placed in an archived category".to_string(),
        ));
    }

    ensure_restore_ancestors_are_active(&parent_by_id, &lifecycle_by_category, category_id)
}

async fn load_category_parents_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, Option<Uuid>>> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let projections = rustok_taxonomy::TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict(
        txn,
        tenant_id,
        rustok_taxonomy::TaxonomyScopeType::Module,
        Some("forum"),
        Some(category_ids),
        "en",
        None,
    )
    .await
    .map_err(|error| ForumError::Validation(format!(
        "Forum Category Taxonomy hierarchy read failed: {error}"
    )))?;
    if projections.len() != category_ids.len() {
        return Err(ForumError::Validation(
            "Forum Category Taxonomy hierarchy coverage is incomplete".to_string(),
        ));
    }
    Ok(projections
        .into_iter()
        .map(|category| (category.id, category.parent_id))
        .collect())
}

fn collect_subtree_ids(
    parent_by_id: &HashMap<Uuid, Option<Uuid>>,
    root_id: Uuid,
) -> ForumResult<Vec<Uuid>> {
    let mut children_by_parent = HashMap::<Uuid, Vec<Uuid>>::new();
    for (category_id, parent_id) in parent_by_id {
        if let Some(parent_id) = parent_id {
            children_by_parent
                .entry(*parent_id)
                .or_default()
                .push(*category_id);
        }
    }
    for children in children_by_parent.values_mut() {
        children.sort();
    }

    let mut result = Vec::new();
    let mut stack = vec![root_id];
    let mut visited = HashSet::new();
    while let Some(category_id) = stack.pop() {
        if !visited.insert(category_id) {
            return Err(ForumError::Validation(
                "Forum category hierarchy cycle".to_string(),
            ));
        }
        result.push(category_id);
        if result.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum category subtree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }
        if let Some(children) = children_by_parent.get(&category_id) {
            stack.extend(children.iter().rev().copied());
        }
    }
    Ok(result)
}

fn ensure_restore_ancestors_are_active(
    parent_by_id: &HashMap<Uuid, Option<Uuid>>,
    lifecycle_by_category: &HashMap<Uuid, forum_category_lifecycle::Model>,
    root_id: Uuid,
) -> ForumResult<()> {
    let mut parent_id = parent_by_id.get(&root_id).copied().flatten();
    while let Some(current_id) = parent_id {
        if !parent_by_id.contains_key(&current_id) {
            return Err(ForumError::Validation(format!(
                "Forum category tree references missing or foreign parent {current_id}"
            )));
        }
        if lifecycle_by_category.contains_key(&current_id) {
            return Err(ForumError::Validation(
                "Category subtree cannot be restored beneath an archived ancestor".to_string(),
            ));
        }
        parent_id = parent_by_id.get(&current_id).copied().flatten();
    }
    Ok(())
}

fn validate_parent_map(parent_by_id: &HashMap<Uuid, Option<Uuid>>) -> ForumResult<()> {
    for category_id in parent_by_id.keys().copied() {
        let mut current_id = category_id;
        let mut depth = 0usize;
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current_id) {
                return Err(ForumError::Validation(
                    "Forum category hierarchy cycle".to_string(),
                ));
            }
            let parent_id = parent_by_id.get(&current_id).ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category tree references missing category {current_id}"
                ))
            })?;
            let Some(parent_id) = *parent_id else {
                break;
            };
            if !parent_by_id.contains_key(&parent_id) {
                return Err(ForumError::Validation(format!(
                    "Forum category tree references missing or foreign parent {parent_id}"
                )));
            }
            depth += 1;
            if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
                return Err(ForumError::Validation(format!(
                    "Forum category tree exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
                )));
            }
            current_id = parent_id;
        }
    }
    Ok(())
}
