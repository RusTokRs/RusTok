use std::collections::{HashMap, HashSet};

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_content::normalize_locale_code;
use rustok_taxonomy::{
    TaxonomyError, TaxonomyOwnerCategory, TaxonomyOwnerCategoryReader, TaxonomyScopeType,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

use crate::dto::{
    CategoryBreadcrumb, CategoryTreeNode, CategoryTreeQuery, CategoryTreeResponse,
    MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES,
};
use crate::entities::{
    forum_category, forum_category_lifecycle, forum_category_taxonomy_binding,
};
use crate::error::{ForumError, ForumResult};
use crate::services::category_policy::CategoryTopicPolicyService;
use crate::services::subscription::SubscriptionService;

struct BoundTaxonomyCategory {
    requested_locale: String,
    effective_locale: String,
    available_locales: Vec<String>,
    name: String,
    slug: String,
    description: Option<String>,
    parent_id: Option<Uuid>,
    position: i32,
    icon: Option<String>,
    color: Option<String>,
}

/// CAT-5 tree adapter: Taxonomy owns canonical copy, hierarchy and presentation;
/// Forum retains lifecycle, policy, counters and viewer-specific state.
pub(super) struct CategoryTaxonomyTreeReadService {
    db: DatabaseConnection,
}

impl CategoryTaxonomyTreeReadService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(super) async fn read_with_hidden_categories(
        &self,
        tenant_id: Uuid,
        query: CategoryTreeQuery,
        hidden_category_ids: &[Uuid],
        user_id: Option<Uuid>,
    ) -> ForumResult<CategoryTreeResponse> {
        let requested_locale = normalize_locale(
            query
                .locale
                .as_deref()
                .unwrap_or(PLATFORM_FALLBACK_LOCALE),
        )?;
        let fallback_locale = query
            .fallback_locale
            .as_deref()
            .map(normalize_locale)
            .transpose()?;

        let mut categories = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
            .all(&self.db)
            .await?;
        if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum category tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
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
        let canonical = self
            .load_bound_categories(
                tenant_id,
                &category_ids,
                &requested_locale,
                fallback_locale.as_deref(),
            )
            .await?;

        categories.sort_by_key(|category| {
            let projected = canonical
                .get(&category.id)
                .expect("canonical projection cardinality checked before tree sort");
            (projected.parent_id, projected.position, category.id)
        });

        let subscriptions = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &category_ids, user_id)
            .await?;
        let topic_policy_flags = CategoryTopicPolicyService::new(self.db.clone())
            .flags_for_categories(tenant_id, &category_ids)
            .await?;
        let lifecycle_by_category = forum_category_lifecycle::Entity::find()
            .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
            .filter(forum_category_lifecycle::Column::CategoryId.is_in(category_ids.clone()))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|lifecycle| (lifecycle.category_id, lifecycle))
            .collect::<HashMap<_, _>>();

        let mut nodes = HashMap::<Uuid, CategoryTreeNode>::with_capacity(categories.len());
        let mut children_by_parent = HashMap::<Option<Uuid>, Vec<Uuid>>::new();
        for category in categories {
            let projected = canonical.get(&category.id).ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {} has no canonical Taxonomy Category projection",
                    category.id
                ))
            })?;
            let lifecycle = lifecycle_by_category.get(&category.id);
            let node = CategoryTreeNode {
                id: category.id,
                parent_id: projected.parent_id,
                depth: 0,
                position: projected.position,
                requested_locale: projected.requested_locale.clone(),
                effective_locale: projected.effective_locale.clone(),
                available_locales: projected.available_locales.clone(),
                name: projected.name.clone(),
                slug: projected.slug.clone(),
                description: projected.description.clone(),
                icon: projected.icon.clone(),
                color: projected.color.clone(),
                moderated: category.moderated,
                allows_topics: topic_policy_flags
                    .get(&category.id)
                    .copied()
                    .unwrap_or(true)
                    && lifecycle.is_none(),
                archived_at: lifecycle.map(|value| value.archived_at.to_rfc3339()),
                is_archived: lifecycle.is_some(),
                topic_count: category.topic_count,
                reply_count: category.reply_count,
                is_subscribed: subscriptions.get(&category.id).copied().unwrap_or(false),
                has_children: false,
                children_count: 0,
                breadcrumbs: Vec::new(),
                children: Vec::new(),
            };
            children_by_parent
                .entry(node.parent_id)
                .or_default()
                .push(node.id);
            nodes.insert(node.id, node);
        }

        for node in nodes.values() {
            if let Some(parent_id) = node.parent_id {
                let parent = nodes.get(&parent_id).ok_or_else(|| {
                    ForumError::Validation(format!(
                        "Taxonomy-backed Forum category tree references missing bound parent {parent_id} for category {}",
                        node.id
                    ))
                })?;
                if parent.is_archived && !node.is_archived {
                    return Err(ForumError::Validation(
                        "Forum category tree contains an active child beneath an archived parent"
                            .to_string(),
                    ));
                }
            }
        }

        let root_ids = children_by_parent.get(&None).cloned().unwrap_or_default();
        if root_ids.is_empty() {
            return Err(ForumError::Validation(
                "Taxonomy-backed Forum category tree contains no root category".to_string(),
            ));
        }

        let mut visited = HashSet::with_capacity(nodes.len());
        let mut active_path = HashSet::new();
        let mut observed_max_depth = 0usize;
        let mut roots = Vec::with_capacity(root_ids.len());
        for root_id in root_ids {
            roots.push(build_node(
                root_id,
                0,
                &nodes,
                &children_by_parent,
                &[],
                &mut active_path,
                &mut visited,
                &mut observed_max_depth,
            )?);
        }
        if visited.len() != nodes.len() {
            return Err(ForumError::Validation(
                "Taxonomy-backed Forum category tree contains a cycle or disconnected hierarchy"
                    .to_string(),
            ));
        }

        let hidden = hidden_category_ids.iter().copied().collect::<HashSet<_>>();
        let (total_nodes, max_depth) = retain_visible_nodes(&mut roots, &hidden);
        Ok(CategoryTreeResponse {
            roots,
            total_nodes,
            max_depth,
        })
    }

    async fn load_bound_categories(
        &self,
        tenant_id: Uuid,
        forum_category_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<HashMap<Uuid, BoundTaxonomyCategory>> {
        let unique_forum_ids = forum_category_ids.iter().copied().collect::<HashSet<_>>();
        let bindings = forum_category_taxonomy_binding::Entity::find()
            .filter(forum_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
            .filter(
                forum_category_taxonomy_binding::Column::ForumCategoryId
                    .is_in(unique_forum_ids.iter().copied()),
            )
            .all(&self.db)
            .await?;
        if bindings.len() != unique_forum_ids.len() {
            return Err(ForumError::Validation(
                "Forum category tree cutover requires a complete same-tenant Taxonomy Category binding set"
                    .to_string(),
            ));
        }

        let taxonomy_ids = bindings
            .iter()
            .map(|binding| binding.taxonomy_category_id)
            .collect::<Vec<_>>();
        let projected = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some("forum"),
                Some(&taxonomy_ids),
                locale,
                fallback_locale,
            )
            .await
            .map_err(map_taxonomy_read_error)?;
        if projected.len() != taxonomy_ids.len() {
            return Err(ForumError::Validation(
                "Forum category tree cutover found an incomplete Taxonomy Category owner projection"
                    .to_string(),
            ));
        }

        let projected_by_taxonomy_id = projected
            .into_iter()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        let taxonomy_to_forum = bindings
            .iter()
            .map(|binding| (binding.taxonomy_category_id, binding.forum_category_id))
            .collect::<HashMap<_, _>>();

        let mut result = HashMap::with_capacity(bindings.len());
        for binding in bindings {
            let category = projected_by_taxonomy_id
                .get(&binding.taxonomy_category_id)
                .ok_or_else(|| {
                    ForumError::Validation(format!(
                        "Forum category {} is bound to a missing Taxonomy Category owner projection",
                        binding.forum_category_id
                    ))
                })?;
            let parent_id = category
                .parent_id
                .map(|parent_taxonomy_id| {
                    taxonomy_to_forum
                        .get(&parent_taxonomy_id)
                        .copied()
                        .ok_or_else(|| {
                            ForumError::Validation(format!(
                                "Taxonomy Category {} references parent {parent_taxonomy_id} without a Forum binding",
                                category.id
                            ))
                        })
                })
                .transpose()?;
            result.insert(
                binding.forum_category_id,
                bind_owner_projection(category, parent_id),
            );
        }
        Ok(result)
    }
}

fn bind_owner_projection(
    category: &TaxonomyOwnerCategory,
    parent_id: Option<Uuid>,
) -> BoundTaxonomyCategory {
    BoundTaxonomyCategory {
        requested_locale: category.requested_locale.clone(),
        effective_locale: category.effective_locale.clone(),
        available_locales: category.available_locales.clone(),
        name: category.name.clone(),
        slug: category.slug.clone(),
        description: category.description.clone(),
        parent_id,
        position: category.position,
        icon: category.icon_key.clone(),
        color: category.color.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_node(
    category_id: Uuid,
    depth: usize,
    nodes: &HashMap<Uuid, CategoryTreeNode>,
    children_by_parent: &HashMap<Option<Uuid>, Vec<Uuid>>,
    parent_breadcrumbs: &[CategoryBreadcrumb],
    active_path: &mut HashSet<Uuid>,
    visited: &mut HashSet<Uuid>,
    observed_max_depth: &mut usize,
) -> ForumResult<CategoryTreeNode> {
    if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
        return Err(ForumError::Validation(format!(
            "Forum category tree exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
        )));
    }
    if !active_path.insert(category_id) {
        return Err(ForumError::Validation(
            "Taxonomy-backed Forum category tree contains a hierarchy cycle".to_string(),
        ));
    }
    if !visited.insert(category_id) {
        active_path.remove(&category_id);
        return Err(ForumError::Validation(
            "Taxonomy-backed Forum category tree contains a category more than once".to_string(),
        ));
    }

    let mut node = nodes.get(&category_id).cloned().ok_or_else(|| {
        ForumError::Validation(format!(
            "Taxonomy-backed Forum category tree references missing category {category_id}"
        ))
    })?;
    node.depth = depth as u16;
    *observed_max_depth = (*observed_max_depth).max(depth);

    let mut breadcrumbs = parent_breadcrumbs.to_vec();
    breadcrumbs.push(CategoryBreadcrumb {
        id: node.id,
        name: node.name.clone(),
        slug: node.slug.clone(),
    });
    node.breadcrumbs = breadcrumbs.clone();

    let child_ids = children_by_parent
        .get(&Some(category_id))
        .cloned()
        .unwrap_or_default();
    node.children_count = child_ids.len() as u32;
    node.has_children = !child_ids.is_empty();
    node.children = child_ids
        .into_iter()
        .map(|child_id| {
            build_node(
                child_id,
                depth + 1,
                nodes,
                children_by_parent,
                &breadcrumbs,
                active_path,
                visited,
                observed_max_depth,
            )
        })
        .collect::<ForumResult<Vec<_>>>()?;

    active_path.remove(&category_id);
    Ok(node)
}

fn retain_visible_nodes(
    nodes: &mut Vec<CategoryTreeNode>,
    hidden: &HashSet<Uuid>,
) -> (u32, u16) {
    nodes.retain(|node| !hidden.contains(&node.id));

    let mut total_nodes = 0u32;
    let mut max_depth = 0u16;
    for node in nodes {
        let (child_total, child_max_depth) = retain_visible_nodes(&mut node.children, hidden);
        node.children_count = node.children.len() as u32;
        node.has_children = !node.children.is_empty();
        total_nodes = total_nodes.saturating_add(1).saturating_add(child_total);
        max_depth = max_depth.max(node.depth).max(child_max_depth);
    }
    (total_nodes, max_depth)
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn map_taxonomy_read_error(error: TaxonomyError) -> ForumError {
    match error {
        TaxonomyError::Database(error) => ForumError::Database(error),
        other => ForumError::Validation(format!(
            "Forum Taxonomy category tree projection failed: {other}"
        )),
    }
}
