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
    projected: Arc<dyn SocialGraphPrivacyReadPort>,
}

impl IndexShadowSocialGraphPrivacyReadPort {
    pub fn new(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        runtime: SharedIndexQueryRuntime,
    ) -> Self {
        Self {
            authoritative,
            projected: Arc::new(IndexSocialGraphPrivacyReadPort::new(runtime)),
        }
    }

    #[cfg(test)]
    fn from_ports(
        authoritative: Arc<dyn SocialGraphPrivacyReadPort>,
        projected: Arc<dyn SocialGraphPrivacyReadPort>,
    ) -> Self {
        Self {
            authoritative,
            projected,
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;
    use rustok_api::PortActor;
    use uuid::Uuid;

    use super::*;

    #[derive(Clone)]
    struct FixedPrivacyPort {
        boolean: bool,
        batch: Vec<Uuid>,
        fail: bool,
    }

    #[async_trait]
    impl SocialGraphPrivacyReadPort for FixedPrivacyPort {
        async fn blocks_between(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            self.boolean_result()
        }

        async fn source_mutes_target(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            self.boolean_result()
        }

        async fn source_follows_target(
            &self,
            _context: PortContext,
            _request: SocialGraphPairRequest,
        ) -> Result<bool, PortError> {
            self.boolean_result()
        }

        async fn source_follows_targets(
            &self,
            _context: PortContext,
            _request: SocialGraphFollowBatchRequest,
        ) -> Result<SocialGraphFollowBatchResult, PortError> {
            if self.fail {
                Err(projected_failure())
            } else {
                Ok(SocialGraphFollowBatchResult {
                    followed_target_user_ids: self.batch.clone(),
                })
            }
        }
    }

    impl FixedPrivacyPort {
        fn boolean_result(&self) -> Result<bool, PortError> {
            if self.fail {
                Err(projected_failure())
            } else {
                Ok(self.boolean)
            }
        }
    }

    fn context() -> PortContext {
        PortContext::new(
            Uuid::from_u128(1).to_string(),
            PortActor::service("privacy-shadow-test"),
            "und",
            "privacy-shadow-test-correlation",
        )
        .with_deadline(Duration::from_secs(1))
    }

    fn pair() -> SocialGraphPairRequest {
        SocialGraphPairRequest {
            source_user_id: Uuid::from_u128(2),
            target_user_id: Uuid::from_u128(3),
        }
    }

    fn projected_failure() -> PortError {
        PortError::unavailable(
            "social_graph.index_privacy_unavailable",
            "projected privacy state is unavailable",
        )
    }

    #[tokio::test]
    async fn mismatch_returns_authoritative_boolean() {
        let shadow = IndexShadowSocialGraphPrivacyReadPort::from_ports(
            Arc::new(FixedPrivacyPort {
                boolean: true,
                batch: Vec::new(),
                fail: false,
            }),
            Arc::new(FixedPrivacyPort {
                boolean: false,
                batch: Vec::new(),
                fail: false,
            }),
        );

        assert!(shadow.blocks_between(context(), pair()).await.unwrap());
    }

    #[tokio::test]
    async fn projected_error_returns_authoritative_batch() {
        let expected = vec![Uuid::from_u128(4), Uuid::from_u128(5)];
        let shadow = IndexShadowSocialGraphPrivacyReadPort::from_ports(
            Arc::new(FixedPrivacyPort {
                boolean: true,
                batch: expected.clone(),
                fail: false,
            }),
            Arc::new(FixedPrivacyPort {
                boolean: false,
                batch: Vec::new(),
                fail: true,
            }),
        );

        let result = shadow
            .source_follows_targets(
                context(),
                SocialGraphFollowBatchRequest {
                    source_user_id: Uuid::from_u128(2),
                    target_user_ids: vec![Uuid::from_u128(4), Uuid::from_u128(5)],
                },
            )
            .await
            .unwrap();

        assert_eq!(result.followed_target_user_ids, expected);
    }
}
