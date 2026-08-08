use chrono::Utc;
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use super::{
    IndexReplayOperatorContext, IndexReplayOperatorError, IndexReplayOperatorRuntime,
    IndexReplayShadowOperatorError,
    source_continuation_runtime::IndexSourceContinuationKeyringRuntime,
};
use crate::error::{Error as ServerError, Result};

#[derive(Debug, Error)]
pub enum IndexReplayShadowTransportError {
    #[error(transparent)]
    Authorization(#[from] IndexReplayOperatorError),
    #[error("Index replay Shadow continuation keyring is not configured")]
    ContinuationUnavailable,
    #[error("Index replay Shadow continuation keyring could not be resolved")]
    ContinuationKeyringUnavailable,
    #[error(transparent)]
    Continuation(#[from] rustok_index::IndexSourceContinuationError),
    #[error(transparent)]
    Request(#[from] rustok_index::IndexReplayDryRunError),
    #[error(transparent)]
    Shadow(#[from] IndexReplayShadowOperatorError),
}

/// Transport-safe Shadow replay result.
///
/// Raw source cursors and source ownership are intentionally absent. An unfinished run returns only
/// the authenticated confidential continuation token produced by the deployment-owned keyring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayShadowTransportOutcome {
    status: rustok_index::IndexReplayDryRunStatus,
    next_token: Option<rustok_index::IndexSourceContinuationToken>,
    pages_scanned: usize,
    mutation_count: usize,
    upsert_count: usize,
    delete_count: usize,
}

impl IndexReplayShadowTransportOutcome {
    fn from_dry_run(
        outcome: rustok_index::IndexReplayDryRunOutcome,
        next_token: Option<rustok_index::IndexSourceContinuationToken>,
    ) -> Self {
        Self {
            status: outcome.status(),
            next_token,
            pages_scanned: outcome.pages_scanned(),
            mutation_count: outcome.mutation_count(),
            upsert_count: outcome.upsert_count(),
            delete_count: outcome.delete_count(),
        }
    }

    pub fn status(&self) -> rustok_index::IndexReplayDryRunStatus {
        self.status
    }

    pub fn next_token(&self) -> Option<&rustok_index::IndexSourceContinuationToken> {
        self.next_token.as_ref()
    }

    pub fn pages_scanned(&self) -> usize {
        self.pages_scanned
    }

    pub fn mutation_count(&self) -> usize {
        self.mutation_count
    }

    pub fn upsert_count(&self) -> usize {
        self.upsert_count
    }

    pub fn delete_count(&self) -> usize {
        self.delete_count
    }
}

/// Server-owned transport adapter for schema-wide Shadow replay.
///
/// Authorization runs before continuation parsing. The adapter owns no database connection,
/// replay job, checkpoint, lease, cancellation state, retry state, scheduler, or worker handle.
#[derive(Clone)]
pub struct IndexReplayShadowTransportRuntime {
    operator: IndexReplayOperatorRuntime,
    sources: rustok_index::SharedIndexSourceRegistry,
    continuation: Option<IndexSourceContinuationKeyringRuntime>,
}

impl IndexReplayShadowTransportRuntime {
    fn new(
        operator: IndexReplayOperatorRuntime,
        sources: rustok_index::SharedIndexSourceRegistry,
        continuation: Option<IndexSourceContinuationKeyringRuntime>,
    ) -> Self {
        Self {
            operator,
            sources,
            continuation,
        }
    }

    pub async fn run_schema_wide(
        &self,
        context: IndexReplayOperatorContext,
        schema: rustok_index::SchemaRef,
        continuation: Option<&str>,
        page_limit: usize,
        max_pages: usize,
    ) -> std::result::Result<
        IndexReplayShadowTransportOutcome,
        IndexReplayShadowTransportError,
    > {
        context.authorize_for(context.tenant_id())?;
        let keyring = self
            .continuation
            .as_ref()
            .ok_or(IndexReplayShadowTransportError::ContinuationUnavailable)?;
        let scope = rustok_index::IndexSourceContinuationScope::from_registry(
            context.tenant_id(),
            schema.clone(),
            &self.sources,
        )?;
        let codec = keyring
            .resolve_codec()
            .await
            .map_err(|_| IndexReplayShadowTransportError::ContinuationKeyringUnavailable)?;
        let cursor = continuation
            .map(|encoded| codec.open_encoded(&scope, encoded, Utc::now()))
            .transpose()?;
        let request = rustok_index::IndexReplayDryRunRequest::new(
            context.tenant_id(),
            schema,
            cursor,
            page_limit,
            max_pages,
        )?;
        let outcome = self.operator.run_shadow(context, request).await?;
        let next_token = outcome
            .next_cursor()
            .map(|cursor| codec.seal(&scope, cursor, Utc::now(), keyring.lifetime()))
            .transpose()?;
        Ok(IndexReplayShadowTransportOutcome::from_dry_run(
            outcome,
            next_token,
        ))
    }
}

pub(super) fn materialize_index_replay_shadow_transport(
    extensions: &mut ModuleRuntimeExtensions,
    continuation: Option<IndexSourceContinuationKeyringRuntime>,
) -> Result<()> {
    if extensions.contains::<IndexReplayShadowTransportRuntime>() {
        return Err(ServerError::Message(
            "Index replay Shadow transport runtime is already materialized".to_string(),
        ));
    }
    let Some(operator) = extensions.get::<IndexReplayOperatorRuntime>().cloned() else {
        return Ok(());
    };
    let sources = extensions
        .get::<rustok_index::SharedIndexSourceRegistry>()
        .cloned()
        .ok_or_else(|| {
            ServerError::Message(
                "Index replay Shadow transport requires the shared source registry".to_string(),
            )
        })?;
    extensions.insert(IndexReplayShadowTransportRuntime::new(
        operator,
        sources,
        continuation,
    ));
    Ok(())
}
