use std::collections::HashMap;

use rustok_ui_core::normalize_css_hex_color;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

fn deserialize_optional_css_hex_color<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.and_then(|value| normalize_css_hex_color(value.as_str())))
}

fn default_allows_topics() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CategoryDetail {
    pub id: String,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_css_hex_color")]
    pub color: Option<String>,
    pub parent_id: Option<String>,
    pub position: i32,
    pub topic_count: i32,
    pub reply_count: i32,
    pub moderated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CategoryListItem {
    pub id: String,
    pub locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_css_hex_color")]
    pub color: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub depth: u16,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub moderated: bool,
    #[serde(default = "default_allows_topics")]
    pub allows_topics: bool,
    #[serde(default)]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub is_archived: bool,
    pub topic_count: i32,
    pub reply_count: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CategoryTreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub depth: u16,
    pub position: i32,
    pub requested_locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_css_hex_color")]
    pub color: Option<String>,
    pub moderated: bool,
    #[serde(default = "default_allows_topics")]
    pub allows_topics: bool,
    pub archived_at: Option<String>,
    pub is_archived: bool,
    pub topic_count: i32,
    pub reply_count: i32,
    pub children: Vec<CategoryTreeNode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CategoryTreeResponse {
    pub roots: Vec<CategoryTreeNode>,
    pub total_nodes: u32,
    pub max_depth: u16,
}

impl CategoryTreeResponse {
    pub fn into_flat_items(self) -> Vec<CategoryListItem> {
        let mut items = Vec::with_capacity(self.total_nodes as usize);
        flatten_category_nodes(self.roots, &mut items);
        items
    }
}

fn flatten_category_nodes(nodes: Vec<CategoryTreeNode>, output: &mut Vec<CategoryListItem>) {
    for node in nodes {
        let children = node.children;
        output.push(CategoryListItem {
            id: node.id,
            locale: node.requested_locale,
            effective_locale: node.effective_locale,
            name: node.name,
            slug: node.slug,
            description: node.description,
            icon: node.icon,
            color: node.color,
            parent_id: node.parent_id,
            depth: node.depth,
            position: node.position,
            moderated: node.moderated,
            allows_topics: node.allows_topics,
            archived_at: node.archived_at,
            is_archived: node.is_archived,
            topic_count: node.topic_count,
            reply_count: node.reply_count,
        });
        flatten_category_nodes(children, output);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CategoryDropPlacement {
    Before,
    Inside,
    RootEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryMoveRequest {
    pub category_id: String,
    pub parent_id: Option<String>,
    pub position: u32,
}

pub fn category_drop_move_request(
    items: &[CategoryListItem],
    dragged_id: &str,
    target_id: Option<&str>,
    placement: CategoryDropPlacement,
) -> Result<Option<CategoryMoveRequest>, String> {
    let by_id = items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let dragged = by_id
        .get(dragged_id)
        .copied()
        .ok_or_else(|| format!("Dragged category not found: {dragged_id}"))?;

    let (parent_id, mut position) = match placement {
        CategoryDropPlacement::RootEnd => {
            let roots_after_move = items
                .iter()
                .filter(|item| item.parent_id.is_none() && item.id != dragged_id)
                .count();
            (None, u32::try_from(roots_after_move).map_err(|_| "Too many root categories")?)
        }
        CategoryDropPlacement::Before => {
            let target_id = target_id.ok_or_else(|| "Drop target is required".to_string())?;
            let target = by_id
                .get(target_id)
                .copied()
                .ok_or_else(|| format!("Drop target category not found: {target_id}"))?;
            if target.id == dragged.id {
                return Ok(None);
            }
            let mut target_position = u32::try_from(target.position)
                .map_err(|_| "Category position must be zero or greater".to_string())?;
            if dragged.parent_id == target.parent_id && dragged.position < target.position {
                target_position = target_position.saturating_sub(1);
            }
            (target.parent_id.clone(), target_position)
        }
        CategoryDropPlacement::Inside => {
            let target_id = target_id.ok_or_else(|| "Drop target is required".to_string())?;
            let target = by_id
                .get(target_id)
                .copied()
                .ok_or_else(|| format!("Drop target category not found: {target_id}"))?;
            if target.id == dragged.id {
                return Err("A category cannot be nested inside itself".to_string());
            }
            if target.is_archived && !dragged.is_archived {
                return Err("An active category cannot be moved beneath an archived category".to_string());
            }
            let child_count_after_move = items
                .iter()
                .filter(|item| item.parent_id.as_deref() == Some(target.id.as_str()) && item.id != dragged_id)
                .count();
            (
                Some(target.id.clone()),
                u32::try_from(child_count_after_move).map_err(|_| "Too many child categories")?,
            )
        }
    };

    if let Some(destination_parent_id) = parent_id.as_deref() {
        if destination_parent_id == dragged.id || is_descendant(&by_id, destination_parent_id, dragged.id.as_str()) {
            return Err("A category cannot be moved into its own subtree".to_string());
        }
        if let Some(parent) = by_id.get(destination_parent_id).copied() {
            if parent.is_archived && !dragged.is_archived {
                return Err("An active category cannot be moved beneath an archived category".to_string());
            }
        }
    }

    if dragged.parent_id == parent_id {
        let current_position = u32::try_from(dragged.position)
            .map_err(|_| "Category position must be zero or greater".to_string())?;
        if current_position == position {
            return Ok(None);
        }
    }

    if placement == CategoryDropPlacement::RootEnd && dragged.parent_id.is_none() {
        let root_count = items.iter().filter(|item| item.parent_id.is_none()).count();
        if root_count > 0 {
            position = position.min((root_count - 1) as u32);
        }
    }

    Ok(Some(CategoryMoveRequest {
        category_id: dragged.id.clone(),
        parent_id,
        position,
    }))
}

fn is_descendant(
    by_id: &HashMap<&str, &CategoryListItem>,
    candidate_id: &str,
    ancestor_id: &str,
) -> bool {
    let mut current_id = Some(candidate_id);
    let mut remaining = by_id.len() + 1;
    while let Some(category_id) = current_id {
        if category_id == ancestor_id {
            return true;
        }
        if remaining == 0 {
            return true;
        }
        remaining -= 1;
        current_id = by_id
            .get(category_id)
            .and_then(|item| item.parent_id.as_deref());
    }
    false
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicDetail {
    pub id: String,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub category_id: String,
    pub author_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub body_format: String,
    pub content_json: Option<Value>,
    pub status: String,
    pub tags: Vec<String>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicListItem {
    pub id: String,
    pub locale: String,
    pub effective_locale: String,
    pub category_id: String,
    pub author_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReplyListItem {
    pub id: String,
    pub locale: String,
    pub effective_locale: String,
    pub topic_id: String,
    pub author_id: Option<String>,
    pub content_preview: String,
    pub status: String,
    pub parent_reply_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct CategoryDraft {
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub position: i32,
    pub moderated: bool,
}

#[derive(Clone, Debug)]
pub struct TopicDraft {
    pub locale: String,
    pub category_id: String,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub body_format: String,
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        category_drop_move_request, CategoryDropPlacement, CategoryListItem, CategoryTreeNode,
        CategoryTreeResponse,
    };

    fn category(id: &str, parent_id: Option<&str>, position: i32, depth: u16) -> CategoryListItem {
        CategoryListItem {
            id: id.to_string(),
            locale: "en".to_string(),
            effective_locale: "en".to_string(),
            name: id.to_string(),
            slug: id.to_string(),
            description: None,
            icon: None,
            color: None,
            parent_id: parent_id.map(str::to_string),
            depth,
            position,
            moderated: false,
            allows_topics: true,
            archived_at: None,
            is_archived: false,
            topic_count: 0,
            reply_count: 0,
        }
    }

    fn category_json(color: &str) -> String {
        serde_json::json!({
            "id": "category-1",
            "locale": "en",
            "effective_locale": "en",
            "name": "General",
            "slug": "general",
            "description": null,
            "icon": null,
            "color": color,
            "topic_count": 0,
            "reply_count": 0
        })
        .to_string()
    }

    #[test]
    fn category_models_normalize_hex_colors_at_transport_boundary() {
        let category: CategoryListItem =
            serde_json::from_str(category_json(" #F59E0B ").as_str()).expect("category");
        assert_eq!(category.color.as_deref(), Some("#F59E0B"));
        assert!(category.allows_topics);
    }

    #[test]
    fn category_models_drop_css_declaration_injection() {
        let category: CategoryListItem = serde_json::from_str(
            category_json("#fff;background:url(https://attacker.invalid/x)").as_str(),
        )
        .expect("category");
        assert_eq!(category.color, None);
    }

    #[test]
    fn canonical_tree_flattens_in_preorder() {
        let response = CategoryTreeResponse {
            total_nodes: 3,
            max_depth: 1,
            roots: vec![CategoryTreeNode {
                id: "root".to_string(),
                parent_id: None,
                depth: 0,
                position: 0,
                requested_locale: "en".to_string(),
                effective_locale: "en".to_string(),
                name: "Root".to_string(),
                slug: "root".to_string(),
                description: None,
                icon: None,
                color: None,
                moderated: false,
                allows_topics: true,
                archived_at: None,
                is_archived: false,
                topic_count: 0,
                reply_count: 0,
                children: vec![
                    CategoryTreeNode {
                        id: "first".to_string(),
                        parent_id: Some("root".to_string()),
                        depth: 1,
                        position: 0,
                        requested_locale: "en".to_string(),
                        effective_locale: "en".to_string(),
                        name: "First".to_string(),
                        slug: "first".to_string(),
                        description: None,
                        icon: None,
                        color: None,
                        moderated: false,
                        allows_topics: true,
                        archived_at: None,
                        is_archived: false,
                        topic_count: 0,
                        reply_count: 0,
                        children: Vec::new(),
                    },
                    CategoryTreeNode {
                        id: "second".to_string(),
                        parent_id: Some("root".to_string()),
                        depth: 1,
                        position: 1,
                        requested_locale: "en".to_string(),
                        effective_locale: "en".to_string(),
                        name: "Second".to_string(),
                        slug: "second".to_string(),
                        description: None,
                        icon: None,
                        color: None,
                        moderated: false,
                        allows_topics: true,
                        archived_at: None,
                        is_archived: false,
                        topic_count: 0,
                        reply_count: 0,
                        children: Vec::new(),
                    },
                ],
            }],
        };
        let ids = response
            .into_flat_items()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["root", "first", "second"]);
    }

    #[test]
    fn drop_plan_moves_before_and_prevents_cycles() {
        let items = vec![
            category("root", None, 0, 0),
            category("first", Some("root"), 0, 1),
            category("second", Some("root"), 1, 1),
            category("nested", Some("first"), 0, 2),
        ];
        let before = category_drop_move_request(
            &items,
            "second",
            Some("first"),
            CategoryDropPlacement::Before,
        )
        .expect("drop plan")
        .expect("move");
        assert_eq!(before.parent_id.as_deref(), Some("root"));
        assert_eq!(before.position, 0);

        let cycle = category_drop_move_request(
            &items,
            "first",
            Some("nested"),
            CategoryDropPlacement::Inside,
        )
        .expect_err("cycle accepted");
        assert!(cycle.contains("own subtree"));
    }
}
