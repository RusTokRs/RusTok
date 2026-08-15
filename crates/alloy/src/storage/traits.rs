use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::ScriptResult;
use crate::model::{
    AlloyImportedDraftCommand, AlloyImportedDraftResult, EventType, ReviewCommand, ReviewDecision,
    Script, ScriptDeletionCommand, ScriptId, ScriptSourceRevision, ScriptStatus, TestCommand,
    TestRun, TestRunClaim, TestRunCompletion,
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
