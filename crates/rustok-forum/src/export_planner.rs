use std::collections::BTreeSet;

use rustok_api::{Action, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::{PermissionScope, SecurityContext};
use thiserror::Error;
use uuid::Uuid;

use crate::error::ForumError;
use crate::services::{CategoryService, ReplyService, TopicService};

use super::{
    ForumExportReadBatch, ForumExportReadTarget, ForumExportReadTargetKind,
    MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT,
};

pub const MAX_FORUM_EXPORT_PLAN_SOURCE_IDS_PER_FRAGMENT: usize =
    MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT;

#[derive(Clone, Debug)]
pub struct ForumExportTargetPlanRequest {
    pub tenant_id: Uuid,
    pub category_ids: Vec<Uuid>,
    pub topic_ids: Vec<Uuid>,
    pub reply_ids: Vec<Uuid>,
}

#[derive(Debug, Error)]
pub enum ForumExportTargetPlanError {
    #[error("Forum export target planning requires an authenticated operator context")]
    OperatorContextRequired,
    #[error("Forum export target planning requires {resource}:manage")]
    ManagePermissionRequired { resource: &'static str },
    #[error("Forum export target planning requires a non-nil tenant id")]
    NilTenantId,
    #[error("Forum export target planning requires at least one source id")]
    EmptySources,
    #[error("Forum export target planning exceeds {max} source ids: {actual}")]
    TooManySourceIds { max: usize, actual: usize },
    #[error("Forum export target planning requires a non-nil {kind} id")]
    NilSourceId { kind: &'static str },
    #[error("Forum export target planning repeats {kind} id {id}")]
    DuplicateSourceId { kind: &'static str, id: Uuid },
    #[error(
        "Forum export {kind} locale enumeration changed cardinality: expected {expected}, actual {actual}"
    )]
    LocaleFactCountChanged {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error(
        "Forum export {kind} locale enumeration changed identity from {expected_id} to {actual_id}"
    )]
    LocaleFactIdentityChanged {
        kind: &'static str,
        expected_id: Uuid,
        actual_id: Uuid,
    },
    #[error("Forum export {kind} {id} locale enumeration returned no stored locales")]
    EmptyLocaleFacts { kind: &'static str, id: Uuid },
    #[error("Forum export {kind} {id} locale enumeration returned invalid locale {locale}")]
    InvalidLocaleFact {
        kind: &'static str,
        id: Uuid,
        locale: String,
    },
    #[error("Forum export {kind} {id} locale enumeration repeats normalized locale {locale}")]
    DuplicateLocaleFact {
        kind: &'static str,
        id: Uuid,
        locale: String,
    },
    #[error("Forum export target planning exceeds {max} localized targets: {actual}")]
    TooManyTargets { max: usize, actual: usize },
    #[error(transparent)]
    Owner(#[from] ForumError),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumExportTargetPlanner;

impl ForumExportTargetPlanner {
    pub async fn plan_fragment(
        &self,
        categories: &CategoryService,
        topics: &TopicService,
        replies: &ReplyService,
        security: &SecurityContext,
        request: &ForumExportTargetPlanRequest,
    ) -> Result<ForumExportReadBatch, ForumExportTargetPlanError> {
        validate_plan_request(security, request)?;
        require_requested_manage_scopes(security, request)?;

        let mut targets = Vec::new();
        if !request.category_ids.is_empty() {
            let facts = categories
                .available_locales_for_categories(
                    request.tenant_id,
                    security.clone(),
                    &request.category_ids,
                )
                .await?;
            append_locale_facts(
                ForumExportReadTargetKind::Category,
                &request.category_ids,
                facts,
                &mut targets,
            )?;
        }
        if !request.topic_ids.is_empty() {
            let facts = topics
                .available_locales_for_topics(
                    request.tenant_id,
                    security.clone(),
                    &request.topic_ids,
                )
                .await?;
            append_locale_facts(
                ForumExportReadTargetKind::Topic,
                &request.topic_ids,
                facts,
                &mut targets,
            )?;
        }
        if !request.reply_ids.is_empty() {
            let facts = replies
                .available_locales_for_replies(
                    request.tenant_id,
                    security.clone(),
                    &request.reply_ids,
                )
                .await?;
            append_locale_facts(
                ForumExportReadTargetKind::Reply,
                &request.reply_ids,
                facts,
                &mut targets,
            )?;
        }

        Ok(ForumExportReadBatch {
            tenant_id: request.tenant_id,
            targets,
        })
    }
}

fn validate_plan_request(
    security: &SecurityContext,
    request: &ForumExportTargetPlanRequest,
) -> Result<(), ForumExportTargetPlanError> {
    if security.is_public_read() {
        return Err(ForumExportTargetPlanError::OperatorContextRequired);
    }
    if request.tenant_id.is_nil() {
        return Err(ForumExportTargetPlanError::NilTenantId);
    }

    let source_count = request
        .category_ids
        .len()
        .saturating_add(request.topic_ids.len())
        .saturating_add(request.reply_ids.len());
    if source_count == 0 {
        return Err(ForumExportTargetPlanError::EmptySources);
    }
    if source_count > MAX_FORUM_EXPORT_PLAN_SOURCE_IDS_PER_FRAGMENT {
        return Err(ForumExportTargetPlanError::TooManySourceIds {
            max: MAX_FORUM_EXPORT_PLAN_SOURCE_IDS_PER_FRAGMENT,
            actual: source_count,
        });
    }

    validate_source_ids("category", &request.category_ids)?;
    validate_source_ids("topic", &request.topic_ids)?;
    validate_source_ids("reply", &request.reply_ids)?;
    Ok(())
}

fn require_requested_manage_scopes(
    security: &SecurityContext,
    request: &ForumExportTargetPlanRequest,
) -> Result<(), ForumExportTargetPlanError> {
    for (requested, resource, label) in [
        (
            !request.category_ids.is_empty(),
            Resource::ForumCategories,
            "forum_categories",
        ),
        (
            !request.topic_ids.is_empty(),
            Resource::ForumTopics,
            "forum_topics",
        ),
        (
            !request.reply_ids.is_empty(),
            Resource::ForumReplies,
            "forum_replies",
        ),
    ] {
        if requested
            && matches!(
                security.get_scope(resource, Action::Manage),
                PermissionScope::None
            )
        {
            return Err(ForumExportTargetPlanError::ManagePermissionRequired { resource: label });
        }
    }
    Ok(())
}

fn validate_source_ids(kind: &'static str, ids: &[Uuid]) -> Result<(), ForumExportTargetPlanError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.is_nil() {
            return Err(ForumExportTargetPlanError::NilSourceId { kind });
        }
        if !seen.insert(*id) {
            return Err(ForumExportTargetPlanError::DuplicateSourceId { kind, id: *id });
        }
    }
    Ok(())
}

fn append_locale_facts(
    kind: ForumExportReadTargetKind,
    expected_ids: &[Uuid],
    facts: Vec<(Uuid, Vec<String>)>,
    targets: &mut Vec<ForumExportReadTarget>,
) -> Result<(), ForumExportTargetPlanError> {
    let label = kind_label(kind);
    if facts.len() != expected_ids.len() {
        return Err(ForumExportTargetPlanError::LocaleFactCountChanged {
            kind: label,
            expected: expected_ids.len(),
            actual: facts.len(),
        });
    }

    for (expected_id, (actual_id, locales)) in expected_ids.iter().copied().zip(facts) {
        if actual_id != expected_id {
            return Err(ForumExportTargetPlanError::LocaleFactIdentityChanged {
                kind: label,
                expected_id,
                actual_id,
            });
        }
        if locales.is_empty() {
            return Err(ForumExportTargetPlanError::EmptyLocaleFacts {
                kind: label,
                id: actual_id,
            });
        }

        let mut seen_locales = BTreeSet::new();
        for locale in locales {
            let normalized = normalize_locale_code(&locale).ok_or_else(|| {
                ForumExportTargetPlanError::InvalidLocaleFact {
                    kind: label,
                    id: actual_id,
                    locale: locale.clone(),
                }
            })?;
            if !seen_locales.insert(normalized.clone()) {
                return Err(ForumExportTargetPlanError::DuplicateLocaleFact {
                    kind: label,
                    id: actual_id,
                    locale: normalized,
                });
            }

            let actual = targets.len().saturating_add(1);
            if actual > MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT {
                return Err(ForumExportTargetPlanError::TooManyTargets {
                    max: MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT,
                    actual,
                });
            }
            targets.push(ForumExportReadTarget {
                kind,
                id: actual_id,
                locale: normalized,
            });
        }
    }
    Ok(())
}

const fn kind_label(kind: ForumExportReadTargetKind) -> &'static str {
    match kind {
        ForumExportReadTargetKind::Category => "category",
        ForumExportReadTargetKind::Topic => "topic",
        ForumExportReadTargetKind::Reply => "reply",
    }
}

#[path = "export_inventory.rs"]
mod inventory;
pub use inventory::*;

#[path = "export_page.rs"]
mod page;
pub use page::*;
