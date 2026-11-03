use crate::error::StarterError;
use crate::model::{
    BlogPostStarter, BlogStarter, ForumCategoryStarter, ForumReplyStarter, ForumStarter,
    ForumTopicStarter, NavigationMenuItemStarter, NavigationMenuStarter, NavigationStarter,
    PageBodyStarter, PageStarter, StarterBlueprint, StarterContent, StarterExecutionReport,
    TaxonomyCategoryStarter, TaxonomyStarter,
};
use uuid::Uuid;

#[test]
fn test_blueprint_deserialization_defaults() {
    let raw_json = r#"{
        "schema_version": "1.0",
        "id": "minimal-blueprint",
        "name": "Minimal Blueprint",
        "content": {}
    }"#;

    let blueprint: StarterBlueprint =
        serde_json::from_str(raw_json).expect("minimal blueprint must deserialize");
    assert_eq!(blueprint.schema_version, "1.0");
    assert_eq!(blueprint.id, "minimal-blueprint");
    assert_eq!(blueprint.name, "Minimal Blueprint");
    assert_eq!(blueprint.locale, "ru");
    assert!(blueprint.description.is_none());
    assert!(blueprint.content.taxonomy.is_none());
    assert!(blueprint.content.pages.is_none());
    assert!(blueprint.content.blog.is_none());
    assert!(blueprint.content.forum.is_none());
    assert!(blueprint.content.navigation.is_none());
}

#[test]
fn test_blueprint_serialization_roundtrip() {
    let blueprint = StarterBlueprint {
        schema_version: "1.0".to_string(),
        id: "roundtrip-test".to_string(),
        name: "Roundtrip Test".to_string(),
        description: Some("Description".to_string()),
        locale: "en".to_string(),
        content: StarterContent {
            taxonomy: Some(TaxonomyStarter {
                categories: vec![TaxonomyCategoryStarter {
                    slug: "tech".to_string(),
                    name: "Technology".to_string(),
                    description: Some("Tech articles".to_string()),
                }],
                tags: vec!["rust".to_string()],
            }),
            pages: Some(vec![PageStarter {
                slug: "landing".to_string(),
                title: "Landing Page".to_string(),
                template: Some("fullwidth".to_string()),
                publish: true,
                channels: Some(vec!["web".to_string()]),
                body: PageBodyStarter {
                    format: "grapesjs".to_string(),
                    document: serde_json::json!({ "components": [] }),
                },
            }]),
            blog: Some(BlogStarter {
                posts: vec![BlogPostStarter {
                    slug: "first-post".to_string(),
                    title: "First Post".to_string(),
                    excerpt: Some("Excerpt".to_string()),
                    category_slug: Some("tech".to_string()),
                    tags: vec!["rust".to_string()],
                    content_markdown: "# Hello".to_string(),
                    publish: true,
                    featured_image_url: None,
                    translations: std::collections::HashMap::new(),
                }],
            }),
            forum: Some(ForumStarter {
                categories: vec![ForumCategoryStarter {
                    slug: "general".to_string(),
                    name: "General".to_string(),
                    description: None,
                    color: Some("#ffffff".to_string()),
                    icon: Some("chat".to_string()),
                    position: Some(1),
                    moderated: false,
                }],
                topics: vec![ForumTopicStarter {
                    category_slug: "general".to_string(),
                    title: "Topic 1".to_string(),
                    slug: Some("topic-1".to_string()),
                    body_markdown: "Body".to_string(),
                    is_pinned: false,
                    tags: vec![],
                    replies: vec![ForumReplyStarter {
                        body_markdown: "Reply 1".to_string(),
                        is_solution: true,
                    }],
                }],
            }),
            navigation: Some(NavigationStarter {
                menus: vec![NavigationMenuStarter {
                    location: "header".to_string(),
                    name: "Header Menu".to_string(),
                    items: vec![NavigationMenuItemStarter {
                        title: "Home".to_string(),
                        url: "/".to_string(),
                        icon: None,
                        position: 1,
                    }],
                }],
            }),
        },
    };

    let serialized = serde_json::to_string(&blueprint).expect("serialization should succeed");
    let deserialized: StarterBlueprint =
        serde_json::from_str(&serialized).expect("deserialization should succeed");
    assert_eq!(blueprint, deserialized);
}

#[test]
fn test_starter_execution_report_defaults() {
    let report = StarterExecutionReport::default();
    assert_eq!(report.tenant_id, Uuid::nil());
    assert_eq!(report.blueprint_id, "");
    assert_eq!(report.pages_created, 0);
    assert_eq!(report.blog_categories_created, 0);
    assert_eq!(report.blog_posts_created, 0);
    assert_eq!(report.forum_categories_created, 0);
    assert_eq!(report.forum_topics_created, 0);
    assert_eq!(report.forum_replies_created, 0);
    assert_eq!(report.menus_created, 0);
    assert_eq!(report.skipped_existing, 0);
    assert_eq!(report.duration_ms, 0);
}

#[test]
fn test_starter_error_display_formatting() {
    let val_err = StarterError::Validation("invalid payload".to_string());
    assert_eq!(format!("{val_err}"), "Validation error: invalid payload");

    let tenant_err = StarterError::TenantNotFound("tenant-123".to_string());
    assert_eq!(format!("{tenant_err}"), "Tenant not found: tenant-123");

    let bp_err = StarterError::BlueprintNotFound("missing-bp".to_string());
    assert_eq!(
        format!("{bp_err}"),
        "Starter blueprint not found: missing-bp"
    );
}

#[test]
fn test_embedded_default_starter_invariants() {
    let blueprint = crate::default_starter();
    assert_eq!(blueprint.id, "default-starter");
    assert_eq!(blueprint.schema_version, "1.0");
    assert_eq!(blueprint.locale, "ru");
    assert!(!blueprint.name.is_empty());
    assert!(blueprint.description.is_some());

    let content = blueprint.content;
    let taxonomy = content.taxonomy.expect("taxonomy must exist");
    assert!(!taxonomy.categories.is_empty());
    assert!(!taxonomy.tags.is_empty());

    let pages = content.pages.expect("pages must exist");
    assert!(!pages.is_empty());
    for page in &pages {
        assert!(!page.slug.is_empty());
        assert!(!page.title.is_empty());
    }

    let blog = content.blog.expect("blog must exist");
    assert!(!blog.posts.is_empty());
    let category_slugs: std::collections::HashSet<_> = taxonomy
        .categories
        .iter()
        .map(|c| c.slug.as_str())
        .collect();
    for post in &blog.posts {
        assert!(!post.slug.is_empty());
        assert!(!post.title.is_empty());
        if let Some(ref cat_slug) = post.category_slug {
            assert!(
                category_slugs.contains(cat_slug.as_str()),
                "Blog post category slug '{cat_slug}' must exist in taxonomy categories"
            );
        }
    }

    let forum = content.forum.expect("forum must exist");
    assert!(!forum.categories.is_empty());
    assert!(!forum.topics.is_empty());
    let forum_cat_slugs: std::collections::HashSet<_> =
        forum.categories.iter().map(|c| c.slug.as_str()).collect();
    for topic in &forum.topics {
        assert!(
            forum_cat_slugs.contains(topic.category_slug.as_str()),
            "Forum topic category slug '{}' must exist in forum categories",
            topic.category_slug
        );
    }

    let navigation = content.navigation.expect("navigation must exist");
    assert!(!navigation.menus.is_empty());
    for menu in &navigation.menus {
        assert!(!menu.items.is_empty());
    }
}
