use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use rustok_api::{Action, Resource, PLATFORM_FALLBACK_LOCALE};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;

use crate::dto::{
    CategoryBreadcrumb, CategoryTreeNode, CategoryTreeQuery, CategoryTreeResponse,
    MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES,
};
use crate::entities::{forum_category, forum_category_translation};
use crate::error::{ForumError, ForumResult};
use crate::services::category_policy::CategoryTopicPolicyService;
use crate::services::rbac::enforce_scope;
use crate::services::subscription::SubscriptionService;

pub(super) struct CategoryTreeService {
    db: DatabaseConnection,
}

impl CategoryTreeService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(super) async fn read(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryTreeQuery,
    ) -> ForumResult<CategoryTreeResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let requested_locale =
            normalize_locale(query.locale.as_deref().unwrap_or(PLATFORM_FALLBACK_LOCALE))?;
        let fallback_locale = query
            .fallback_locale
            .as_deref()
            .map(normalize_locale)
            .transpose()?;

        let mut categories = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(forum_category::Column::Position)
            .order_by_asc(forum_category::Column::Id)
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
        let translations = forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.is_in(category_ids.clone()))
            .order_by_asc(forum_category_translation::Column::CategoryId)
            .order_by_asc(forum_category_translation::Column::Locale)
            .all(&self.db)
            .await?;
        let mut translations_by_category =
            HashMap::<Uuid, Vec<forum_category_translation::Model>>::new();
        for translation in translations {
            translations_by_category
                .entry(translation.category_id)
                .or_default()
                .push(translation);
        }

        let subscriptions = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &category_ids, security.user_id)
            .await?;
        let topic_policy_flags = CategoryTopicPolicyService::new(self.db.clone())
            .flags_for_categories(tenant_id, &category_ids)
            .await?;

        let total_nodes = categories.len() as u32;
        let mut nodes = HashMap::<Uuid, CategoryTreeNode>::with_capacity(categories.len());
        let mut children_by_parent = HashMap::<Option<Uuid>, Vec<Uuid>>::new();

        for category in categories.drain(..) {
            let localized = translations_by_category
                .remove(&category.id)
                .unwrap_or_default();
            let resolved = resolve_by_locale_with_fallback(
                &localized,
                &requested_locale,
                fallback_locale.as_deref(),
                |translation| translation.locale.as_str(),
            );
            let translation = resolved.item.ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {} has no localized translation",
                    category.id
                ))
            })?;
            let node = CategoryTreeNode {
                id: category.id,
                parent_id: category.parent_id,
                depth: 0,
                position: category.position,
                requested_locale: requested_locale.clone(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&localized, |translation| {
                    translation.locale.as_str()
                }),
                name: translation.name.clone(),
                slug: translation.slug.clone(),
                description: translation.description.clone(),
                icon: category.icon,
                color: category.color,
                moderated: category.moderated,
                allows_topics: topic_policy_flags
                    .get(&category.id)
                    .copied()
                    .unwrap_or(true),
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
                if !nodes.contains_key(&parent_id) {
                    return Err(ForumError::Validation(format!(
                        "Forum category tree contains missing or foreign parent {parent_id} for category {}",
                        node.id
                    )));
                }
            }
        }

        let root_ids = children_by_parent.get(&None).cloned().unwrap_or_default();
        if root_ids.is_empty() {
            return Err(ForumError::Validation(
                "Forum category tree contains no root category".to_string(),
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
                "Forum category tree contains a cycle or disconnected hierarchy".to_string(),
            ));
        }

        Ok(CategoryTreeResponse {
            roots,
            total_nodes,
            max_depth: observed_max_depth as u16,
        })
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
            "Forum category tree contains a hierarchy cycle".to_string(),
        ));
    }
    if !visited.insert(category_id) {
        active_path.remove(&category_id);
        return Err(ForumError::Validation(
            "Forum category tree contains a category more than once".to_string(),
        ));
    }

    let mut node = nodes.get(&category_id).cloned().ok_or_else(|| {
        ForumError::Validation(format!(
            "Forum category tree references missing category {category_id}"
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

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}
