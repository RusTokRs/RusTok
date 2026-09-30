use chrono::{DateTime, Utc};
use rustok_core::RetentionPolicy;
use rustok_modules::{ArtifactReleaseRef, ModuleBuildPublicationReceipt};
use rustok_sandbox::LocalSandboxScenarioComparison;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveValue, QueryFilter};
use std::str::FromStr;

use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    EventType, HttpMethod, ReviewDecision, ReviewStatus, RhaiWorkspace, RustComponentCandidate,
    RustComponentCandidateBuild, RustComponentCandidateBuildExecution, RustComponentCandidateReview,
    RustComponentWorkspace, Script, ScriptDeletionCommand, ScriptDeletionError,
    ScriptEvidenceRetentionError, ScriptEvidenceRetentionState, ScriptId, ScriptSourceRevision,
    ScriptStatus, ScriptTrigger, TestRun, TestRunStatus,
};
use crate::storage::ScriptQuery;

use super::entities::{
    component_candidate, component_candidate_build, component_candidate_build_execution,
    component_candidate_review, draft_retention_receipt, draft_review, draft_revision,
    draft_test_run, draft_tombstone, ActiveModel, Column, Entity, Model,
};

pub(crate) fn retention_state_from_parts(
    script_id: ScriptId,
    tenant_id: Uuid,
    deletion_request_digest: String,
    retention_policy: String,
    retain_until: Option<DateTime<Utc>>,
    retention_revision: i32,
) -> ScriptResult<ScriptEvidenceRetentionState> {
    let policy = RetentionPolicy::from_str(&retention_policy)
        .map_err(|_| ScriptEvidenceRetentionError::InvalidStoredState)?;
    let retention_revision = u32::try_from(retention_revision)
        .map_err(|_| ScriptEvidenceRetentionError::InvalidStoredState)?;
    ScriptEvidenceRetentionState::new(
        script_id,
        tenant_id,
        deletion_request_digest,
        policy,
        retain_until,
        retention_revision,
    )
    .map_err(ScriptError::from)
}

pub(crate) fn retention_state_from_tombstone(
    tombstone: &draft_tombstone::Model,
) -> ScriptResult<ScriptEvidenceRetentionState> {
    retention_state_from_parts(
        tombstone.id,
        tombstone.tenant_id,
        tombstone.request_digest.clone(),
        tombstone.retention_policy.clone(),
        tombstone.retain_until,
        tombstone.retention_revision,
    )
}

pub(crate) fn retention_state_from_receipt(
    receipt: &draft_retention_receipt::Model,
) -> ScriptResult<ScriptEvidenceRetentionState> {
    retention_state_from_parts(
        receipt.script_id,
        receipt.tenant_id,
        receipt.deletion_request_digest.clone(),
        receipt.retention_policy.clone(),
        receipt.retain_until,
        receipt.retention_revision,
    )
}

pub(crate) fn replay_deleted_command(
    existing: &draft_tombstone::Model,
    command: &ScriptDeletionCommand,
    request_digest: &str,
) -> ScriptResult<()> {
    if existing.request_digest == request_digest {
        return Ok(());
    }
    if existing.idempotency_key == command.idempotency_key {
        return Err(ScriptDeletionError::IdempotencyConflict.into());
    }
    Err(ScriptError::NotFound {
        name: command.script_id.to_string(),
    })
}

pub(crate) fn trigger_to_parts(trigger: &ScriptTrigger) -> (String, serde_json::Value) {
    match trigger {
        ScriptTrigger::Event { entity_type, event } => (
            "event".to_string(),
            serde_json::json!({
                "entity_type": entity_type,
                "event": event.as_str(),
            }),
        ),
        ScriptTrigger::Cron { expression } => (
            "cron".to_string(),
            serde_json::json!({ "expression": expression }),
        ),
        ScriptTrigger::Manual => ("manual".to_string(), serde_json::json!({})),
        ScriptTrigger::Api { path, method } => (
            "api".to_string(),
            serde_json::json!({
                "path": path,
                "method": method.as_str(),
            }),
        ),
    }
}

pub(crate) fn trigger_from_parts(
    trigger_type: &str,
    trigger_config: &serde_json::Value,
) -> ScriptResult<ScriptTrigger> {
    match trigger_type {
        "event" => {
            let entity_type = trigger_config
                .get("entity_type")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let event_str = trigger_config
                .get("event")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let event = EventType::parse(event_str)
                .ok_or_else(|| ScriptError::InvalidTrigger(format!("event: {event_str}")))?;
            Ok(ScriptTrigger::Event { entity_type, event })
        }
        "cron" => {
            let expression = trigger_config
                .get("expression")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            Ok(ScriptTrigger::Cron { expression })
        }
        "manual" => Ok(ScriptTrigger::Manual),
        "api" => {
            let path = trigger_config
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let method_str = trigger_config
                .get("method")
                .and_then(|value| value.as_str())
                .unwrap_or("GET");
            let method = HttpMethod::parse(method_str)
                .ok_or_else(|| ScriptError::InvalidTrigger(format!("method: {method_str}")))?;
            Ok(ScriptTrigger::Api { path, method })
        }
        _ => Err(ScriptError::InvalidTrigger(trigger_type.to_string())),
    }
}

pub(crate) fn status_from_str(value: &str) -> ScriptResult<ScriptStatus> {
    match value {
        "draft" => Ok(ScriptStatus::Draft),
        "active" => Ok(ScriptStatus::Active),
        "paused" => Ok(ScriptStatus::Paused),
        "disabled" => Ok(ScriptStatus::Disabled),
        "archived" => Ok(ScriptStatus::Archived),
        _ => Err(ScriptError::InvalidStatus(value.to_string())),
    }
}

pub(crate) fn model_to_script(model: Model) -> ScriptResult<Script> {
    let trigger = trigger_from_parts(&model.trigger_type, &model.trigger_config)?;
    let status = status_from_str(&model.status)?;
    let permissions = model
        .permissions
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let workspace: RhaiWorkspace =
        serde_json::from_value(model.workspace).map_err(|error| {
            ScriptError::InvalidWorkspace(format!("stored workspace is invalid: {error}"))
        })?;
    workspace.validate().map_err(ScriptError::from)?;
    let source_provenance = source_provenance_from_json(model.source_provenance)?;

    Ok(Script {
        id: model.id,
        tenant_id: model.tenant_id,
        name: model.name,
        description: model.description,
        workspace,
        trigger,
        status,
        version: model.version.max(1) as u32,
        run_as_system: model.run_as_system,
        permissions,
        author_id: model.author_id,
        source_provenance,
        parent_release: release_ref_from_parts(
            model.parent_release_slug,
            model.parent_release_version,
            model.parent_release_digest,
        )?,
        created_at: model.created_at,
        updated_at: model.updated_at,
        error_count: model.error_count.max(0) as u32,
        last_error_at: model.last_error_at,
    })
}

pub(crate) fn permissions_to_json(permissions: &[String]) -> serde_json::Value {
    serde_json::Value::Array(
        permissions
            .iter()
            .map(|value| serde_json::Value::String(value.clone()))
            .collect(),
    )
}

pub(crate) fn workspace_to_json(workspace: &RhaiWorkspace) -> ScriptResult<serde_json::Value> {
    workspace.validate().map_err(ScriptError::from)?;
    serde_json::to_value(workspace)
        .map_err(|error| ScriptError::InvalidWorkspace(error.to_string()))
}

pub(crate) fn source_provenance_to_json(
    provenance: &crate::SourceProvenance,
) -> ScriptResult<serde_json::Value> {
    provenance
        .validate()
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
    serde_json::to_value(provenance).map_err(|error| ScriptError::Storage(error.to_string()))
}

pub(crate) fn source_provenance_from_json(
    value: serde_json::Value,
) -> ScriptResult<crate::SourceProvenance> {
    let provenance: crate::SourceProvenance =
        serde_json::from_value(value).map_err(|error| {
            ScriptError::Storage(format!("stored source provenance is invalid: {error}"))
        })?;
    provenance
        .validate()
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
    Ok(provenance)
}

pub(crate) fn new_script_active_model(script: &Script) -> ScriptResult<ActiveModel> {
    let (trigger_type, trigger_config) = trigger_to_parts(&script.trigger);
    Ok(ActiveModel {
        id: ActiveValue::Set(script.id),
        tenant_id: ActiveValue::Set(script.tenant_id),
        name: ActiveValue::Set(script.name.clone()),
        description: ActiveValue::Set(script.description.clone()),
        workspace: ActiveValue::Set(workspace_to_json(&script.workspace)?),
        trigger_type: ActiveValue::Set(trigger_type),
        trigger_config: ActiveValue::Set(trigger_config),
        status: ActiveValue::Set(script.status.as_str().to_string()),
        version: ActiveValue::Set(script.version as i32),
        run_as_system: ActiveValue::Set(script.run_as_system),
        permissions: ActiveValue::Set(permissions_to_json(&script.permissions)),
        author_id: ActiveValue::Set(script.author_id.clone()),
        source_provenance: ActiveValue::Set(source_provenance_to_json(
            &script.source_provenance,
        )?),
        parent_release_slug: ActiveValue::Set(
            script
                .parent_release
                .as_ref()
                .map(|release| release.slug.clone()),
        ),
        parent_release_version: ActiveValue::Set(
            script
                .parent_release
                .as_ref()
                .map(|release| release.version.clone()),
        ),
        parent_release_digest: ActiveValue::Set(
            script
                .parent_release
                .as_ref()
                .map(|release| release.digest.clone()),
        ),
        error_count: ActiveValue::Set(script.error_count as i32),
        last_error_at: ActiveValue::Set(script.last_error_at),
        created_at: ActiveValue::Set(script.created_at),
        updated_at: ActiveValue::Set(script.updated_at),
    })
}

pub(crate) fn source_digest(workspace: &RhaiWorkspace) -> ScriptResult<String> {
    workspace.digest().map_err(ScriptError::from)
}

pub(crate) fn model_to_source_revision(
    model: draft_revision::Model,
) -> ScriptResult<ScriptSourceRevision> {
    let revision = u32::try_from(model.revision).map_err(|_| {
        ScriptError::Storage("durable source revision is outside the supported range".into())
    })?;
    if revision == 0 {
        return Err(ScriptError::Storage(
            "durable source revision must be positive".into(),
        ));
    }
    let parent_revision = model
        .parent_revision
        .map(u32::try_from)
        .transpose()
        .map_err(|_| {
            ScriptError::Storage(
                "durable source parent revision is outside the supported range".into(),
            )
        })?;
    if parent_revision.is_some_and(|parent| parent == 0 || parent >= revision) {
        return Err(ScriptError::Storage(
            "durable source parent revision must precede its child revision".into(),
        ));
    }

    let workspace: RhaiWorkspace =
        serde_json::from_value(model.workspace).map_err(|error| {
            ScriptError::InvalidWorkspace(format!(
                "stored revision workspace is invalid: {error}"
            ))
        })?;
    workspace.validate().map_err(ScriptError::from)?;
    let source_provenance = source_provenance_from_json(model.source_provenance)?;

    Ok(ScriptSourceRevision {
        script_id: model.script_id,
        tenant_id: model.tenant_id,
        revision,
        parent_revision,
        source_digest: model.source_digest,
        workspace,
        author_id: model.author_id,
        source_provenance,
        parent_release: release_ref_from_parts(
            model.parent_release_slug,
            model.parent_release_version,
            model.parent_release_digest,
        )?,
        created_at: model.created_at,
    })
}

pub(crate) fn model_to_review_decision(model: draft_review::Model) -> ScriptResult<ReviewDecision> {
    let revision = u32::try_from(model.revision).map_err(|_| {
        ScriptError::Storage("durable review revision is outside the supported range".into())
    })?;
    let status = ReviewStatus::parse(&model.status).ok_or_else(|| {
        ScriptError::Storage(format!("stored review status is invalid: {}", model.status))
    })?;
    Ok(ReviewDecision {
        id: model.id,
        script_id: model.script_id,
        tenant_id: model.tenant_id,
        revision,
        source_digest: model.source_digest,
        status,
        policy_revision: model.policy_revision,
        actor_id: model.actor_id,
        reason: model.reason,
        idempotency_key: model.idempotency_key,
        request_digest: model.request_digest,
        created_at: model.created_at,
    })
}

pub(crate) fn model_to_component_candidate(
    model: component_candidate::Model,
) -> ScriptResult<RustComponentCandidate> {
    let parent_revision = u32::try_from(model.parent_revision).map_err(|_| {
        ScriptError::Storage(
            "durable component candidate revision is outside the supported range".into(),
        )
    })?;
    if parent_revision == 0 {
        return Err(ScriptError::Storage(
            "durable component candidate revision must be positive".into(),
        ));
    }
    let workspace: RustComponentWorkspace =
        serde_json::from_value(model.workspace).map_err(|error| {
            ScriptError::Storage(format!(
                "stored component candidate workspace is invalid: {error}"
            ))
        })?;
    workspace.validate().map_err(|error| {
        ScriptError::Storage(format!(
            "stored component candidate workspace is invalid: {error}"
        ))
    })?;
    let source_digest = workspace.source_digest().map_err(|error| {
        ScriptError::Storage(format!(
            "stored component candidate source digest is invalid: {error}"
        ))
    })?;
    let scenario_digest = workspace
        .scenario()
        .and_then(|scenario| {
            scenario.canonical_digest().map_err(|error| {
                crate::model::RustComponentWorkspaceError::Serialization(error.to_string())
            })
        })
        .map_err(|error| {
            ScriptError::Storage(format!(
                "stored component candidate scenario is invalid: {error}"
            ))
        })?;
    if source_digest != model.source_digest || scenario_digest != model.scenario_digest {
        return Err(ScriptError::Storage(
            "stored component candidate digest does not match its immutable workspace".into(),
        ));
    }
    let parent_release = release_ref_from_parts(
        Some(model.parent_release_slug),
        Some(model.parent_release_version),
        Some(model.parent_release_digest),
    )?
    .ok_or_else(|| {
        ScriptError::Storage("stored component candidate parent release is missing".into())
    })?;
    Ok(RustComponentCandidate {
        id: model.id,
        tenant_id: model.tenant_id,
        script_id: model.script_id,
        parent_revision,
        parent_source_digest: model.parent_source_digest,
        parent_release,
        workspace,
        source_digest: model.source_digest,
        scenario_digest: model.scenario_digest,
        actor_id: model.actor_id,
        idempotency_key: model.idempotency_key,
        request_digest: model.request_digest,
        created_at: model.created_at,
    })
}

pub(crate) fn model_to_component_candidate_review(
    model: component_candidate_review::Model,
) -> ScriptResult<RustComponentCandidateReview> {
    let status = ReviewStatus::parse(&model.status).ok_or_else(|| {
        ScriptError::Storage(format!(
            "stored component candidate review status is invalid: {}",
            model.status
        ))
    })?;
    Ok(RustComponentCandidateReview {
        id: model.id,
        candidate_id: model.candidate_id,
        tenant_id: model.tenant_id,
        source_digest: model.source_digest,
        scenario_digest: model.scenario_digest,
        status,
        policy_revision: model.policy_revision,
        actor_id: model.actor_id,
        reason: model.reason,
        idempotency_key: model.idempotency_key,
        request_digest: model.request_digest,
        created_at: model.created_at,
    })
}

pub(crate) fn model_to_component_candidate_build(
    model: component_candidate_build::Model,
) -> ScriptResult<RustComponentCandidateBuild> {
    if model.id.is_nil()
        || model.candidate_id.is_nil()
        || model.tenant_id.is_nil()
        || model.build_request_id.is_nil()
        || model.actor_id.is_nil()
        || model.idempotency_key.is_nil()
        || !valid_digest(&model.candidate_source_digest)
        || !valid_digest(&model.scenario_digest)
        || !valid_digest(&model.archive_source_digest)
        || !valid_digest(&model.request_digest)
        || model.source_reference != format!("cas://{}", model.archive_source_digest)
    {
        return Err(ScriptError::Storage(
            "stored component candidate build contains an invalid identity".into(),
        ));
    }
    Ok(RustComponentCandidateBuild {
        id: model.id,
        candidate_id: model.candidate_id,
        tenant_id: model.tenant_id,
        candidate_source_digest: model.candidate_source_digest,
        scenario_digest: model.scenario_digest,
        archive_source_digest: model.archive_source_digest,
        build_request_id: model.build_request_id,
        source_reference: model.source_reference,
        actor_id: model.actor_id,
        idempotency_key: model.idempotency_key,
        request_digest: model.request_digest,
        created_at: model.created_at,
    })
}

pub(crate) fn model_to_component_candidate_build_execution(
    model: component_candidate_build_execution::Model,
) -> ScriptResult<RustComponentCandidateBuildExecution> {
    let build_result_revision = u64::try_from(model.build_result_revision).map_err(|_| {
        ScriptError::Storage(
            "durable component candidate build execution revision is outside the supported range"
                .into(),
        )
    })?;
    let publication: ModuleBuildPublicationReceipt = serde_json::from_value(model.publication)
        .map_err(|error| {
            ScriptError::Storage(format!(
                "stored component candidate build publication is invalid: {error}"
            ))
        })?;
    let scenario_comparison: LocalSandboxScenarioComparison =
        serde_json::from_value(model.scenario_comparison).map_err(|error| {
            ScriptError::Storage(format!(
                "stored component candidate scenario comparison is invalid: {error}"
            ))
        })?;
    if model.candidate_build_id.is_nil()
        || model.candidate_id.is_nil()
        || model.tenant_id.is_nil()
        || model.build_request_id.is_nil()
        || build_result_revision == 0
        || !valid_digest(&model.candidate_source_digest)
        || !valid_digest(&model.scenario_digest)
        || !valid_digest(&model.archive_source_digest)
        || !valid_digest(&model.component_digest)
        || !valid_digest(&model.sbom_digest)
        || !valid_digest(&model.provenance_digest)
        || model.source_reference != format!("cas://{}", model.archive_source_digest)
        || scenario_comparison.scenario_digest != model.scenario_digest
        || publication.artifact.validate().is_err()
        || publication.signature_manifest.validate().is_err()
        || publication.artifact.registry != publication.signature_manifest.registry
        || publication.artifact.repository != publication.signature_manifest.repository
    {
        return Err(ScriptError::Storage(
            "stored component candidate build execution contains invalid evidence".into(),
        ));
    }
    Ok(RustComponentCandidateBuildExecution {
        candidate_id: model.candidate_id,
        candidate_build_id: model.candidate_build_id,
        tenant_id: model.tenant_id,
        candidate_source_digest: model.candidate_source_digest,
        scenario_digest: model.scenario_digest,
        archive_source_digest: model.archive_source_digest,
        build_request_id: model.build_request_id,
        source_reference: model.source_reference,
        build_result_revision,
        component_digest: model.component_digest,
        sbom_digest: model.sbom_digest,
        provenance_digest: model.provenance_digest,
        publication,
        scenario_comparison,
        created_at: model.created_at,
    })
}

pub(crate) fn model_to_test_run(model: draft_test_run::Model) -> ScriptResult<TestRun> {
    let revision = u32::try_from(model.revision).map_err(|_| {
        ScriptError::Storage("durable test revision is outside the supported range".into())
    })?;
    if revision == 0 {
        return Err(ScriptError::Storage(
            "durable test revision must be positive".into(),
        ));
    }
    let status = TestRunStatus::parse(&model.status).ok_or_else(|| {
        ScriptError::Storage(format!("stored test status is invalid: {}", model.status))
    })?;
    match status {
        TestRunStatus::Pending
            if model.passed.is_some()
                || model.error.is_some()
                || model.completed_at.is_some()
                || model.lease_token.is_none()
                || model.lease_expires_at.is_none() =>
        {
            return Err(ScriptError::Storage(
                "pending test run has invalid terminal or lease fields".into(),
            ));
        }
        TestRunStatus::Passed
            if model.passed != Some(true)
                || model.error.is_some()
                || model.completed_at.is_none()
                || model.lease_token.is_some()
                || model.lease_expires_at.is_some() =>
        {
            return Err(ScriptError::Storage(
                "passed test run has invalid result or lease fields".into(),
            ));
        }
        TestRunStatus::Failed
            if model.passed != Some(false)
                || model.completed_at.is_none()
                || model.lease_token.is_some()
                || model.lease_expires_at.is_some() =>
        {
            return Err(ScriptError::Storage(
                "failed test run has invalid result or lease fields".into(),
            ));
        }
        _ => {}
    }
    Ok(TestRun {
        id: model.id,
        script_id: model.script_id,
        tenant_id: model.tenant_id,
        revision,
        source_digest: model.source_digest,
        test_path: model.test_path,
        actor_id: model.actor_id,
        idempotency_key: model.idempotency_key,
        request_digest: model.request_digest,
        status,
        passed: model.passed,
        error: model.error,
        created_at: model.created_at,
        completed_at: model.completed_at,
    })
}

pub(crate) fn release_ref_from_parts(
    slug: Option<String>,
    version: Option<String>,
    digest: Option<String>,
) -> ScriptResult<Option<ArtifactReleaseRef>> {
    match (slug, version, digest) {
        (None, None, None) => Ok(None),
        (Some(slug), Some(version), Some(digest)) => {
            let release = ArtifactReleaseRef {
                slug,
                version,
                digest,
            };
            release
                .validate()
                .map_err(|error| ScriptError::InvalidLineage(error.to_string()))?;
            Ok(Some(release))
        }
        _ => Err(ScriptError::InvalidLineage(
            "durable parent release identity is incomplete".to_string(),
        )),
    }
}

pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

pub(crate) fn validate_parent_release(release: &Option<ArtifactReleaseRef>) -> ScriptResult<()> {
    if let Some(release) = release {
        release
            .validate()
            .map_err(|error| ScriptError::InvalidLineage(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn apply_query(
    select: sea_orm::Select<Entity>,
    query: ScriptQuery,
    tenant_id: Option<Uuid>,
) -> sea_orm::Select<Entity> {
    let select = match query {
        ScriptQuery::ById(id) => select.filter(Column::Id.eq(id)),
        ScriptQuery::ByName(name) => select.filter(Column::Name.eq(name)),
        ScriptQuery::ByEvent { entity_type, event } => select
            .filter(Column::TriggerType.eq("event"))
            .filter(Column::Status.eq(ScriptStatus::Active.as_str()))
            .filter(Expr::cust_with_values(
                "trigger_config->>'entity_type' = $1",
                [entity_type],
            ))
            .filter(Expr::cust_with_values(
                "trigger_config->>'event' = $1",
                [event.as_str()],
            )),
        ScriptQuery::ByApiPath(path) => select
            .filter(Column::TriggerType.eq("api"))
            .filter(Column::Status.eq(ScriptStatus::Active.as_str()))
            .filter(Expr::cust_with_values(
                "trigger_config->>'path' = $1",
                [path],
            )),
        ScriptQuery::Scheduled => select
            .filter(Column::TriggerType.eq("cron"))
            .filter(Column::Status.eq(ScriptStatus::Active.as_str())),
        ScriptQuery::ByStatus(status) => select.filter(Column::Status.eq(status.as_str())),
        ScriptQuery::All => select,
    };

    if let Some(tid) = tenant_id {
        select.filter(Column::TenantId.eq(tid))
    } else {
        select
    }
}
