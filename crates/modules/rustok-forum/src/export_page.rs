use std::collections::BTreeSet;

use rustok_core::SecurityContext;
use thiserror::Error;
use uuid::Uuid;

use crate::services::{CategoryService, ReplyService, TopicService};

use super::super::{
    ForumExportFragment, ForumExportReadError, ForumExportReadTargetKind, ForumOwnerExportReader,
};
use super::{
    ForumExportSourceInventoryError, ForumExportSourceInventoryPage,
    ForumExportSourceInventoryRequest, ForumExportSourceInventoryService,
    ForumExportTargetPlanError, ForumExportTargetPlanner,
};

#[derive(Clone, Debug)]
pub struct ForumExportPage {
    pub source: ForumExportSourceInventoryPage,
    pub fragment: Option<ForumExportFragment>,
}

#[derive(Debug, Error)]
pub enum ForumExportPageComposeError {
    #[error(transparent)]
    Inventory(#[from] ForumExportSourceInventoryError),
    #[error(transparent)]
    Plan(#[from] ForumExportTargetPlanError),
    #[error(transparent)]
    Read(#[from] ForumExportReadError),
    #[error("Forum export page returned an empty source page while claiming more rows")]
    EmptyPageHasMore,
    #[error("Forum export page fragment tenant changed from {expected} to {actual}")]
    FragmentTenantChanged { expected: Uuid, actual: Uuid },
    #[error("Forum export page fragment contains records outside requested {kind} kind")]
    FragmentKindContaminated { kind: &'static str },
    #[error("Forum export page fragment source identities differ from the inventory {kind} page")]
    FragmentSourceIdentityChanged { kind: &'static str },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumExportPageComposer;

impl ForumExportPageComposer {
    pub async fn compose_page(
        &self,
        inventory: &ForumExportSourceInventoryService,
        categories: &CategoryService,
        topics: &TopicService,
        replies: &ReplyService,
        security: &SecurityContext,
        request: &ForumExportSourceInventoryRequest,
    ) -> Result<ForumExportPage, ForumExportPageComposeError> {
        let source = inventory.list_page(security, request).await?;
        let Some(plan_request) = source.target_plan_request() else {
            if source.has_more {
                return Err(ForumExportPageComposeError::EmptyPageHasMore);
            }
            return Ok(ForumExportPage {
                source,
                fragment: None,
            });
        };

        let read_batch = ForumExportTargetPlanner
            .plan_fragment(categories, topics, replies, security, &plan_request)
            .await?;
        let fragment = ForumOwnerExportReader
            .read_fragment(categories, topics, replies, security, &read_batch)
            .await?;
        validate_fragment(&source, &fragment)?;

        Ok(ForumExportPage {
            source,
            fragment: Some(fragment),
        })
    }
}

fn validate_fragment(
    source: &ForumExportSourceInventoryPage,
    fragment: &ForumExportFragment,
) -> Result<(), ForumExportPageComposeError> {
    if fragment.tenant_id != source.tenant_id {
        return Err(ForumExportPageComposeError::FragmentTenantChanged {
            expected: source.tenant_id,
            actual: fragment.tenant_id,
        });
    }

    let actual_ids = match source.kind {
        ForumExportReadTargetKind::Category => {
            if !fragment.topics.is_empty() || !fragment.replies.is_empty() {
                return Err(ForumExportPageComposeError::FragmentKindContaminated {
                    kind: kind_label(source.kind),
                });
            }
            unique_ids(fragment.categories.iter().map(|record| record.id))
        }
        ForumExportReadTargetKind::Topic => {
            if !fragment.categories.is_empty() || !fragment.replies.is_empty() {
                return Err(ForumExportPageComposeError::FragmentKindContaminated {
                    kind: kind_label(source.kind),
                });
            }
            unique_ids(fragment.topics.iter().map(|record| record.id))
        }
        ForumExportReadTargetKind::Reply => {
            if !fragment.categories.is_empty() || !fragment.topics.is_empty() {
                return Err(ForumExportPageComposeError::FragmentKindContaminated {
                    kind: kind_label(source.kind),
                });
            }
            unique_ids(fragment.replies.iter().map(|record| record.id))
        }
    };

    if actual_ids != source.ids {
        return Err(ForumExportPageComposeError::FragmentSourceIdentityChanged {
            kind: kind_label(source.kind),
        });
    }
    Ok(())
}

fn unique_ids(ids: impl IntoIterator<Item = Uuid>) -> Vec<Uuid> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for id in ids {
        if seen.insert(id) {
            unique.push(id);
        }
    }
    unique
}

const fn kind_label(kind: ForumExportReadTargetKind) -> &'static str {
    match kind {
        ForumExportReadTargetKind::Category => "category",
        ForumExportReadTargetKind::Topic => "topic",
        ForumExportReadTargetKind::Reply => "reply",
    }
}
