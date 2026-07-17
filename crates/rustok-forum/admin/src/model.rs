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
    pub topic_count: i32,
    pub reply_count: i32,
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
    use super::CategoryListItem;

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
    }

    #[test]
    fn category_models_drop_css_declaration_injection() {
        let category: CategoryListItem = serde_json::from_str(
            category_json("#fff;background:url(https://attacker.invalid/x)").as_str(),
        )
        .expect("category");
        assert_eq!(category.color, None);
    }
}
