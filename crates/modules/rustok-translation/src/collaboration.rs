use chrono::{DateTime, FixedOffset, Utc};
use rustok_api::{
    Action, PortActor, PortCallPolicy, PortContext, Resource, manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_events::TranslationWorkflowEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{job, job_item, workflow_note},
    progress::authorize_translation,
    workflow::{
        actor_kind_value, event_actor_id, next_revision, validate_workflow_actor, workflow_actor,
    },
};

pub const MAX_WORKFLOW_NOTE_BODY_CHARACTERS: usize = 4_000;
pub const MAX_WORKFLOW_NOTE_LIST_LIMIT: u16 = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateWorkflowNoteInput {
    pub job_id: Uuid,
    pub item_id: Option<Uuid>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListWorkflowNotesInput {
    pub job_id: Uuid,
    pub item_id: Option<Uuid>,
    pub include_resolved: bool,
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveWorkflowNoteInput {
    pub note_id: Uuid,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowNoteRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub item_id: Option<Uuid>,
    pub body: String,
    pub author: PortActor,
    pub revision: i64,
    pub resolved_at: Option<DateTime<FixedOffset>>,
    pub resolved_by: Option<PortActor>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

/// Owner service for private, append-only Translation workflow collaboration.
///
/// These notes are not generic comments: they inherit Translation job/item RBAC,
/// are never publicly projected, and are intentionally excluded from Translation
/// Memory, machine requests, and owner data application.
pub struct TranslationCollaborationService {
    database: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl TranslationCollaborationService {
    pub fn new(database: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            database,
            event_bus,
        }
    }

    pub async fn list_workflow_notes(
        &self,
        context: PortContext,
        input: ListWorkflowNotesInput,
    ) -> TranslationResult<Vec<WorkflowNoteRecord>> {
        let tenant_id = authorize_translation(&context, PortCallPolicy::read(), Action::Read)?;
        validate_list_input(&input)?;
        ensure_job(&self.database, tenant_id, input.job_id).await?;

        let mut query = workflow_note::Entity::find()
            .filter(workflow_note::Column::TenantId.eq(tenant_id))
            .filter(workflow_note::Column::JobId.eq(input.job_id));
        if let Some(item_id) = input.item_id {
            query = query.filter(workflow_note::Column::ItemId.eq(item_id));
        }
        if !input.include_resolved {
            query = query.filter(workflow_note::Column::ResolvedAt.is_null());
        }
        query
            .order_by_desc(workflow_note::Column::CreatedAt)
            .order_by_desc(workflow_note::Column::Id)
            .limit(u64::from(input.limit))
            .all(&self.database)
            .await?
            .into_iter()
            .map(workflow_note_record)
            .collect()
    }

    pub async fn create_workflow_note(
        &self,
        context: PortContext,
        input: CreateWorkflowNoteInput,
    ) -> TranslationResult<WorkflowNoteRecord> {
        let (tenant_id, security) = authorize_note_writer(&context)?;
        validate_workflow_actor(&context.actor)?;
        validate_note_body(&input.body)?;
        let idempotency_key = idempotency_key(&context)?;
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_note_by_create_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_note_creation(existing, &context, &request_hash);
        }

        let transaction = self.database.begin().await?;
        ensure_job(&transaction, tenant_id, input.job_id).await?;
        if let Some(item_id) = input.item_id {
            let item = ensure_item(&transaction, tenant_id, input.job_id, item_id).await?;
            ensure_item_note_writer(&security, &context.actor, &item)?;
        }

        let now = Utc::now().fixed_offset();
        let note_id = generate_id();
        workflow_note::Entity::insert(workflow_note::ActiveModel {
            id: Set(note_id),
            tenant_id: Set(tenant_id),
            job_id: Set(input.job_id),
            item_id: Set(input.item_id),
            body: Set(input.body),
            created_by_actor_kind: Set(actor_kind_value(&context.actor.kind).to_string()),
            created_by_actor_id: Set(context.actor.id.clone()),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            revision: Set(0),
            resolved_at: Set(None),
            resolved_by_actor_kind: Set(None),
            resolved_by_actor_id: Set(None),
            resolution_idempotency_key: Set(None),
            resolution_request_hash: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                workflow_note::Column::TenantId,
                workflow_note::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted = find_note_by_create_idempotency(&transaction, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        if persisted.id != note_id {
            transaction.rollback().await?;
            return replay_note_creation(persisted, &context, &request_hash);
        }

        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::NoteCreated {
                    note_id,
                    job_id: input.job_id,
                    item_id: input.item_id,
                    revision: 0,
                },
            )
            .await?;
        transaction.commit().await?;
        replay_note_creation(persisted, &context, &request_hash)
    }

    pub async fn resolve_workflow_note(
        &self,
        context: PortContext,
        input: ResolveWorkflowNoteInput,
    ) -> TranslationResult<WorkflowNoteRecord> {
        let tenant_id = authorize_translation(&context, PortCallPolicy::write(), Action::Resolve)?;
        validate_workflow_actor(&context.actor)?;
        if input.expected_revision < 0 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let idempotency_key = idempotency_key(&context)?;
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_note_by_resolution_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_note_resolution(existing, &context, &request_hash);
        }

        let transaction = self.database.begin().await?;
        let note = workflow_note::Entity::find_by_id(input.note_id)
            .filter(workflow_note::Column::TenantId.eq(tenant_id))
            .one(&transaction)
            .await?
            .ok_or(TranslationError::WorkflowNoteNotFound)?;
        if note.resolved_at.is_some() || note.revision != input.expected_revision {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let revision = next_revision(note.revision)?;
        let now = Utc::now().fixed_offset();
        let update = workflow_note::Entity::update_many()
            .col_expr(workflow_note::Column::Revision, Expr::value(revision))
            .col_expr(workflow_note::Column::ResolvedAt, Expr::value(now))
            .col_expr(
                workflow_note::Column::ResolvedByActorKind,
                Expr::value(actor_kind_value(&context.actor.kind)),
            )
            .col_expr(
                workflow_note::Column::ResolvedByActorId,
                Expr::value(context.actor.id.clone()),
            )
            .col_expr(
                workflow_note::Column::ResolutionIdempotencyKey,
                Expr::value(idempotency_key.clone()),
            )
            .col_expr(
                workflow_note::Column::ResolutionRequestHash,
                Expr::value(request_hash.clone()),
            )
            .col_expr(workflow_note::Column::UpdatedAt, Expr::value(now))
            .filter(workflow_note::Column::TenantId.eq(tenant_id))
            .filter(workflow_note::Column::Id.eq(input.note_id))
            .filter(workflow_note::Column::Revision.eq(input.expected_revision))
            .filter(workflow_note::Column::ResolvedAt.is_null())
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let persisted = workflow_note::Entity::find_by_id(input.note_id)
            .filter(workflow_note::Column::TenantId.eq(tenant_id))
            .one(&transaction)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::NoteResolved {
                    note_id: note.id,
                    job_id: note.job_id,
                    item_id: note.item_id,
                    revision,
                },
            )
            .await?;
        transaction.commit().await?;
        replay_note_resolution(persisted, &context, &request_hash)
    }
}

fn validate_list_input(input: &ListWorkflowNotesInput) -> TranslationResult<()> {
    if input.limit == 0 || input.limit > MAX_WORKFLOW_NOTE_LIST_LIMIT {
        return Err(TranslationError::InvalidRequest(format!(
            "workflow note limit must be between 1 and {MAX_WORKFLOW_NOTE_LIST_LIMIT}",
        )));
    }
    Ok(())
}

fn validate_note_body(body: &str) -> TranslationResult<()> {
    if body.trim().is_empty()
        || body.trim() != body
        || body.chars().count() > MAX_WORKFLOW_NOTE_BODY_CHARACTERS
    {
        return Err(TranslationError::InvalidRequest(format!(
            "workflow note body must be trimmed and contain between 1 and {MAX_WORKFLOW_NOTE_BODY_CHARACTERS} characters",
        )));
    }
    Ok(())
}

fn idempotency_key(context: &PortContext) -> TranslationResult<String> {
    let key = context.idempotency_key.as_deref().unwrap_or_default();
    if key.is_empty() || key.len() > 191 || key.trim() != key {
        return Err(TranslationError::InvalidRequest(
            "workflow note idempotency key is invalid".to_string(),
        ));
    }
    Ok(key.to_string())
}

fn authorize_note_writer(context: &PortContext) -> TranslationResult<(Uuid, SecurityContext)> {
    context.require_policy(PortCallPolicy::write())?;
    let security = SecurityContext::try_from_port_context(context)?;
    if [Action::Update, Action::Resolve, Action::Manage]
        .into_iter()
        .all(|action| security.get_scope(Resource::Translations, action) == PermissionScope::None)
    {
        return Err(TranslationError::Forbidden);
    }
    let tenant_id =
        Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
    Ok((tenant_id, security))
}

fn ensure_item_note_writer(
    security: &SecurityContext,
    actor: &PortActor,
    item: &job_item::Model,
) -> TranslationResult<()> {
    if security.get_scope(Resource::Translations, Action::Manage) != PermissionScope::None {
        return Ok(());
    }
    if security.get_scope(Resource::Translations, Action::Resolve) != PermissionScope::None
        && item.status == "in_review"
    {
        return Ok(());
    }
    if security.get_scope(Resource::Translations, Action::Update) != PermissionScope::None
        && crate::workflow::assignment_actor(item)?.as_ref() == Some(actor)
    {
        return Ok(());
    }
    Err(TranslationError::Forbidden)
}

async fn ensure_job<C>(database: &C, tenant_id: Uuid, job_id: Uuid) -> TranslationResult<job::Model>
where
    C: ConnectionTrait,
{
    job::Entity::find_by_id(job_id)
        .filter(job::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::JobNotFound)
}

async fn ensure_item<C>(
    database: &C,
    tenant_id: Uuid,
    job_id: Uuid,
    item_id: Uuid,
) -> TranslationResult<job_item::Model>
where
    C: ConnectionTrait,
{
    job_item::Entity::find_by_id(item_id)
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .filter(job_item::Column::JobId.eq(job_id))
        .one(database)
        .await?
        .ok_or(TranslationError::ItemNotFound)
}

async fn find_note_by_create_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<workflow_note::Model>>
where
    C: ConnectionTrait,
{
    Ok(workflow_note::Entity::find()
        .filter(workflow_note::Column::TenantId.eq(tenant_id))
        .filter(workflow_note::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_note_by_resolution_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<workflow_note::Model>>
where
    C: ConnectionTrait,
{
    Ok(workflow_note::Entity::find()
        .filter(workflow_note::Column::TenantId.eq(tenant_id))
        .filter(workflow_note::Column::ResolutionIdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

fn replay_note_creation(
    model: workflow_note::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<WorkflowNoteRecord> {
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.created_by_actor_kind != actor_kind_value(&context.actor.kind)
        || model.created_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    workflow_note_record(model)
}

fn replay_note_resolution(
    model: workflow_note::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<WorkflowNoteRecord> {
    if model.resolution_request_hash.as_deref() != Some(request_hash) {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.resolved_by_actor_kind.as_deref() != Some(actor_kind_value(&context.actor.kind))
        || model.resolved_by_actor_id.as_deref() != Some(context.actor.id.as_str())
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    workflow_note_record(model)
}

fn workflow_note_record(model: workflow_note::Model) -> TranslationResult<WorkflowNoteRecord> {
    let author = workflow_actor(&model.created_by_actor_kind, &model.created_by_actor_id)?;
    let resolved_by = match (&model.resolved_by_actor_kind, &model.resolved_by_actor_id) {
        (None, None) => None,
        (Some(kind), Some(id)) => Some(workflow_actor(kind, id)?),
        _ => return Err(TranslationError::WorkflowRevisionConflict),
    };
    if (model.resolved_at.is_some()) != resolved_by.is_some() {
        return Err(TranslationError::WorkflowRevisionConflict);
    }
    Ok(WorkflowNoteRecord {
        id: model.id,
        job_id: model.job_id,
        item_id: model.item_id,
        body: model.body,
        author,
        revision: model.revision,
        resolved_at: model.resolved_at,
        resolved_by,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}
