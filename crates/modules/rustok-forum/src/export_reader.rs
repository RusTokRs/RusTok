use std::collections::BTreeSet;

use rustok_api::{Action, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::{PermissionScope, SecurityContext};
use thiserror::Error;
use uuid::Uuid;

use crate::error::ForumError;
use crate::services::{CategoryService, ReplyService, TopicService};

use super::{
    ForumExportFragment, ForumExportMappingError, ForumExportOwnerViewBatch,
    ForumOwnerExportMapper, MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT,
};

pub const MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT: usize =
    MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ForumExportReadTargetKind {
    Category,
    Topic,
    Reply,
}

impl ForumExportReadTargetKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::Topic => "topic",
            Self::Reply => "reply",
        }
    }

    const fn permission_label(self) -> &'static str {
        match self {
            Self::Category => "forum_categories",
            Self::Topic => "forum_topics",
            Self::Reply => "forum_replies",
        }
    }

    const fn resource(self) -> Resource {
        match self {
            Self::Category => Resource::ForumCategories,
            Self::Topic => Resource::ForumTopics,
            Self::Reply => Resource::ForumReplies,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumExportReadTarget {
    pub kind: ForumExportReadTargetKind,
    pub id: Uuid,
    pub locale: String,
}

#[derive(Clone, Debug)]
pub struct ForumExportReadBatch {
    pub tenant_id: Uuid,
    pub targets: Vec<ForumExportReadTarget>,
}

#[derive(Debug, Error)]
pub enum ForumExportReadError {
    #[error("Forum export owner read requires an authenticated operator context")]
    OperatorContextRequired,
    #[error("Forum export owner read requires at least one localized target")]
    EmptyTargets,
    #[error("Forum export owner read requires {resource}:manage")]
    ManagePermissionRequired { resource: &'static str },
    #[error("Forum export owner read requires a non-nil tenant id")]
    NilTenantId,
    #[error("Forum export owner read exceeds {max} localized targets: {actual}")]
    TooManyTargets { max: usize, actual: usize },
    #[error("Forum export {kind} target requires a non-nil id")]
    NilTargetId { kind: &'static str },
    #[error("Forum export {kind} {id} has invalid locale {locale}")]
    InvalidLocale {
        kind: &'static str,
        id: Uuid,
        locale: String,
    },
    #[error("Forum export {kind} {id} repeats locale {locale}")]
    DuplicateTarget {
        kind: &'static str,
        id: Uuid,
        locale: String,
    },
    #[error("Forum export {kind} target {requested_id} resolved to different owner id {actual_id}")]
    OwnerIdentityChanged {
        kind: &'static str,
        requested_id: Uuid,
        actual_id: Uuid,
    },
    #[error(
        "Forum export {kind} {id} requested exact locale {requested_locale} but owner resolved {effective_locale}"
    )]
    LocaleNotStored {
        kind: &'static str,
        id: Uuid,
        requested_locale: String,
        effective_locale: String,
    },
    #[error(transparent)]
    Owner(#[from] ForumError),
    #[error(transparent)]
    Mapping(#[from] ForumExportMappingError),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumOwnerExportReader;

impl ForumOwnerExportReader {
    pub async fn read_fragment(
        &self,
        categories: &CategoryService,
        topics: &TopicService,
        replies: &ReplyService,
        security: &SecurityContext,
        batch: &ForumExportReadBatch,
    ) -> Result<ForumExportFragment, ForumExportReadError> {
        let targets = validate_read_batch(security, batch)?;
        require_requested_manage_scopes(security, &targets)?;

        let mut owner_views = ForumExportOwnerViewBatch {
            tenant_id: batch.tenant_id,
            categories: Vec::new(),
            topics: Vec::new(),
            replies: Vec::new(),
        };

        for target in targets {
            match target.kind {
                ForumExportReadTargetKind::Category => {
                    let response = categories
                        .get_with_locale_fallback(
                            batch.tenant_id,
                            security.clone(),
                            target.id,
                            &target.locale,
                            None,
                        )
                        .await?;
                    ensure_exact_owner_view(
                        target.kind,
                        target.id,
                        &target.locale,
                        response.id,
                        &response.effective_locale,
                    )?;
                    owner_views.categories.push(response);
                }
                ForumExportReadTargetKind::Topic => {
                    let response = topics
                        .get_with_locale_fallback(
                            batch.tenant_id,
                            security.clone(),
                            target.id,
                            &target.locale,
                            None,
                        )
                        .await?;
                    ensure_exact_owner_view(
                        target.kind,
                        target.id,
                        &target.locale,
                        response.id,
                        &response.effective_locale,
                    )?;
                    owner_views.topics.push(response);
                }
                ForumExportReadTargetKind::Reply => {
                    let response = replies
                        .get_with_locale_fallback(
                            batch.tenant_id,
                            security.clone(),
                            target.id,
                            &target.locale,
                            None,
                        )
                        .await?;
                    ensure_exact_owner_view(
                        target.kind,
                        target.id,
                        &target.locale,
                        response.id,
                        &response.effective_locale,
                    )?;
                    owner_views.replies.push(response);
                }
            }
        }

        Ok(ForumOwnerExportMapper.map_fragment(&owner_views)?)
    }
}

fn validate_read_batch(
    security: &SecurityContext,
    batch: &ForumExportReadBatch,
) -> Result<Vec<ForumExportReadTarget>, ForumExportReadError> {
    if security.is_public_read() {
        return Err(ForumExportReadError::OperatorContextRequired);
    }
    if batch.tenant_id.is_nil() {
        return Err(ForumExportReadError::NilTenantId);
    }
    if batch.targets.is_empty() {
        return Err(ForumExportReadError::EmptyTargets);
    }
    if batch.targets.len() > MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT {
        return Err(ForumExportReadError::TooManyTargets {
            max: MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT,
            actual: batch.targets.len(),
        });
    }

    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(batch.targets.len());
    for target in &batch.targets {
        let kind = target.kind.label();
        if target.id.is_nil() {
            return Err(ForumExportReadError::NilTargetId { kind });
        }
        let locale = normalize_locale_code(&target.locale).ok_or_else(|| {
            ForumExportReadError::InvalidLocale {
                kind,
                id: target.id,
                locale: target.locale.clone(),
            }
        })?;
        if !seen.insert((target.kind, target.id, locale.clone())) {
            return Err(ForumExportReadError::DuplicateTarget {
                kind,
                id: target.id,
                locale,
            });
        }
        normalized.push(ForumExportReadTarget {
            kind: target.kind,
            id: target.id,
            locale,
        });
    }
    Ok(normalized)
}

fn require_requested_manage_scopes(
    security: &SecurityContext,
    targets: &[ForumExportReadTarget],
) -> Result<(), ForumExportReadError> {
    let kinds = targets
        .iter()
        .map(|target| target.kind)
        .collect::<BTreeSet<_>>();
    for kind in kinds {
        if matches!(
            security.get_scope(kind.resource(), Action::Manage),
            PermissionScope::None
        ) {
            return Err(ForumExportReadError::ManagePermissionRequired {
                resource: kind.permission_label(),
            });
        }
    }
    Ok(())
}

fn ensure_exact_owner_view(
    kind: ForumExportReadTargetKind,
    requested_id: Uuid,
    requested_locale: &str,
    actual_id: Uuid,
    effective_locale: &str,
) -> Result<(), ForumExportReadError> {
    if actual_id != requested_id {
        return Err(ForumExportReadError::OwnerIdentityChanged {
            kind: kind.label(),
            requested_id,
            actual_id,
        });
    }
    let effective_locale =
        normalize_locale_code(effective_locale).unwrap_or_else(|| effective_locale.to_owned());
    if effective_locale != requested_locale {
        return Err(ForumExportReadError::LocaleNotStored {
            kind: kind.label(),
            id: requested_id,
            requested_locale: requested_locale.to_owned(),
            effective_locale,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_owner_view_rejects_fallback_and_identity_redirects() {
        let id = Uuid::new_v4();
        assert!(matches!(
            ensure_exact_owner_view(ForumExportReadTargetKind::Reply, id, "de", id, "en",),
            Err(ForumExportReadError::LocaleNotStored { .. })
        ));

        assert!(matches!(
            ensure_exact_owner_view(
                ForumExportReadTargetKind::Topic,
                id,
                "en",
                Uuid::new_v4(),
                "en",
            ),
            Err(ForumExportReadError::OwnerIdentityChanged { .. })
        ));
    }

    #[test]
    fn target_kind_maps_to_exact_manage_resource() {
        assert_eq!(
            ForumExportReadTargetKind::Category.resource(),
            Resource::ForumCategories
        );
        assert_eq!(
            ForumExportReadTargetKind::Topic.resource(),
            Resource::ForumTopics
        );
        assert_eq!(
            ForumExportReadTargetKind::Reply.resource(),
            Resource::ForumReplies
        );
        assert_eq!(
            ForumExportReadTargetKind::Reply.permission_label(),
            "forum_replies"
        );
    }
}
