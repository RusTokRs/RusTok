use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FORUM_IMPORT_SOURCE_NODEBB: &str = "nodebb";
pub const MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumImportEntityKind {
    Category,
    Topic,
    Post,
    User,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumImportExternalRef {
    pub source: String,
    pub kind: ForumImportEntityKind,
    pub key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumImportPostRole {
    TopicBody,
    Reply,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumImportCategoryCandidate {
    pub source: ForumImportExternalRef,
    pub parent_source: Option<ForumImportExternalRef>,
    pub name: String,
    pub description: Option<String>,
    pub position: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumImportTopicCandidate {
    pub source: ForumImportExternalRef,
    pub category_source: ForumImportExternalRef,
    pub author_source: Option<ForumImportExternalRef>,
    pub title: String,
    pub slug: Option<String>,
    pub body_post_source: Option<ForumImportExternalRef>,
    pub created_at_ms: Option<i64>,
    pub is_pinned: bool,
    pub is_locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumImportPostCandidate {
    pub source: ForumImportExternalRef,
    pub topic_source: ForumImportExternalRef,
    pub author_source: Option<ForumImportExternalRef>,
    pub role: ForumImportPostRole,
    pub body: String,
    pub created_at_ms: Option<i64>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumImportCandidateBatch {
    pub categories: Vec<ForumImportCategoryCandidate>,
    pub topics: Vec<ForumImportTopicCandidate>,
    pub posts: Vec<ForumImportPostCandidate>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodebbExportBatch {
    #[serde(default)]
    pub categories: Vec<NodebbCategoryRecord>,
    #[serde(default)]
    pub topics: Vec<NodebbTopicRecord>,
    #[serde(default)]
    pub posts: Vec<NodebbPostRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodebbCategoryRecord {
    pub cid: i64,
    #[serde(rename = "parentCid", alias = "parent_cid", default)]
    pub parent_cid: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub order: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodebbTopicRecord {
    pub tid: i64,
    pub cid: i64,
    #[serde(default)]
    pub uid: Option<i64>,
    pub title: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(rename = "mainPid", alias = "main_pid", default)]
    pub main_pid: Option<i64>,
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodebbPostRecord {
    pub pid: i64,
    pub tid: i64,
    #[serde(default)]
    pub uid: Option<i64>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ForumImportMappingError {
    #[error("Forum import batch exceeds {max} source records: {actual}")]
    BatchTooLarge { max: usize, actual: usize },
    #[error("NodeBB {kind} source id must be positive: {id}")]
    InvalidSourceId { kind: &'static str, id: i64 },
    #[error("NodeBB {kind} source id is duplicated in the batch: {id}")]
    DuplicateSourceId { kind: &'static str, id: i64 },
    #[error("NodeBB {kind} required text field is empty: {field}")]
    EmptyRequiredText {
        kind: &'static str,
        field: &'static str,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NodebbForumImportMapper;

impl NodebbForumImportMapper {
    pub fn map_batch(
        &self,
        batch: &NodebbExportBatch,
    ) -> Result<ForumImportCandidateBatch, ForumImportMappingError> {
        ensure_batch_bound(batch)?;
        ensure_unique_positive_ids(
            "category",
            batch.categories.iter().map(|record| record.cid),
        )?;
        ensure_unique_positive_ids("topic", batch.topics.iter().map(|record| record.tid))?;
        ensure_unique_positive_ids("post", batch.posts.iter().map(|record| record.pid))?;

        let topic_main_posts = batch
            .topics
            .iter()
            .map(|topic| {
                ensure_positive("category", topic.cid)?;
                let main_pid = positive_optional(topic.main_pid);
                Ok((topic.tid, main_pid))
            })
            .collect::<Result<BTreeMap<_, _>, ForumImportMappingError>>()?;

        let categories = batch
            .categories
            .iter()
            .map(map_category)
            .collect::<Result<Vec<_>, _>>()?;
        let topics = batch
            .topics
            .iter()
            .map(map_topic)
            .collect::<Result<Vec<_>, _>>()?;
        let posts = batch
            .posts
            .iter()
            .map(|post| map_post(post, &topic_main_posts))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ForumImportCandidateBatch {
            categories,
            topics,
            posts,
        })
    }
}

fn ensure_batch_bound(batch: &NodebbExportBatch) -> Result<(), ForumImportMappingError> {
    let actual = batch
        .categories
        .len()
        .saturating_add(batch.topics.len())
        .saturating_add(batch.posts.len());
    if actual > MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH {
        return Err(ForumImportMappingError::BatchTooLarge {
            max: MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH,
            actual,
        });
    }
    Ok(())
}

fn ensure_unique_positive_ids(
    kind: &'static str,
    ids: impl IntoIterator<Item = i64>,
) -> Result<(), ForumImportMappingError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        ensure_positive(kind, id)?;
        if !seen.insert(id) {
            return Err(ForumImportMappingError::DuplicateSourceId { kind, id });
        }
    }
    Ok(())
}

fn ensure_positive(kind: &'static str, id: i64) -> Result<i64, ForumImportMappingError> {
    if id <= 0 {
        Err(ForumImportMappingError::InvalidSourceId { kind, id })
    } else {
        Ok(id)
    }
}

fn required_text(
    kind: &'static str,
    field: &'static str,
    value: &str,
) -> Result<String, ForumImportMappingError> {
    let value = value.trim();
    if value.is_empty() {
        Err(ForumImportMappingError::EmptyRequiredText { kind, field })
    } else {
        Ok(value.to_owned())
    }
}

fn optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn positive_optional(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value > 0)
}

fn source_ref(kind: ForumImportEntityKind, id: i64) -> ForumImportExternalRef {
    let kind_name = match kind {
        ForumImportEntityKind::Category => "category",
        ForumImportEntityKind::Topic => "topic",
        ForumImportEntityKind::Post => "post",
        ForumImportEntityKind::User => "user",
    };
    ForumImportExternalRef {
        source: FORUM_IMPORT_SOURCE_NODEBB.to_owned(),
        kind,
        key: format!("{kind_name}:{id}"),
    }
}

fn author_ref(uid: Option<i64>) -> Option<ForumImportExternalRef> {
    positive_optional(uid).map(|uid| source_ref(ForumImportEntityKind::User, uid))
}

fn map_category(
    record: &NodebbCategoryRecord,
) -> Result<ForumImportCategoryCandidate, ForumImportMappingError> {
    ensure_positive("category", record.cid)?;
    Ok(ForumImportCategoryCandidate {
        source: source_ref(ForumImportEntityKind::Category, record.cid),
        parent_source: positive_optional(record.parent_cid)
            .map(|cid| source_ref(ForumImportEntityKind::Category, cid)),
        name: required_text("category", "name", &record.name)?,
        description: optional_text(record.description.as_deref()),
        position: record.order,
    })
}

fn map_topic(
    record: &NodebbTopicRecord,
) -> Result<ForumImportTopicCandidate, ForumImportMappingError> {
    ensure_positive("topic", record.tid)?;
    ensure_positive("category", record.cid)?;
    Ok(ForumImportTopicCandidate {
        source: source_ref(ForumImportEntityKind::Topic, record.tid),
        category_source: source_ref(ForumImportEntityKind::Category, record.cid),
        author_source: author_ref(record.uid),
        title: required_text("topic", "title", &record.title)?,
        slug: optional_text(record.slug.as_deref()),
        body_post_source: positive_optional(record.main_pid)
            .map(|pid| source_ref(ForumImportEntityKind::Post, pid)),
        created_at_ms: record.timestamp.filter(|value| *value >= 0),
        is_pinned: record.pinned,
        is_locked: record.locked,
    })
}

fn map_post(
    record: &NodebbPostRecord,
    topic_main_posts: &BTreeMap<i64, Option<i64>>,
) -> Result<ForumImportPostCandidate, ForumImportMappingError> {
    ensure_positive("post", record.pid)?;
    ensure_positive("topic", record.tid)?;
    let role = match topic_main_posts.get(&record.tid) {
        Some(Some(main_pid)) if *main_pid == record.pid => ForumImportPostRole::TopicBody,
        Some(Some(_)) => ForumImportPostRole::Reply,
        Some(None) | None => ForumImportPostRole::Unresolved,
    };
    Ok(ForumImportPostCandidate {
        source: source_ref(ForumImportEntityKind::Post, record.pid),
        topic_source: source_ref(ForumImportEntityKind::Topic, record.tid),
        author_source: author_ref(record.uid),
        role,
        body: record.content.clone(),
        created_at_ms: record.timestamp.filter(|value| *value >= 0),
        deleted: record.deleted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_nodebb_ids_as_external_keys_without_rustok_identity() {
        let batch = NodebbExportBatch {
            categories: vec![NodebbCategoryRecord {
                cid: 4,
                parent_cid: Some(0),
                name: "General".to_owned(),
                description: None,
                order: Some(2),
            }],
            topics: vec![NodebbTopicRecord {
                tid: 9,
                cid: 4,
                uid: Some(7),
                title: "Welcome".to_owned(),
                slug: Some("welcome".to_owned()),
                main_pid: Some(11),
                timestamp: Some(1234),
                pinned: true,
                locked: false,
            }],
            posts: vec![
                NodebbPostRecord {
                    pid: 11,
                    tid: 9,
                    uid: Some(7),
                    content: "Topic body".to_owned(),
                    timestamp: Some(1234),
                    deleted: false,
                },
                NodebbPostRecord {
                    pid: 12,
                    tid: 9,
                    uid: Some(8),
                    content: "Reply".to_owned(),
                    timestamp: Some(1235),
                    deleted: false,
                },
            ],
        };

        let mapped = NodebbForumImportMapper.map_batch(&batch).unwrap();
        assert_eq!(mapped.categories[0].source.key, "category:4");
        assert_eq!(mapped.categories[0].parent_source, None);
        assert_eq!(mapped.topics[0].source.key, "topic:9");
        assert_eq!(
            mapped.topics[0]
                .author_source
                .as_ref()
                .map(|value| value.key.as_str()),
            Some("user:7")
        );
        assert_eq!(mapped.posts[0].role, ForumImportPostRole::TopicBody);
        assert_eq!(mapped.posts[1].role, ForumImportPostRole::Reply);
    }

    #[test]
    fn post_role_stays_unresolved_when_topic_is_outside_batch() {
        let batch = NodebbExportBatch {
            posts: vec![NodebbPostRecord {
                pid: 12,
                tid: 9,
                uid: Some(0),
                content: "Cross-page post".to_owned(),
                timestamp: None,
                deleted: false,
            }],
            ..Default::default()
        };

        let mapped = NodebbForumImportMapper.map_batch(&batch).unwrap();
        assert_eq!(mapped.posts[0].role, ForumImportPostRole::Unresolved);
        assert_eq!(mapped.posts[0].author_source, None);
    }

    #[test]
    fn post_role_stays_unresolved_when_topic_has_no_main_post() {
        let batch = NodebbExportBatch {
            topics: vec![NodebbTopicRecord {
                tid: 9,
                cid: 4,
                uid: None,
                title: "No main post yet".to_owned(),
                slug: None,
                main_pid: None,
                timestamp: None,
                pinned: false,
                locked: false,
            }],
            posts: vec![NodebbPostRecord {
                pid: 12,
                tid: 9,
                uid: None,
                content: "Ambiguous post".to_owned(),
                timestamp: None,
                deleted: false,
            }],
            ..Default::default()
        };

        let mapped = NodebbForumImportMapper.map_batch(&batch).unwrap();
        assert_eq!(mapped.posts[0].role, ForumImportPostRole::Unresolved);
    }

    #[test]
    fn rejects_duplicate_or_non_positive_owner_source_ids() {
        let duplicate = NodebbExportBatch {
            categories: vec![
                NodebbCategoryRecord {
                    cid: 4,
                    parent_cid: None,
                    name: "A".to_owned(),
                    description: None,
                    order: None,
                },
                NodebbCategoryRecord {
                    cid: 4,
                    parent_cid: None,
                    name: "B".to_owned(),
                    description: None,
                    order: None,
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            NodebbForumImportMapper.map_batch(&duplicate),
            Err(ForumImportMappingError::DuplicateSourceId {
                kind: "category",
                id: 4,
            })
        );

        let invalid = NodebbExportBatch {
            topics: vec![NodebbTopicRecord {
                tid: 0,
                cid: 4,
                uid: None,
                title: "Invalid".to_owned(),
                slug: None,
                main_pid: None,
                timestamp: None,
                pinned: false,
                locked: false,
            }],
            ..Default::default()
        };
        assert_eq!(
            NodebbForumImportMapper.map_batch(&invalid),
            Err(ForumImportMappingError::InvalidSourceId {
                kind: "topic",
                id: 0,
            })
        );
    }
}
