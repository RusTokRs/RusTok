async fn lock_category_tree_in_tx(txn: &DatabaseTransaction, tenant_id: Uuid) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [tenant_id.to_string().into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum category lifecycle does not support {backend:?}"
        ))),
    }
}

async fn load_categories_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<Vec<forum_category::Model>> {
    let categories = forum_category::Entity::find()
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(forum_category::Column::Id)
        .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
        .all(txn)
        .await?;
    if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
        return Err(ForumError::Validation(format!(
            "Forum category tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
        )));
    }
    Ok(categories)
}

async fn load_category_parents_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, Option<Uuid>>> {
    let hierarchy_rows = rustok_taxonomy::entities::taxonomy_category_hierarchy::Entity::find()
        .filter(rustok_taxonomy::entities::taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(rustok_taxonomy::entities::taxonomy_category_hierarchy::Column::TermId.is_in(category_ids.iter().copied()))
        .all(txn)
        .await?;
    let mut parent_by_id = hierarchy_rows
        .into_iter()
        .map(|row| (row.term_id, row.parent_term_id))
        .collect::<HashMap<_, _>>();
    for id in category_ids {
        parent_by_id.entry(*id).or_insert(None);
    }
    Ok(parent_by_id)
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
