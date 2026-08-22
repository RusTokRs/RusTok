use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource, TenantLocale};
use rustok_content::{available_locales_from, resolve_by_locale};
use rustok_core::SecurityContext;

use crate::dto::{CategoryTreeNode, CategoryTreeResponse, MAX_BLOG_CATEGORY_TREE_NODES};
use crate::entities::{blog_category, blog_category_translation};
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::enforce_scope;

pub struct CategoryTreeService {
    db: DatabaseConnection,
}

#[derive(Debug, Clone)]
struct FlatCategoryNode {
    node: CategoryTreeNode,
    stored_depth: i32,
}

impl CategoryTreeService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn read(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: Option<&str>,
    ) -> BlogResult<CategoryTreeResponse> {
        enforce_scope(&security, Resource::BlogCategories, Action::List)?;
        let requested_locale = normalize_locale(locale.unwrap_or(PLATFORM_FALLBACK_LOCALE))?;

        let categories = blog_category::Entity::find()
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(blog_category::Column::Position)
            .order_by_asc(blog_category::Column::Id)
            .limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)
            .all(&self.db)
            .await?;
        if categories.len() > MAX_BLOG_CATEGORY_TREE_NODES as usize {
            return Err(BlogError::validation(format!(
                "Blog category tree exceeds the bounded limit of {MAX_BLOG_CATEGORY_TREE_NODES} nodes"
            )));
        }
        if categories.is_empty() {
            return Ok(CategoryTreeResponse {
                roots: Vec::new(),
                total_nodes: 0,
                max_depth: 0,
            });
        }

        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        let translations = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .filter(blog_category_translation::Column::CategoryId.is_in(category_ids))
            .order_by_asc(blog_category_translation::Column::CategoryId)
            .order_by_asc(blog_category_translation::Column::Locale)
            .all(&self.db)
            .await?;
        let mut translations_by_category =
            HashMap::<Uuid, Vec<blog_category_translation::Model>>::new();
        for translation in translations {
            translations_by_category
                .entry(translation.category_id)
                .or_default()
                .push(translation);
        }

        let total_nodes = u32::try_from(categories.len())
            .map_err(|_| BlogError::validation("Blog category tree node count exceeds u32 range"))?;
        let mut nodes = HashMap::<Uuid, FlatCategoryNode>::with_capacity(categories.len());
        let mut children_by_parent = HashMap::<Option<Uuid>, Vec<Uuid>>::new();

        for category in categories {
            if category.depth < 0 {
                return Err(BlogError::validation(format!(
                    "Blog category {} has negative materialized depth {}",
                    category.id, category.depth
                )));
            }
            let localized = translations_by_category
                .remove(&category.id)
                .unwrap_or_default();
            let resolved = resolve_by_locale(&localized, &requested_locale, |translation| {
                translation.locale.as_str()
            });
            let translation = resolved.item;
            let node = CategoryTreeNode {
                id: category.id,
                tenant_id: category.tenant_id,
                requested_locale: requested_locale.clone(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&localized, |translation| {
                    translation.locale.as_str()
                }),
                name: translation
                    .map(|translation| translation.name.clone())
                    .unwrap_or_default(),
                slug: translation
                    .map(|translation| translation.slug.clone())
                    .unwrap_or_default(),
                description: translation.and_then(|translation| translation.description.clone()),
                parent_id: category.parent_id,
                position: category.position,
                depth: category.depth,
                settings: category.settings,
                children: Vec::new(),
            };
            children_by_parent
                .entry(node.parent_id)
                .or_default()
                .push(node.id);
            nodes.insert(
                node.id,
                FlatCategoryNode {
                    stored_depth: node.depth,
                    node,
                },
            );
        }

        for flat in nodes.values() {
            if let Some(parent_id) = flat.node.parent_id
                && !nodes.contains_key(&parent_id)
            {
                return Err(BlogError::validation(format!(
                    "Blog category {} references missing or foreign parent {parent_id}",
                    flat.node.id
                )));
            }
        }

        let root_ids = children_by_parent.get(&None).cloned().unwrap_or_default();
        if root_ids.is_empty() {
            return Err(BlogError::validation(
                "Blog category tree contains no root category",
            ));
        }

        let mut visited = HashSet::with_capacity(nodes.len());
        let mut active_path = HashSet::new();
        let mut max_depth = 0i32;
        let mut roots = Vec::with_capacity(root_ids.len());
        for root_id in root_ids {
            roots.push(build_node(
                root_id,
                0,
                &nodes,
                &children_by_parent,
                &mut active_path,
                &mut visited,
                &mut max_depth,
            )?);
        }
        if visited.len() != nodes.len() {
            return Err(BlogError::validation(
                "Blog category tree contains a cycle or disconnected hierarchy",
            ));
        }

        Ok(CategoryTreeResponse {
            roots,
            total_nodes,
            max_depth,
        })
    }
}

fn build_node(
    category_id: Uuid,
    depth: i32,
    nodes: &HashMap<Uuid, FlatCategoryNode>,
    children_by_parent: &HashMap<Option<Uuid>, Vec<Uuid>>,
    active_path: &mut HashSet<Uuid>,
    visited: &mut HashSet<Uuid>,
    max_depth: &mut i32,
) -> BlogResult<CategoryTreeNode> {
    if !active_path.insert(category_id) {
        return Err(BlogError::validation("Blog category tree contains a hierarchy cycle"));
    }
    if !visited.insert(category_id) {
        active_path.remove(&category_id);
        return Err(BlogError::validation(
            "Blog category tree contains a category more than once",
        ));
    }

    let flat = nodes
        .get(&category_id)
        .ok_or_else(|| BlogError::category_not_found(category_id))?;
    if flat.stored_depth != depth {
        active_path.remove(&category_id);
        return Err(BlogError::validation(format!(
            "Blog category {} materialized depth {} does not match hierarchy depth {depth}",
            category_id, flat.stored_depth
        )));
    }
    *max_depth = (*max_depth).max(depth);

    let child_depth = depth
        .checked_add(1)
        .ok_or_else(|| BlogError::validation("Blog category hierarchy depth is exhausted"))?;
    let mut node = flat.node.clone();
    node.children = children_by_parent
        .get(&Some(category_id))
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|child_id| {
            build_node(
                child_id,
                child_depth,
                nodes,
                children_by_parent,
                active_path,
                visited,
                max_depth,
            )
        })
        .collect::<BlogResult<Vec<_>>>()?;

    active_path.remove(&category_id);
    Ok(node)
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|_| BlogError::validation("Invalid locale"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: u128, parent_id: Option<u128>, depth: i32) -> FlatCategoryNode {
        let id = Uuid::from_u128(id);
        FlatCategoryNode {
            stored_depth: depth,
            node: CategoryTreeNode {
                id,
                tenant_id: Uuid::from_u128(100),
                requested_locale: "en".to_string(),
                effective_locale: "en".to_string(),
                available_locales: vec!["en".to_string()],
                name: id.to_string(),
                slug: id.to_string(),
                description: None,
                parent_id: parent_id.map(Uuid::from_u128),
                position: 0,
                depth,
                settings: json!({}),
                children: Vec::new(),
            },
        }
    }

    #[test]
    fn tree_builder_rejects_materialized_depth_drift() {
        let root = Uuid::from_u128(1);
        let child = Uuid::from_u128(2);
        let nodes = HashMap::from([(root, node(1, None, 0)), (child, node(2, Some(1), 7))]);
        let children = HashMap::from([(None, vec![root]), (Some(root), vec![child])]);
        let mut active = HashSet::new();
        let mut visited = HashSet::new();
        let mut max_depth = 0;

        let error = build_node(
            root,
            0,
            &nodes,
            &children,
            &mut active,
            &mut visited,
            &mut max_depth,
        )
        .expect_err("stored depth drift must fail closed");
        assert!(error.to_string().contains("materialized depth"));
    }

    #[test]
    fn tree_builder_preserves_sibling_order_and_computes_max_depth() {
        let root = Uuid::from_u128(1);
        let first = Uuid::from_u128(2);
        let second = Uuid::from_u128(3);
        let grandchild = Uuid::from_u128(4);
        let nodes = HashMap::from([
            (root, node(1, None, 0)),
            (first, node(2, Some(1), 1)),
            (second, node(3, Some(1), 1)),
            (grandchild, node(4, Some(2), 2)),
        ]);
        let children = HashMap::from([
            (None, vec![root]),
            (Some(root), vec![first, second]),
            (Some(first), vec![grandchild]),
        ]);
        let mut active = HashSet::new();
        let mut visited = HashSet::new();
        let mut max_depth = 0;

        let tree = build_node(
            root,
            0,
            &nodes,
            &children,
            &mut active,
            &mut visited,
            &mut max_depth,
        )
        .expect("valid tree");
        assert_eq!(tree.children[0].id, first);
        assert_eq!(tree.children[1].id, second);
        assert_eq!(tree.children[0].children[0].id, grandchild);
        assert_eq!(max_depth, 2);
        assert_eq!(visited.len(), 4);
    }
}
