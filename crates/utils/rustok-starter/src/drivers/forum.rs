use std::collections::HashMap;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_core::SecurityContext;
use rustok_forum::{
    CategoryService as ForumCategoryService, CategoryTreeNode, CategoryTreeQuery,
    CreateCategoryInput, CreateReplyInput, CreateTopicInput, ListTopicsFilter, ModerationService,
    ReplyService, TopicService,
};
use rustok_outbox::TransactionalEventBus;

use crate::error::StarterResult;
use crate::model::{ForumCategoryStarter, ForumTopicStarter};

fn collect_tree_categories(nodes: &[CategoryTreeNode], map: &mut HashMap<String, Uuid>) {
    for node in nodes {
        map.insert(node.slug.clone(), node.id);
        collect_tree_categories(&node.children, map);
    }
}

/// Imports Discourse-style forum categories.
pub async fn import_forum_categories(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: &SecurityContext,
    categories: &[ForumCategoryStarter],
    locale: &str,
) -> StarterResult<(HashMap<String, Uuid>, usize, usize)> {
    let cat_service = ForumCategoryService::new(db.clone());
    let mut slug_to_id = HashMap::new();
    let mut created = 0;
    let mut skipped = 0;

    // Load existing categories through the canonical CategoryService tree
    let existing_tree = cat_service
        .tree(
            tenant_id,
            security.clone(),
            CategoryTreeQuery {
                locale: Some(locale.to_string()),
                ..Default::default()
            },
        )
        .await
        .ok();

    let mut existing_map: HashMap<String, Uuid> = HashMap::new();
    if let Some(tree) = existing_tree {
        collect_tree_categories(&tree.roots, &mut existing_map);
    }

    for cat in categories {
        if let Some(&existing_id) = existing_map.get(&cat.slug) {
            tracing::info!(slug = %cat.slug, "Forum category already exists, reusing ID");
            slug_to_id.insert(cat.slug.clone(), existing_id);
            skipped += 1;
            continue;
        }

        let response = cat_service
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: locale.to_string(),
                    name: cat.name.clone(),
                    slug: cat.slug.clone(),
                    description: cat.description.clone(),
                    icon: cat.icon.clone(),
                    color: cat.color.clone(),
                    parent_id: None,
                    position: cat.position,
                    moderated: cat.moderated,
                },
            )
            .await?;

        slug_to_id.insert(cat.slug.clone(), response.id);
        created += 1;
    }

    Ok((slug_to_id, created, skipped))
}

/// Imports forum topics, tags, replies, and accepted solutions.
pub async fn import_forum_topics(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: &SecurityContext,
    topics: &[ForumTopicStarter],
    category_map: &HashMap<String, Uuid>,
    locale: &str,
) -> StarterResult<(usize, usize, usize)> {
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let moderation_service = ModerationService::new(db.clone(), event_bus.clone());

    let mut topics_created = 0;
    let mut replies_created = 0;
    let mut skipped = 0;

    for topic in topics {
        let Some(&category_id) = category_map.get(&topic.category_slug) else {
            tracing::warn!(cat_slug = %topic.category_slug, "Category not found for topic, skipping");
            skipped += 1;
            continue;
        };

        // Idempotency: check if topic with this title already exists in category
        let existing_topics = topic_service
            .list(
                tenant_id,
                security.clone(),
                ListTopicsFilter {
                    category_id: Some(category_id),
                    per_page: 100,
                    ..Default::default()
                },
            )
            .await
            .map(|(items, _)| items)
            .unwrap_or_default();

        let exists = existing_topics.iter().any(|item| {
            item.title == topic.title
                || topic
                    .slug
                    .as_deref()
                    .is_some_and(|s| s == item.slug)
        });

        if exists {
            tracing::info!(title = %topic.title, "Forum topic already exists, skipping");
            skipped += 1;
            continue;
        }

        let created_topic = topic_service
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: locale.to_string(),
                    category_id,
                    title: topic.title.clone(),
                    slug: topic.slug.clone(),
                    body: rustok_api::RichTextDocument::single_paragraph(&topic.body_markdown),
                    metadata: serde_json::json!({}),
                    tags: topic.tags.clone(),
                    channel_slugs: None,
                },
            )
            .await?;

        if topic.is_pinned {
            let pin_res = moderation_service
                .pin_topic(tenant_id, created_topic.id, security.clone())
                .await;
            if let Err(e) = pin_res {
                tracing::warn!(error = ?e, "Failed to pin starter forum topic");
            }
        }

        topics_created += 1;

        for reply in &topic.replies {
            let created_reply = reply_service
                .create(
                    tenant_id,
                    security.clone(),
                    created_topic.id,
                    CreateReplyInput {
                        locale: locale.to_string(),
                        content: rustok_api::RichTextDocument::single_paragraph(&reply.body_markdown),
                        parent_reply_id: None,
                    },
                )
                .await?;

            if reply.is_solution {
                let solution_res = moderation_service
                    .mark_solution(
                        tenant_id,
                        created_topic.id,
                        created_reply.id,
                        security.clone(),
                    )
                    .await;
                if let Err(e) = solution_res {
                    tracing::warn!(error = ?e, "Failed to mark starter reply as accepted solution");
                }
            }

            replies_created += 1;
        }
    }

    Ok((topics_created, replies_created, skipped))
}
