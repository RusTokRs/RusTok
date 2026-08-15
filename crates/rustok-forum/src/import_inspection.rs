use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::import_mapping::{
    ForumImportCandidateBatch, ForumImportExternalRef, ForumImportMappingError,
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH, NodebbExportBatch, NodebbForumImportMapper,
};

pub const MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH: usize =
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH * 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumImportDependencyRelation {
    CategoryParent,
    TopicCategory,
    TopicMainPost,
    PostTopic,
    AuthorUser,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumImportDependencyDisposition {
    MissingBatchRecord,
    MismatchedBatchRecord,
    CyclicBatchRelation,
    ExternalOwnerResolution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumImportDependencyIssue {
    pub owner: ForumImportExternalRef,
    pub relation: ForumImportDependencyRelation,
    pub target: ForumImportExternalRef,
    pub disposition: ForumImportDependencyDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodebbForumImportInspection {
    pub candidates: ForumImportCandidateBatch,
    pub unresolved_dependencies: Vec<ForumImportDependencyIssue>,
}

impl NodebbForumImportInspection {
    pub fn is_dependency_complete(&self) -> bool {
        self.unresolved_dependencies.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NodebbForumImportInspector;

impl NodebbForumImportInspector {
    pub fn inspect_batch(
        &self,
        batch: &NodebbExportBatch,
    ) -> Result<NodebbForumImportInspection, ForumImportMappingError> {
        let candidates = NodebbForumImportMapper.map_batch(batch)?;
        let category_ids = batch
            .categories
            .iter()
            .map(|record| record.cid)
            .collect::<BTreeSet<_>>();
        let cyclic_category_ids = category_cycle_members(batch, &category_ids);
        let topic_ids = batch
            .topics
            .iter()
            .map(|record| record.tid)
            .collect::<BTreeSet<_>>();
        let post_topics = batch
            .posts
            .iter()
            .map(|record| (record.pid, record.tid))
            .collect::<BTreeMap<_, _>>();

        let mut unresolved_dependencies = Vec::new();

        for (record, candidate) in batch.categories.iter().zip(&candidates.categories) {
            if let Some(parent_id) = record.parent_cid.filter(|value| *value > 0)
                && let Some(parent_source) = candidate.parent_source.as_ref()
            {
                let disposition = if !category_ids.contains(&parent_id) {
                    Some(ForumImportDependencyDisposition::MissingBatchRecord)
                } else if cyclic_category_ids.contains(&record.cid) {
                    Some(ForumImportDependencyDisposition::CyclicBatchRelation)
                } else {
                    None
                };
                if let Some(disposition) = disposition {
                    unresolved_dependencies.push(ForumImportDependencyIssue {
                        owner: candidate.source.clone(),
                        relation: ForumImportDependencyRelation::CategoryParent,
                        target: parent_source.clone(),
                        disposition,
                    });
                }
            }
        }

        for (record, candidate) in batch.topics.iter().zip(&candidates.topics) {
            if !category_ids.contains(&record.cid) {
                unresolved_dependencies.push(ForumImportDependencyIssue {
                    owner: candidate.source.clone(),
                    relation: ForumImportDependencyRelation::TopicCategory,
                    target: candidate.category_source.clone(),
                    disposition: ForumImportDependencyDisposition::MissingBatchRecord,
                });
            }

            if let Some(main_pid) = record.main_pid.filter(|value| *value > 0)
                && let Some(main_post_source) = candidate.body_post_source.as_ref()
            {
                match post_topics.get(&main_pid) {
                    None => unresolved_dependencies.push(ForumImportDependencyIssue {
                        owner: candidate.source.clone(),
                        relation: ForumImportDependencyRelation::TopicMainPost,
                        target: main_post_source.clone(),
                        disposition: ForumImportDependencyDisposition::MissingBatchRecord,
                    }),
                    Some(post_topic_id) if *post_topic_id != record.tid => {
                        unresolved_dependencies.push(ForumImportDependencyIssue {
                            owner: candidate.source.clone(),
                            relation: ForumImportDependencyRelation::TopicMainPost,
                            target: main_post_source.clone(),
                            disposition: ForumImportDependencyDisposition::MismatchedBatchRecord,
                        });
                    }
                    Some(_) => {}
                }
            }

            if let Some(author_source) = candidate.author_source.as_ref() {
                unresolved_dependencies.push(ForumImportDependencyIssue {
                    owner: candidate.source.clone(),
                    relation: ForumImportDependencyRelation::AuthorUser,
                    target: author_source.clone(),
                    disposition: ForumImportDependencyDisposition::ExternalOwnerResolution,
                });
            }
        }

        for (record, candidate) in batch.posts.iter().zip(&candidates.posts) {
            if !topic_ids.contains(&record.tid) {
                unresolved_dependencies.push(ForumImportDependencyIssue {
                    owner: candidate.source.clone(),
                    relation: ForumImportDependencyRelation::PostTopic,
                    target: candidate.topic_source.clone(),
                    disposition: ForumImportDependencyDisposition::MissingBatchRecord,
                });
            }

            if let Some(author_source) = candidate.author_source.as_ref() {
                unresolved_dependencies.push(ForumImportDependencyIssue {
                    owner: candidate.source.clone(),
                    relation: ForumImportDependencyRelation::AuthorUser,
                    target: author_source.clone(),
                    disposition: ForumImportDependencyDisposition::ExternalOwnerResolution,
                });
            }
        }

        debug_assert!(
            unresolved_dependencies.len() <= MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH
        );

        Ok(NodebbForumImportInspection {
            candidates,
            unresolved_dependencies,
        })
    }
}

fn category_cycle_members(
    batch: &NodebbExportBatch,
    category_ids: &BTreeSet<i64>,
) -> BTreeSet<i64> {
    let parent_by_category = batch
        .categories
        .iter()
        .filter_map(|record| {
            record
                .parent_cid
                .filter(|parent_id| *parent_id > 0)
                .map(|parent_id| (record.cid, parent_id))
        })
        .collect::<BTreeMap<_, _>>();

    let mut completed = BTreeSet::new();
    let mut cyclic = BTreeSet::new();

    for start in category_ids.iter().copied() {
        if completed.contains(&start) {
            continue;
        }

        let mut path = Vec::new();
        let mut position_by_category = BTreeMap::new();
        let mut current = start;

        loop {
            if let Some(position) = position_by_category.get(&current).copied() {
                cyclic.extend(path[position..].iter().copied());
                break;
            }
            if completed.contains(&current) {
                break;
            }

            position_by_category.insert(current, path.len());
            path.push(current);

            let Some(parent_id) = parent_by_category.get(&current).copied() else {
                break;
            };
            if !category_ids.contains(&parent_id) {
                break;
            }
            current = parent_id;
        }

        completed.extend(path);
    }

    cyclic
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_mapping::{NodebbCategoryRecord, NodebbPostRecord, NodebbTopicRecord};

    #[test]
    fn reports_missing_mismatched_and_external_dependencies_in_source_order() {
        let batch = NodebbExportBatch {
            categories: vec![NodebbCategoryRecord {
                cid: 2,
                parent_cid: Some(99),
                name: "Child".to_owned(),
                description: None,
                order: None,
            }],
            topics: vec![
                NodebbTopicRecord {
                    tid: 10,
                    cid: 77,
                    uid: Some(7),
                    title: "Missing category".to_owned(),
                    slug: None,
                    main_pid: Some(101),
                    timestamp: None,
                    pinned: false,
                    locked: false,
                },
                NodebbTopicRecord {
                    tid: 11,
                    cid: 2,
                    uid: None,
                    title: "Mismatched body".to_owned(),
                    slug: None,
                    main_pid: Some(102),
                    timestamp: None,
                    pinned: false,
                    locked: false,
                },
            ],
            posts: vec![
                NodebbPostRecord {
                    pid: 102,
                    tid: 10,
                    uid: None,
                    content: "Belongs to another topic".to_owned(),
                    timestamp: None,
                    deleted: false,
                },
                NodebbPostRecord {
                    pid: 103,
                    tid: 88,
                    uid: Some(8),
                    content: "Missing topic".to_owned(),
                    timestamp: None,
                    deleted: false,
                },
            ],
        };

        let inspected = NodebbForumImportInspector.inspect_batch(&batch).unwrap();
        let issues = &inspected.unresolved_dependencies;
        assert_eq!(issues.len(), 7);
        assert_eq!(
            issues[0].relation,
            ForumImportDependencyRelation::CategoryParent
        );
        assert_eq!(
            issues[0].disposition,
            ForumImportDependencyDisposition::MissingBatchRecord
        );
        assert_eq!(
            issues[1].relation,
            ForumImportDependencyRelation::TopicCategory
        );
        assert_eq!(
            issues[2].relation,
            ForumImportDependencyRelation::TopicMainPost
        );
        assert_eq!(issues[2].target.key, "post:101");
        assert_eq!(
            issues[3].relation,
            ForumImportDependencyRelation::AuthorUser
        );
        assert_eq!(
            issues[3].disposition,
            ForumImportDependencyDisposition::ExternalOwnerResolution
        );
        assert_eq!(
            issues[4].relation,
            ForumImportDependencyRelation::TopicMainPost
        );
        assert_eq!(
            issues[4].disposition,
            ForumImportDependencyDisposition::MismatchedBatchRecord
        );
        assert_eq!(issues[5].relation, ForumImportDependencyRelation::PostTopic);
        assert_eq!(
            issues[6].relation,
            ForumImportDependencyRelation::AuthorUser
        );
        assert_eq!(issues[6].target.key, "user:8");
    }

    #[test]
    fn reports_self_and_multi_node_category_cycles_in_source_order() {
        let batch = NodebbExportBatch {
            categories: vec![
                NodebbCategoryRecord {
                    cid: 1,
                    parent_cid: Some(1),
                    name: "Self".to_owned(),
                    description: None,
                    order: None,
                },
                NodebbCategoryRecord {
                    cid: 2,
                    parent_cid: Some(3),
                    name: "Cycle A".to_owned(),
                    description: None,
                    order: None,
                },
                NodebbCategoryRecord {
                    cid: 3,
                    parent_cid: Some(2),
                    name: "Cycle B".to_owned(),
                    description: None,
                    order: None,
                },
                NodebbCategoryRecord {
                    cid: 4,
                    parent_cid: Some(99),
                    name: "External parent".to_owned(),
                    description: None,
                    order: None,
                },
            ],
            ..Default::default()
        };

        let inspected = NodebbForumImportInspector.inspect_batch(&batch).unwrap();
        let issues = &inspected.unresolved_dependencies;
        assert_eq!(issues.len(), 4);
        assert_eq!(issues[0].owner.key, "category:1");
        assert_eq!(issues[0].target.key, "category:1");
        assert_eq!(
            issues[0].disposition,
            ForumImportDependencyDisposition::CyclicBatchRelation
        );
        assert_eq!(issues[1].owner.key, "category:2");
        assert_eq!(issues[1].target.key, "category:3");
        assert_eq!(
            issues[1].disposition,
            ForumImportDependencyDisposition::CyclicBatchRelation
        );
        assert_eq!(issues[2].owner.key, "category:3");
        assert_eq!(issues[2].target.key, "category:2");
        assert_eq!(
            issues[2].disposition,
            ForumImportDependencyDisposition::CyclicBatchRelation
        );
        assert_eq!(issues[3].owner.key, "category:4");
        assert_eq!(
            issues[3].disposition,
            ForumImportDependencyDisposition::MissingBatchRecord
        );
        assert!(!inspected.is_dependency_complete());
    }

    #[test]
    fn acyclic_in_batch_category_chain_is_dependency_complete() {
        let batch = NodebbExportBatch {
            categories: vec![
                NodebbCategoryRecord {
                    cid: 1,
                    parent_cid: None,
                    name: "Root".to_owned(),
                    description: None,
                    order: None,
                },
                NodebbCategoryRecord {
                    cid: 2,
                    parent_cid: Some(1),
                    name: "Child".to_owned(),
                    description: None,
                    order: None,
                },
                NodebbCategoryRecord {
                    cid: 3,
                    parent_cid: Some(2),
                    name: "Grandchild".to_owned(),
                    description: None,
                    order: None,
                },
            ],
            ..Default::default()
        };

        let inspected = NodebbForumImportInspector.inspect_batch(&batch).unwrap();
        assert!(inspected.is_dependency_complete());
    }

    #[test]
    fn complete_in_batch_relations_need_no_dependency_guessing() {
        let batch = NodebbExportBatch {
            categories: vec![NodebbCategoryRecord {
                cid: 2,
                parent_cid: None,
                name: "General".to_owned(),
                description: None,
                order: None,
            }],
            topics: vec![NodebbTopicRecord {
                tid: 10,
                cid: 2,
                uid: None,
                title: "Topic".to_owned(),
                slug: None,
                main_pid: Some(101),
                timestamp: None,
                pinned: false,
                locked: false,
            }],
            posts: vec![NodebbPostRecord {
                pid: 101,
                tid: 10,
                uid: None,
                content: "Body".to_owned(),
                timestamp: None,
                deleted: false,
            }],
        };

        let inspected = NodebbForumImportInspector.inspect_batch(&batch).unwrap();
        assert!(inspected.is_dependency_complete());
        assert_eq!(inspected.candidates.posts.len(), 1);
    }
}
