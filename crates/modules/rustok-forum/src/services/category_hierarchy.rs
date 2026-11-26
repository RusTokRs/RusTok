//! Validate provider hierarchy against Forum's tenant-local category membership.
use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::dto::{MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES};
use crate::error::{ForumError, ForumResult};

/// A provider cannot omit an authenticated subtree from the base floor, inject
/// foreign membership, or hide a broken graph behind a local visibility override.
/// Validate the complete bounded graph before any filtering or early policy return.
pub(super) fn validate_category_parent_projection(
    category_ids: &[Uuid],
    parents: &HashMap<Uuid, Option<Uuid>>,
) -> ForumResult<()> {
    let expected = category_ids.iter().copied().collect::<HashSet<_>>();
    if category_ids.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize
        || expected.len() != category_ids.len()
        || parents.len() != expected.len()
        || parents.keys().any(|id| !expected.contains(id))
    {
        return Err(ForumError::Validation(
            "Forum category hierarchy projection must match tenant category membership".into(),
        ));
    }
    for id in category_ids {
        let mut current = Some(*id);
        let mut visited = HashSet::new();
        let mut depth = 0usize;
        while let Some(current_id) = current {
            if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
                return Err(ForumError::Validation(
                    "Forum category hierarchy projection exceeds the maximum depth".into(),
                ));
            }
            if !visited.insert(current_id) {
                return Err(ForumError::Validation(
                    "Forum category hierarchy projection contains a cycle".into(),
                ));
            }
            current = *parents.get(&current_id).ok_or_else(|| {
                ForumError::Validation(
                    "Forum category hierarchy projection references a missing or foreign parent"
                        .into(),
                )
            })?;
            depth += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_empty_and_complete_tenant_hierarchy() {
        assert!(validate_category_parent_projection(&[], &HashMap::new()).is_ok());
        let root = Uuid::new_v4();
        let child = Uuid::new_v4();
        assert!(
            validate_category_parent_projection(
                &[root, child],
                &HashMap::from([(root, None), (child, Some(root))])
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_missing_extra_foreign_and_cyclic_provider_membership() {
        let root = Uuid::new_v4();
        let child = Uuid::new_v4();
        let foreign = Uuid::new_v4();
        for parents in [
            HashMap::from([(root, None)]),
            HashMap::from([(root, None), (child, None), (foreign, None)]),
            HashMap::from([(root, None), (foreign, None)]),
            HashMap::from([(root, None), (child, Some(foreign))]),
            HashMap::from([(root, Some(child)), (child, Some(root))]),
            HashMap::from([(root, None), (child, Some(child))]),
        ] {
            assert!(validate_category_parent_projection(&[root, child], &parents).is_err());
        }
    }

    #[test]
    fn enforces_depth_bound_without_off_by_one() {
        let ids = (0..MAX_FORUM_CATEGORY_TREE_DEPTH + 2)
            .map(|_| Uuid::new_v4())
            .collect::<Vec<_>>();
        let parents = ids
            .iter()
            .enumerate()
            .map(|(index, id)| (*id, index.checked_sub(1).map(|parent| ids[parent])))
            .collect::<HashMap<_, _>>();
        assert!(validate_category_parent_projection(&ids, &parents).is_err());
        let valid_ids = &ids[..ids.len() - 1];
        let valid = parents
            .into_iter()
            .filter(|(id, _)| valid_ids.contains(id))
            .collect();
        assert!(validate_category_parent_projection(valid_ids, &valid).is_ok());
    }
}
