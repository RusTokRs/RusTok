use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Typed schema representation of a RusToK Starter Blueprint package.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StarterBlueprint {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_locale")]
    pub locale: String,
    pub content: StarterContent,
}

fn default_locale() -> String {
    "ru".to_string()
}

/// Content payload grouped by domain boundaries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StarterContent {
    #[serde(default)]
    pub taxonomy: Option<TaxonomyStarter>,
    #[serde(default)]
    pub pages: Option<Vec<PageStarter>>,
    #[serde(default)]
    pub blog: Option<BlogStarter>,
    #[serde(default)]
    pub forum: Option<ForumStarter>,
    #[serde(default)]
    pub navigation: Option<NavigationStarter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TaxonomyStarter {
    #[serde(default)]
    pub categories: Vec<TaxonomyCategoryStarter>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaxonomyCategoryStarter {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageStarter {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default = "default_true")]
    pub publish: bool,
    #[serde(default)]
    pub channels: Option<Vec<String>>,
    pub body: PageBodyStarter,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBodyStarter {
    #[serde(default = "default_grapesjs_format")]
    pub format: String,
    pub document: serde_json::Value,
}

fn default_grapesjs_format() -> String {
    "grapesjs".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BlogStarter {
    #[serde(default)]
    pub posts: Vec<BlogPostStarter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogPostStarter {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub excerpt: Option<String>,
    #[serde(default)]
    pub category_slug: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub content_markdown: String,
    #[serde(default = "default_true")]
    pub publish: bool,
    #[serde(default)]
    pub featured_image_url: Option<String>,
    #[serde(default)]
    pub translations: std::collections::HashMap<String, BlogPostTranslationStarter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BlogPostTranslationStarter {
    pub title: String,
    #[serde(default)]
    pub excerpt: Option<String>,
    pub content_markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ForumStarter {
    #[serde(default)]
    pub categories: Vec<ForumCategoryStarter>,
    #[serde(default)]
    pub topics: Vec<ForumTopicStarter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumCategoryStarter {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub position: Option<i32>,
    #[serde(default)]
    pub moderated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumTopicStarter {
    pub category_slug: String,
    pub title: String,
    #[serde(default)]
    pub slug: Option<String>,
    pub body_markdown: String,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub replies: Vec<ForumReplyStarter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumReplyStarter {
    pub body_markdown: String,
    #[serde(default)]
    pub is_solution: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NavigationStarter {
    #[serde(default)]
    pub menus: Vec<NavigationMenuStarter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigationMenuStarter {
    #[serde(default = "default_header_location")]
    pub location: String,
    pub name: String,
    #[serde(default)]
    pub items: Vec<NavigationMenuItemStarter>,
}

fn default_header_location() -> String {
    "header".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigationMenuItemStarter {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub position: i32,
}

/// Execution telemetry and statistics returned after applying a blueprint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StarterExecutionReport {
    pub tenant_id: Uuid,
    pub blueprint_id: String,
    pub pages_created: usize,
    pub blog_categories_created: usize,
    pub blog_posts_created: usize,
    pub forum_categories_created: usize,
    pub forum_topics_created: usize,
    pub forum_replies_created: usize,
    pub menus_created: usize,
    pub skipped_existing: usize,
    pub duration_ms: u128,
}
