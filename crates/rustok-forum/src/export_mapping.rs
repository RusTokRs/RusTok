use std::collections::BTreeSet;

use rustok_api::RichTextDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{CategoryResponse, ReplyResponse, TopicResponse};

pub const FORUM_EXPORT_SCHEMA_V1: &str = "rustok.forum.export.v1";
pub const MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT: usize = 512;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ForumExportOwnerViewBatch {
    #[serde(default)]
    pub categories: Vec<CategoryResponse>,
    #[serde(default)]
    pub topics: Vec<TopicResponse>,
    #[serde(default)]
    pub replies: Vec<ReplyResponse>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumExportUserRef {
    pub user_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForumExportCategoryRecord {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub position: i32,
    pub moderated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForumExportTopicRecord {
    pub id: Uuid,
    pub category_id: Uuid,
    pub author: Option<ForumExportUserRef>,
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub body: RichTextDocument,
    pub metadata: Value,
    pub status: String,
    pub tags: Vec<String>,
    pub channel_slugs: Vec<String>,
    pub solution_reply_id: Option<Uuid>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForumExportReplyRecord {
    pub id: Uuid,
    pub topic_id: Uuid,
    pub author: Option<ForumExportUserRef>,
    pub parent_reply_id: Option<Uuid>,
    pub locale: String,
    pub content: RichTextDocument,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForumExportFragment {
    pub schema: String,
    pub categories: Vec<ForumExportCategoryRecord>,
    pub topics: Vec<ForumExportTopicRecord>,
    pub replies: Vec<ForumExportReplyRecord>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ForumExportMappingError {
    #[error("Forum export fragment exceeds {max} owner views: {actual}")]
    FragmentTooLarge { max: usize, actual: usize },
    #[error("Forum export {kind} {id} has no effective locale")]
    EmptyEffectiveLocale { kind: &'static str, id: Uuid },
    #[error("Forum export {kind} {id} repeats effective locale {locale}")]
    DuplicateLocalizedView {
        kind: &'static str,
        id: Uuid,
        locale: String,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumOwnerExportMapper;

impl ForumOwnerExportMapper {
    pub fn map_fragment(
        &self,
        batch: &ForumExportOwnerViewBatch,
    ) -> Result<ForumExportFragment, ForumExportMappingError> {
        ensure_fragment_bound(batch)?;

        let mut category_locales = BTreeSet::new();
        let mut topic_locales = BTreeSet::new();
        let mut reply_locales = BTreeSet::new();

        let categories = batch
            .categories
            .iter()
            .map(|view| {
                let locale = unique_effective_locale(
                    "category",
                    view.id,
                    &view.effective_locale,
                    &mut category_locales,
                )?;
                Ok(ForumExportCategoryRecord {
                    id: view.id,
                    parent_id: view.parent_id,
                    locale,
                    name: view.name.clone(),
                    slug: view.slug.clone(),
                    description: view.description.clone(),
                    icon: view.icon.clone(),
                    color: view.color.clone(),
                    position: view.position,
                    moderated: view.moderated,
                })
            })
            .collect::<Result<Vec<_>, ForumExportMappingError>>()?;

        let topics = batch
            .topics
            .iter()
            .map(|view| {
                let locale = unique_effective_locale(
                    "topic",
                    view.id,
                    &view.effective_locale,
                    &mut topic_locales,
                )?;
                Ok(ForumExportTopicRecord {
                    id: view.id,
                    category_id: view.category_id,
                    author: export_user_ref(view.author_id),
                    locale,
                    title: view.title.clone(),
                    slug: view.slug.clone(),
                    body: view.body.document.clone(),
                    metadata: view.metadata.clone(),
                    status: view.status.clone(),
                    tags: view.tags.clone(),
                    channel_slugs: view.channel_slugs.clone(),
                    solution_reply_id: view.solution_reply_id,
                    is_pinned: view.is_pinned,
                    is_locked: view.is_locked,
                    created_at: view.created_at.clone(),
                    updated_at: view.updated_at.clone(),
                })
            })
            .collect::<Result<Vec<_>, ForumExportMappingError>>()?;

        let replies = batch
            .replies
            .iter()
            .map(|view| {
                let locale = unique_effective_locale(
                    "reply",
                    view.id,
                    &view.effective_locale,
                    &mut reply_locales,
                )?;
                Ok(ForumExportReplyRecord {
                    id: view.id,
                    topic_id: view.topic_id,
                    author: export_user_ref(view.author_id),
                    parent_reply_id: view.parent_reply_id,
                    locale,
                    content: view.content.document.clone(),
                    status: view.status.clone(),
                    created_at: view.created_at.clone(),
                    updated_at: view.updated_at.clone(),
                })
            })
            .collect::<Result<Vec<_>, ForumExportMappingError>>()?;

        Ok(ForumExportFragment {
            schema: FORUM_EXPORT_SCHEMA_V1.to_owned(),
            categories,
            topics,
            replies,
        })
    }
}

fn ensure_fragment_bound(batch: &ForumExportOwnerViewBatch) -> Result<(), ForumExportMappingError> {
    let actual = batch
        .categories
        .len()
        .saturating_add(batch.topics.len())
        .saturating_add(batch.replies.len());
    if actual > MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT {
        return Err(ForumExportMappingError::FragmentTooLarge {
            max: MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT,
            actual,
        });
    }
    Ok(())
}

fn unique_effective_locale(
    kind: &'static str,
    id: Uuid,
    effective_locale: &str,
    seen: &mut BTreeSet<(Uuid, String)>,
) -> Result<String, ForumExportMappingError> {
    if effective_locale.trim().is_empty() {
        return Err(ForumExportMappingError::EmptyEffectiveLocale { kind, id });
    }
    let locale = effective_locale.to_owned();
    if !seen.insert((id, locale.clone())) {
        return Err(ForumExportMappingError::DuplicateLocalizedView { kind, id, locale });
    }
    Ok(locale)
}

fn export_user_ref(user_id: Option<Uuid>) -> Option<ForumExportUserRef> {
    user_id.map(|user_id| ForumExportUserRef { user_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::RichTextView;
    use serde_json::json;

    fn category(id: Uuid, requested: &str, effective: &str) -> CategoryResponse {
        CategoryResponse {
            id,
            requested_locale: requested.to_owned(),
            locale: requested.to_owned(),
            effective_locale: effective.to_owned(),
            available_locales: vec![effective.to_owned()],
            name: "General".to_owned(),
            slug: "general".to_owned(),
            description: Some("General discussion".to_owned()),
            icon: Some("messages".to_owned()),
            color: Some("#112233".to_owned()),
            parent_id: None,
            position: 2,
            topic_count: 99,
            reply_count: 199,
            moderated: true,
            is_subscribed: true,
        }
    }

    fn topic(id: Uuid, author_id: Uuid) -> TopicResponse {
        let document = RichTextDocument::single_paragraph("Canonical topic body");
        TopicResponse {
            id,
            requested_locale: "de".to_owned(),
            locale: "de".to_owned(),
            effective_locale: "en".to_owned(),
            available_locales: vec!["en".to_owned()],
            category_id: Uuid::new_v4(),
            author_id: Some(author_id),
            title: "Topic".to_owned(),
            slug: "topic".to_owned(),
            body: RichTextView {
                document,
                html: "<p>rendered</p>".to_owned(),
            },
            body_plain_text: "Canonical topic body".to_owned(),
            metadata: json!({"kind": "discussion"}),
            status: "open".to_owned(),
            tags: vec!["migration".to_owned()],
            channel_slugs: vec!["web".to_owned()],
            vote_score: 42,
            current_user_vote: Some(1),
            is_subscribed: true,
            solution_reply_id: None,
            is_pinned: true,
            is_locked: false,
            reply_count: 5,
            created_at: "2026-08-09T00:00:00Z".to_owned(),
            updated_at: "2026-08-09T01:00:00Z".to_owned(),
        }
    }

    fn reply(id: Uuid, topic_id: Uuid, author_id: Uuid) -> ReplyResponse {
        let document = RichTextDocument::single_paragraph("Canonical reply body");
        ReplyResponse {
            id,
            requested_locale: "en".to_owned(),
            locale: "en".to_owned(),
            effective_locale: "en".to_owned(),
            topic_id,
            author_id: Some(author_id),
            content: RichTextView {
                document,
                html: "<p>rendered reply</p>".to_owned(),
            },
            content_plain_text: "Canonical reply body".to_owned(),
            status: "approved".to_owned(),
            vote_score: 7,
            current_user_vote: Some(-1),
            is_solution: true,
            parent_reply_id: None,
            created_at: "2026-08-09T00:30:00Z".to_owned(),
            updated_at: "2026-08-09T00:45:00Z".to_owned(),
        }
    }

    #[test]
    fn maps_effective_locale_and_canonical_documents_without_viewer_state() {
        let category_id = Uuid::new_v4();
        let topic_id = Uuid::new_v4();
        let reply_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();
        let batch = ForumExportOwnerViewBatch {
            categories: vec![category(category_id, "de", "en")],
            topics: vec![topic(topic_id, author_id)],
            replies: vec![reply(reply_id, topic_id, author_id)],
        };

        let mapped = ForumOwnerExportMapper.map_fragment(&batch).unwrap();
        assert_eq!(mapped.schema, FORUM_EXPORT_SCHEMA_V1);
        assert_eq!(mapped.categories[0].locale, "en");
        assert_eq!(mapped.topics[0].locale, "en");
        assert_eq!(mapped.topics[0].author.as_ref().unwrap().user_id, author_id);
        assert_eq!(
            mapped.topics[0].body,
            RichTextDocument::single_paragraph("Canonical topic body")
        );
        assert_eq!(
            mapped.replies[0].content,
            RichTextDocument::single_paragraph("Canonical reply body")
        );

        let wire = serde_json::to_value(mapped).unwrap();
        let topic_wire = &wire["topics"][0];
        let reply_wire = &wire["replies"][0];
        assert!(topic_wire.get("html").is_none());
        assert!(topic_wire.get("body_plain_text").is_none());
        assert!(topic_wire.get("vote_score").is_none());
        assert!(topic_wire.get("current_user_vote").is_none());
        assert!(topic_wire.get("is_subscribed").is_none());
        assert!(topic_wire.get("reply_count").is_none());
        assert!(reply_wire.get("content_plain_text").is_none());
        assert!(reply_wire.get("vote_score").is_none());
        assert!(reply_wire.get("current_user_vote").is_none());
        assert!(reply_wire.get("is_solution").is_none());
    }

    #[test]
    fn rejects_duplicate_effective_locale_even_when_requested_locale_differs() {
        let category_id = Uuid::new_v4();
        let batch = ForumExportOwnerViewBatch {
            categories: vec![
                category(category_id, "de", "en"),
                category(category_id, "fr", "en"),
            ],
            ..Default::default()
        };

        assert_eq!(
            ForumOwnerExportMapper.map_fragment(&batch),
            Err(ForumExportMappingError::DuplicateLocalizedView {
                kind: "category",
                id: category_id,
                locale: "en".to_owned(),
            })
        );
    }
}
