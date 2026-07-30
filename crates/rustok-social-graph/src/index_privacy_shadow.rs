use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_index::SharedIndexQueryRuntime;

use crate::{
    IndexSocialGraphPrivacyReadPort, SocialGraphFollowBatchRequest, SocialGraphFollowBatchResult,
    SocialGraphPairRequest, SocialGraphPrivacyReadPort,
};

const OPERATION_BLOCKS_BETWEEN: &str = "blocks_between";
const OPERATION_SOURCE_MUTES_TARGET: &str = "source_mutes_target";
const OPERATION_SOURCE_FOLLOWS_TARGET: &str = "source_follows_target";
const OPERATION_SOURCE_FOLLOWS_TARGETS: &str = "source_follows_targets";

/// Non-authoritative parity observer for Social Graph privacy reads.
///
/// The owner port always determines the returned result. Index is queried only after the
/// owner succeeds, and projection errors or mismatches are recorded without changing policy.
#[derive(Clone)]
pub struct IndexShadowSocialGraphPrivacyReadPort {
    authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
    projected: IndexSocialGraphPrivacyReadPort,
}

impl IndexShadowSocialGraphPrivacyReadPort {
    pub fn new(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        runtime: SharedIndexQueryRuntime,
    ) -> Self {
        Self {
            authoritative,
            projected: IndexSocialGraphPrivacyReadPort::new(runtime),
        }
    }
}

#[async_trait]
impl SocialGraphPrivacyReadPort for IndexShadowSocialGraphPrivacyReadPort {
    async fn blocks_between(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let authoritative = self
            .authoritative
            .blocks_between(context.clone(), request)
            .await?;
        observe_bool(
            OPERATION_BLOCKS_BETWEEN,
            authoritative,
            self.projected.blocks_between(context, request).await,
        );
        Ok(authoritative)
    }

    async fn source_mutes_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let authoritative = self
            .authoritative
            .source_mutes_target(context.clone(), request)
            .await?;
        observe_bool(
            OPERATION_SOURCE_MUTES_TARGET,
            authoritative,
            self.projected.source_mutes_target(context, request).await,
        );
        Ok(authoritative)
    }

    async fn source_follows_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        let authoritative = self
            .authoritative
            .source_follows_target(context.clone(), request)
            .await?;
        observe_bool(
            OPERATION_SOURCE_FOLLOWS_TARGET,
            authoritative,
            self.projected.source_follows_target(context, request).await,
        );
        Ok(authoritative)
    }

    async fn source_follows_targets(
        &self,
        context: PortContext,
        request: SocialGraphFollowBatchRequest,
    ) -> Result<SocialGraphFollowBatchResult, PortError> {
        let authoritative = self
            .authoritative
            .source_follows_targets(context.clone(), request.clone())
            .await?;
        observe_batch(
            OPERATION_SOURCE_FOLLOWS_TARGETS,
            &authoritative,
            self.projected.source_follows_targets(context, request).await,
        );
        Ok(authoritative)
    }
}

fn observe_bool(operation: &'static str, authoritative: bool, projected: Result<bool, PortError>) {
    match projected {
        Ok(projected) if projected == authoritative => {
            tracing::debug!(
                operation,
                "Social Graph Index privacy shadow matched authoritative owner result"
            );
        }
        Ok(projected) => {
            tracing::warn!(
                operation,
                authoritative,
                projected,
                "Social Graph Index privacy shadow mismatch"
            );
        }
        Err(error) => {
            tracing::warn!(
                operation,
                code = %error.code,
                retryable = error.retryable,
                "Social Graph Index privacy shadow read failed"
            );
        }
    }
}

fn observe_batch(
    operation: &'static str,
    authoritative: &SocialGraphFollowBatchResult,
    projected: Result<SocialGraphFollowBatchResult, PortError>,
) {
    match projected {
        Ok(projected) if projected == *authoritative => {
            tracing::debug!(
                operation,
                authoritative_count = authoritative.followed_target_user_ids.len(),
                "Social Graph Index privacy shadow matched authoritative owner result"
            );
        }
        Ok(projected) => {
            tracing::warn!(
                operation,
                authoritative_count = authoritative.followed_target_user_ids.len(),
                projected_count = projected.followed_target_user_ids.len(),
                "Social Graph Index privacy shadow mismatch"
            );
        }
        Err(error) => {
            tracing::warn!(
                operation,
                code = %error.code,
                retryable = error.retryable,
                "Social Graph Index privacy shadow read failed"
            );
        }
    }
}
