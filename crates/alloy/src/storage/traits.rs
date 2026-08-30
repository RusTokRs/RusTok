use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::ScriptResult;
use crate::model::{
    AlloyImportedDraftCommand, AlloyImportedDraftResult, EventType, ReviewCommand, ReviewDecision,
    RustComponentCandidate, RustComponentCandidateBuild, RustComponentCandidateCommand,
    RustComponentCandidateReview, RustComponentCandidateReviewCommand, Script,
    ScriptDeletionCommand, ScriptId, ScriptSourceRevision, ScriptStatus, TestCommand, TestRun,
    TestRunClaim, TestRunCompletion,
};

#[derive(Clone)]
pub enum ScriptQuery {
    ById(ScriptId),
    ByName(String),
    ByEvent {
        entity_type: String,
        event: EventType,
    },
    ByApiPath(String),
    Scheduled,
    ByStatus(ScriptStatus),
    All,
}

pub struct ScriptPage {
    pub items: Vec<Script>,
    pub total: u64,
}

#[async_trait]
pub trait ScriptRegistry: Send + Sync {
    async fn find(&self, query: ScriptQuery) -> ScriptResult<Vec<Script>>;
    async fn find_paginated(
        &self,
        query: ScriptQuery,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<ScriptPage>;
    async fn get(&self, id: ScriptId) -> ScriptResult<Script>;
    async fn get_source_revision(
        &self,
        id: ScriptId,
        revision: u32,
    ) -> ScriptResult<ScriptSourceRevision>;
    async fn list_source_revisions(&self, id: ScriptId) -> ScriptResult<Vec<ScriptSourceRevision>>;
    async fn review(&self, command: ReviewCommand) -> ScriptResult<ReviewDecision>;
    async fn list_reviews(&self, id: ScriptId, revision: u32) -> ScriptResult<Vec<ReviewDecision>>;
    async fn claim_test_run(&self, command: TestCommand) -> ScriptResult<TestRunClaim>;
    async fn complete_test_run(
        &self,
        run_id: uuid::Uuid,
        lease_token: uuid::Uuid,
        completion: TestRunCompletion,
    ) -> ScriptResult<TestRun>;
    async fn get_by_name(&self, name: &str) -> ScriptResult<Script>;
    async fn import_published_release(
        &self,
        command: AlloyImportedDraftCommand,
    ) -> ScriptResult<AlloyImportedDraftResult>;
    /// Persists one data-only Rust Component rewrite candidate after proving
    /// its Rhai parent revision is current, approved, and release-pinned.
    async fn create_component_candidate(
        &self,
        command: RustComponentCandidateCommand,
    ) -> ScriptResult<RustComponentCandidate>;
    /// Reads one immutable candidate only while its owning draft remains
    /// visible within the caller's tenant scope.
    async fn get_component_candidate(&self, id: uuid::Uuid)
    -> ScriptResult<RustComponentCandidate>;
    /// Appends one authorized state transition over an immutable Component
    /// candidate. Approval is required before source preparation can begin.
    async fn review_component_candidate(
        &self,
        command: RustComponentCandidateReviewCommand,
    ) -> ScriptResult<RustComponentCandidateReview>;
    async fn list_component_candidate_reviews(
        &self,
        candidate_id: uuid::Uuid,
    ) -> ScriptResult<Vec<RustComponentCandidateReview>>;
    /// Persists one owner build receipt only after the external module owner
    /// has accepted the immutable prepared archive.
    async fn record_component_candidate_build(
        &self,
        build: RustComponentCandidateBuild,
    ) -> ScriptResult<RustComponentCandidateBuild>;
    async fn get_component_candidate_build(
        &self,
        candidate_id: uuid::Uuid,
        idempotency_key: uuid::Uuid,
    ) -> ScriptResult<Option<RustComponentCandidateBuild>>;
    async fn save(&self, script: Script) -> ScriptResult<Script>;
    async fn delete(&self, command: ScriptDeletionCommand) -> ScriptResult<()>;
    /// Returns source-free retention state for a deleted draft that is still
    /// under owner control.
    async fn get_deleted_evidence_retention(
        &self,
        id: ScriptId,
    ) -> ScriptResult<crate::model::ScriptEvidenceRetentionState>;
    /// Applies one owner-attributable, idempotent retention transition.
    async fn update_deleted_evidence_retention(
        &self,
        command: crate::model::ScriptEvidenceRetentionCommand,
    ) -> ScriptResult<crate::model::ScriptEvidenceRetentionState>;
    /// Collects deleted evidence only when its persisted retention policy has
    /// expired. Durable implementations preserve a content-free purge receipt.
    async fn purge_expired_evidence(&self, now: DateTime<Utc>, limit: u16) -> ScriptResult<u64>;
    async fn set_status(&self, id: ScriptId, status: ScriptStatus) -> ScriptResult<()>;
    async fn record_error(&self, id: ScriptId) -> ScriptResult<bool>;
    async fn reset_errors(&self, id: ScriptId) -> ScriptResult<()>;
}
