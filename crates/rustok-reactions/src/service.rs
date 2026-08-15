use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_events::ReactionsEvent;
use rustok_outbox::{ContractEventWriteOnceError, TransactionalEventBus, idempotency};
use rustok_reactions_api::{
    ApplyReactionCommand, ReactionAction, ReactionActorState, ReactionAggregate, ReactionCatalog,
    ReactionContractError, ReactionKey, ReactionProviderError, ReactionReadPort,
    ReactionReadRequest, ReactionSelectionPolicy, ReactionSnapshot, ReactionSubjectAccess,
    ReactionSubjectAuthorization, ReactionSubjectRef, ReactionSubjectRegistry,
    ReactionSubjectRegistryEntry, ReactionSubjectRequest, ReactionWritePort, ReactionWriteReceipt,
};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::entities::{actor_state, aggregate, catalog, subject};

const REACTION_OWNER_SLUG: &str = "reactions";
const APPLY_REACTION_OPERATION: &str = "apply_reaction";
const ACT_AS_ACTOR_CLAIM: &str = "reactions:act_as_actor";

#[derive(Clone)]
pub struct ReactionsService {
    database: DatabaseConnection,
    subjects: Arc<ReactionSubjectRegistry>,
}

impl ReactionsService {
    pub fn new(database: DatabaseConnection, subjects: Arc<ReactionSubjectRegistry>) -> Self {
        Self { database, subjects }
    }

    pub fn from_runtime_extensions(
        database: DatabaseConnection,
        extensions: &rustok_core::ModuleRuntimeExtensions,
    ) -> Self {
        let subjects = rustok_reactions_api::reaction_subject_registry_from_extensions(extensions)
            .unwrap_or_else(|| Arc::new(ReactionSubjectRegistry::default()));
        Self::new(database, subjects)
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.database
    }

    pub fn subject_sources(&self) -> Vec<ReactionSubjectRegistryEntry> {
        self.subjects.entries()
    }

    pub fn subject_source_count(&self) -> usize {
        self.subjects.len()
    }

    pub fn has_subject_sources(&self) -> bool {
        !self.subjects.is_empty()
    }

    async fn authorize_subject(
        &self,
        context: &PortContext,
        subject: ReactionSubjectRef,
        access: ReactionSubjectAccess,
    ) -> Result<(ReactionSubjectRef, u64, ReactionCatalog), PortError> {
        let provider = self.subjects.get(subject.source()).ok_or_else(|| {
            PortError::unavailable(
                "reactions.subject_provider_unavailable",
                "reaction subject provider is unavailable",
            )
        })?;
        if !provider.supported_kinds().contains(subject.kind()) {
            return Err(PortError::validation(
                "reactions.subject_kind_unsupported",
                "reaction subject kind is not supported by its source provider",
            ));
        }

        let request = ReactionSubjectRequest { subject, access };
        request.validate().map_err(contract_error_to_port_error)?;
        let authorization = provider
            .authorize(context.clone(), request.clone())
            .await
            .map_err(provider_error_to_port_error)?;
        authorization
            .validate_for(&request)
            .map_err(contract_error_to_port_error)?;

        match authorization {
            ReactionSubjectAuthorization::Allowed {
                canonical_subject,
                catalog,
            } => {
                let catalog_revision = canonical_subject.subject_revision();
                Ok((canonical_subject, catalog_revision, catalog))
            }
            ReactionSubjectAuthorization::Unavailable => Err(PortError::not_found(
                "reactions.subject_unavailable",
                "reaction subject is unavailable",
            )),
        }
    }

    async fn read_snapshot(
        &self,
        subject_ref: ReactionSubjectRef,
        catalog: ReactionCatalog,
        actor_id: Option<Uuid>,
    ) -> Result<ReactionSnapshot, PortError> {
        let tenant_id = subject_ref.tenant_id();
        let stored_subject = find_subject(self.database(), &subject_ref).await?;
        let Some(stored_subject) = stored_subject else {
            return ReactionSnapshot::try_new(subject_ref, catalog, None, Vec::new())
                .map_err(contract_error_to_port_error);
        };

        let aggregates = aggregate::Entity::find()
            .filter(aggregate::Column::TenantId.eq(tenant_id))
            .filter(aggregate::Column::ReactionSubjectId.eq(stored_subject.id))
            .filter(aggregate::Column::Count.gt(0_i64))
            .order_by_asc(aggregate::Column::ReactionKey)
            .all(self.database())
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|row| {
                let reaction =
                    ReactionKey::new(row.reaction_key).map_err(contract_error_to_port_error)?;
                if !catalog.contains(&reaction) {
                    return Err(PortError::conflict(
                        "reactions.catalog_reconciliation_required",
                        "stored reaction aggregate is outside the authorized catalog",
                    ));
                }
                let count = u64::try_from(row.count).map_err(|_| {
                    PortError::invariant_violation(
                        "reactions.aggregate_count_invalid",
                        "stored reaction aggregate count is negative",
                    )
                })?;
                Ok(ReactionAggregate { reaction, count })
            })
            .collect::<Result<Vec<_>, PortError>>()?;

        let actor_state = match actor_id {
            None => None,
            Some(actor_id) => actor_state::Entity::find()
                .filter(actor_state::Column::TenantId.eq(tenant_id))
                .filter(actor_state::Column::ReactionSubjectId.eq(stored_subject.id))
                .filter(actor_state::Column::ActorId.eq(actor_id))
                .one(self.database())
                .await
                .map_err(database_error)?
                .map(decode_actor_state)
                .transpose()?,
        };

        ReactionSnapshot::try_new(subject_ref, catalog, actor_state, aggregates)
            .map_err(contract_error_to_port_error)
    }

    async fn execute_apply(
        &self,
        lease: idempotency::Lease,
        command: &ApplyReactionCommand,
        canonical_subject: ReactionSubjectRef,
        catalog_revision: u64,
        catalog_value: ReactionCatalog,
    ) -> Result<ReactionWriteReceipt, PortError> {
        let transaction = self.database.begin().await.map_err(database_error)?;
        let result = apply_inside_transaction(
            &transaction,
            lease.operation_id,
            command,
            canonical_subject,
            catalog_revision,
            catalog_value,
        )
        .await;

        match result {
            Ok(receipt) => {
                idempotency::complete(&transaction, lease, &receipt).await?;
                transaction.commit().await.map_err(database_error)?;
                Ok(receipt)
            }
            Err(error) => Err(error),
        }
    }
}

#[async_trait]
impl ReactionReadPort for ReactionsService {
    async fn read_reactions(
        &self,
        context: PortContext,
        request: ReactionReadRequest,
    ) -> Result<ReactionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_subject_tenant(&context, request.subject())?;
        if let Some(actor_id) = request.actor_id() {
            authorize_actor(&context, actor_id)?;
        }

        let (canonical_subject, _catalog_revision, catalog) = self
            .authorize_subject(
                &context,
                request.subject().clone(),
                ReactionSubjectAccess::Read {
                    actor_id: request.actor_id(),
                },
            )
            .await?;
        self.read_snapshot(canonical_subject, catalog, request.actor_id())
            .await
    }
}

#[async_trait]
impl ReactionWritePort for ReactionsService {
    async fn apply_reaction(
        &self,
        context: PortContext,
        command: ApplyReactionCommand,
    ) -> Result<ReactionWriteReceipt, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        validate_subject_tenant(&context, command.subject())?;
        authorize_actor(&context, command.identity().actor_id())?;

        let expected_key = command.identity().command_id().to_string();
        if context.idempotency_key.as_deref() != Some(expected_key.as_str()) {
            return Err(PortError::validation(
                "reactions.command_idempotency_mismatch",
                "reaction command UUID must equal the port idempotency key",
            ));
        }

        let (canonical_subject, catalog_revision, catalog_value) = self
            .authorize_subject(
                &context,
                command.subject().clone(),
                ReactionSubjectAccess::Apply {
                    command: command.clone(),
                },
            )
            .await?;
        if !catalog_value.contains(command.reaction()) {
            return Err(PortError::validation(
                "reactions.reaction_not_allowed",
                "reaction key is not present in the authorized catalog",
            ));
        }

        let lease = match idempotency::admit(
            self.database(),
            canonical_subject.tenant_id(),
            REACTION_OWNER_SLUG,
            expected_key.as_str(),
            APPLY_REACTION_OPERATION,
            &command,
        )
        .await?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_replay(value),
            idempotency::Admission::ReplayError(error) => return Err(error),
        };

        let result = self
            .execute_apply(
                lease,
                &command,
                canonical_subject,
                catalog_revision,
                catalog_value,
            )
            .await;
        if let Err(error) = &result
            && let Err(receipt_error) = idempotency::fail(self.database(), lease, error).await
        {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "failed to persist Reactions command failure receipt"
            );
        }
        result
    }
}

async fn apply_inside_transaction(
    transaction: &DatabaseTransaction,
    event_envelope_id: Uuid,
    command: &ApplyReactionCommand,
    canonical_subject: ReactionSubjectRef,
    catalog_revision: u64,
    catalog_value: ReactionCatalog,
) -> Result<ReactionWriteReceipt, PortError> {
    let stored_subject = synchronize_subject(
        transaction,
        &canonical_subject,
        catalog_revision,
        &catalog_value,
    )
    .await?;
    let actor_id = command.identity().actor_id();
    let existing_state = actor_state::Entity::find()
        .filter(actor_state::Column::TenantId.eq(canonical_subject.tenant_id()))
        .filter(actor_state::Column::ReactionSubjectId.eq(stored_subject.id))
        .filter(actor_state::Column::ActorId.eq(actor_id))
        .one(transaction)
        .await
        .map_err(database_error)?;

    let state_exists = existing_state.is_some();
    let (state_id, current_revision, mut selected, created_at) = match existing_state {
        Some(row) => {
            let selected = decode_selected(row.selected_json)?;
            (
                row.id,
                i64_to_u64(row.revision, "actor state revision")?,
                selected,
                row.created_at,
            )
        }
        None => (Uuid::new_v4(), 0, Vec::new(), Utc::now().fixed_offset()),
    };

    if selected.iter().any(|key| !catalog_value.contains(key))
        || selected.len() > catalog_value.selection().maximum_selected()
    {
        return Err(PortError::conflict(
            "reactions.catalog_reconciliation_required",
            "stored actor state is outside the authorized catalog",
        ));
    }

    let (changed, next_selected, mut deltas) = plan_selection(
        catalog_value.selection(),
        &selected,
        command.reaction(),
        command.action(),
    )?;
    selected = next_selected;

    deltas.retain(|_, delta| *delta != 0);
    let event_deltas = deltas.clone();
    let next_revision = if changed {
        current_revision.checked_add(1).ok_or_else(|| {
            PortError::invariant_violation(
                "reactions.actor_state_revision_exhausted",
                "reaction actor state revision is exhausted",
            )
        })?
    } else {
        current_revision
    };

    if changed {
        persist_actor_state(
            transaction,
            canonical_subject.tenant_id(),
            stored_subject.id,
            actor_id,
            state_id,
            created_at,
            next_revision,
            &selected,
            state_exists,
        )
        .await?;
        for (reaction, delta) in deltas {
            apply_aggregate_delta(
                transaction,
                canonical_subject.tenant_id(),
                stored_subject.id,
                reaction,
                delta,
            )
            .await?;
        }
        publish_actor_state_changed(
            transaction,
            event_envelope_id,
            command,
            &canonical_subject,
            next_revision,
            &selected,
            &event_deltas,
        )
        .await?;
    }

    ReactionWriteReceipt::new(
        command.identity().command_id(),
        actor_id,
        canonical_subject,
        command.reaction().clone(),
        command.action(),
        changed,
        next_revision,
    )
    .map_err(contract_error_to_port_error)
}

async fn publish_actor_state_changed(
    transaction: &DatabaseTransaction,
    envelope_id: Uuid,
    command: &ApplyReactionCommand,
    subject: &ReactionSubjectRef,
    state_revision: u64,
    selected: &[ReactionKey],
    deltas: &BTreeMap<ReactionKey, i64>,
) -> Result<(), PortError> {
    let action = match command.action() {
        ReactionAction::Add => "add",
        ReactionAction::Remove => "remove",
    };
    let added_keys = deltas
        .iter()
        .filter(|(_, delta)| **delta > 0)
        .map(|(reaction, _)| reaction.to_string())
        .collect();
    let removed_keys = deltas
        .iter()
        .filter(|(_, delta)| **delta < 0)
        .map(|(reaction, _)| reaction.to_string())
        .collect();
    let event = ReactionsEvent::ActorStateChanged {
        command_id: command.identity().command_id(),
        source_slug: subject.source().to_string(),
        subject_kind: subject.kind().to_string(),
        subject_id: subject.subject_id(),
        subject_revision: u64_to_i64(
            subject.subject_revision(),
            "reaction event subject revision",
        )?,
        actor_id: command.identity().actor_id(),
        requested_reaction: command.reaction().to_string(),
        action: action.to_string(),
        state_revision: u64_to_i64(state_revision, "reaction event state revision")?,
        selected_keys: selected.iter().map(ToString::to_string).collect(),
        added_keys,
        removed_keys,
    };

    match TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        transaction,
        envelope_id,
        subject.tenant_id(),
        Some(command.identity().actor_id()),
        event,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(ContractEventWriteOnceError::Conflict) => Err(PortError::conflict(
            "reactions.event_identity_conflict",
            "reaction semantic event identity is already bound to different facts",
        )),
        Err(ContractEventWriteOnceError::Unavailable) => Err(PortError::unavailable(
            "reactions.event_unavailable",
            "reaction semantic event could not be persisted",
        )),
    }
}

fn plan_selection(
    policy: ReactionSelectionPolicy,
    current: &[ReactionKey],
    reaction: &ReactionKey,
    action: ReactionAction,
) -> Result<(bool, Vec<ReactionKey>, BTreeMap<ReactionKey, i64>), PortError> {
    let mut selected = current.to_vec();
    let mut deltas = BTreeMap::<ReactionKey, i64>::new();
    let changed = match action {
        ReactionAction::Add => match policy {
            ReactionSelectionPolicy::Single => {
                if selected.len() == 1 && selected[0] == *reaction {
                    false
                } else {
                    for previous in selected.drain(..) {
                        *deltas.entry(previous).or_default() -= 1;
                    }
                    selected.push(reaction.clone());
                    *deltas.entry(reaction.clone()).or_default() += 1;
                    true
                }
            }
            ReactionSelectionPolicy::Multiple { max_selected } => {
                if selected.contains(reaction) {
                    false
                } else {
                    if selected.len() >= usize::from(max_selected) {
                        return Err(PortError::conflict(
                            "reactions.selection_limit_reached",
                            "actor reaction selection limit has been reached",
                        ));
                    }
                    selected.push(reaction.clone());
                    selected.sort();
                    *deltas.entry(reaction.clone()).or_default() += 1;
                    true
                }
            }
        },
        ReactionAction::Remove => {
            if let Some(index) = selected.iter().position(|selected| selected == reaction) {
                let removed = selected.remove(index);
                *deltas.entry(removed).or_default() -= 1;
                true
            } else {
                false
            }
        }
    };
    deltas.retain(|_, delta| *delta != 0);
    Ok((changed, selected, deltas))
}

async fn synchronize_subject(
    transaction: &DatabaseTransaction,
    canonical_subject: &ReactionSubjectRef,
    catalog_revision: u64,
    catalog_value: &ReactionCatalog,
) -> Result<subject::Model, PortError> {
    let tenant_id = canonical_subject.tenant_id();
    let subject_revision = u64_to_i64(
        canonical_subject.subject_revision(),
        "reaction subject revision",
    )?;
    let catalog_revision = u64_to_i64(catalog_revision, "reaction catalog revision")?;
    let now = Utc::now().fixed_offset();
    let candidate_id = Uuid::new_v4();

    subject::Entity::insert(subject::ActiveModel {
        id: Set(candidate_id),
        tenant_id: Set(tenant_id),
        source_slug: Set(canonical_subject.source().to_string()),
        subject_kind: Set(canonical_subject.kind().to_string()),
        subject_id: Set(canonical_subject.subject_id()),
        subject_revision: Set(subject_revision),
        current_catalog_revision: Set(catalog_revision),
        created_at: Set(now),
        updated_at: Set(now),
    })
    .on_conflict(
        OnConflict::columns([
            subject::Column::TenantId,
            subject::Column::SourceSlug,
            subject::Column::SubjectKind,
            subject::Column::SubjectId,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(transaction)
    .await
    .map_err(database_error)?;

    let stored = find_subject(transaction, canonical_subject)
        .await?
        .ok_or_else(|| {
            PortError::invariant_violation(
                "reactions.subject_insert_missing",
                "reaction subject row was not observable after admission",
            )
        })?;

    subject::Entity::update_many()
        .col_expr(subject::Column::UpdatedAt, Expr::value(now))
        .filter(subject::Column::Id.eq(stored.id))
        .filter(subject::Column::TenantId.eq(tenant_id))
        .exec(transaction)
        .await
        .map_err(database_error)?;

    let stored = subject::Entity::find_by_id(stored.id)
        .filter(subject::Column::TenantId.eq(tenant_id))
        .one(transaction)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            PortError::invariant_violation(
                "reactions.subject_lock_missing",
                "reaction subject row disappeared during command serialization",
            )
        })?;

    if subject_revision < stored.subject_revision {
        return Err(PortError::conflict(
            "reactions.subject_revision_stale",
            "reaction command targets a stale subject revision",
        ));
    }
    if catalog_revision < stored.current_catalog_revision {
        return Err(PortError::conflict(
            "reactions.catalog_revision_stale",
            "reaction command targets a stale catalog revision",
        ));
    }

    let catalog_json = serde_json::to_value(catalog_value).map_err(|error| {
        PortError::invariant_violation("reactions.catalog_encode", error.to_string())
    })?;
    if let Some(existing_catalog) = catalog::Entity::find()
        .filter(catalog::Column::TenantId.eq(tenant_id))
        .filter(catalog::Column::ReactionSubjectId.eq(stored.id))
        .filter(catalog::Column::CatalogRevision.eq(catalog_revision))
        .one(transaction)
        .await
        .map_err(database_error)?
    {
        if existing_catalog.catalog_json != catalog_json {
            return Err(PortError::conflict(
                "reactions.catalog_revision_rebound",
                "reaction catalog revision is already bound to different content",
            ));
        }
    } else {
        ensure_catalog_accepts_live_aggregates(transaction, tenant_id, stored.id, catalog_value)
            .await?;
        catalog::Entity::insert(catalog::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            reaction_subject_id: Set(stored.id),
            catalog_revision: Set(catalog_revision),
            catalog_json: Set(catalog_json),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                catalog::Column::TenantId,
                catalog::Column::ReactionSubjectId,
                catalog::Column::CatalogRevision,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map_err(database_error)?;

        let persisted = catalog::Entity::find()
            .filter(catalog::Column::TenantId.eq(tenant_id))
            .filter(catalog::Column::ReactionSubjectId.eq(stored.id))
            .filter(catalog::Column::CatalogRevision.eq(catalog_revision))
            .one(transaction)
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "reactions.catalog_insert_missing",
                    "reaction catalog row was not observable after insertion",
                )
            })?;
        if persisted.catalog_json
            != serde_json::to_value(catalog_value).map_err(|error| {
                PortError::invariant_violation("reactions.catalog_encode", error.to_string())
            })?
        {
            return Err(PortError::conflict(
                "reactions.catalog_revision_rebound",
                "reaction catalog revision is already bound to different content",
            ));
        }
    }

    if subject_revision != stored.subject_revision
        || catalog_revision != stored.current_catalog_revision
    {
        subject::Entity::update_many()
            .col_expr(
                subject::Column::SubjectRevision,
                Expr::value(subject_revision),
            )
            .col_expr(
                subject::Column::CurrentCatalogRevision,
                Expr::value(catalog_revision),
            )
            .col_expr(subject::Column::UpdatedAt, Expr::value(now))
            .filter(subject::Column::Id.eq(stored.id))
            .filter(subject::Column::TenantId.eq(tenant_id))
            .exec(transaction)
            .await
            .map_err(database_error)?;
    }

    subject::Entity::find_by_id(stored.id)
        .filter(subject::Column::TenantId.eq(tenant_id))
        .one(transaction)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            PortError::invariant_violation(
                "reactions.subject_update_missing",
                "reaction subject row disappeared after catalog synchronization",
            )
        })
}

async fn ensure_catalog_accepts_live_aggregates(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    reaction_subject_id: Uuid,
    catalog_value: &ReactionCatalog,
) -> Result<(), PortError> {
    let rows = aggregate::Entity::find()
        .filter(aggregate::Column::TenantId.eq(tenant_id))
        .filter(aggregate::Column::ReactionSubjectId.eq(reaction_subject_id))
        .filter(aggregate::Column::Count.gt(0_i64))
        .all(transaction)
        .await
        .map_err(database_error)?;
    for row in rows {
        let key = ReactionKey::new(row.reaction_key).map_err(contract_error_to_port_error)?;
        if !catalog_value.contains(&key) {
            return Err(PortError::conflict(
                "reactions.catalog_reconciliation_required",
                "new reaction catalog removes a key with live aggregate state",
            ));
        }
    }
    Ok(())
}

async fn persist_actor_state(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    reaction_subject_id: Uuid,
    actor_id: Uuid,
    state_id: Uuid,
    created_at: chrono::DateTime<chrono::FixedOffset>,
    revision: u64,
    selected: &[ReactionKey],
    exists: bool,
) -> Result<(), PortError> {
    let revision = u64_to_i64(revision, "reaction actor state revision")?;
    let selected_json = serde_json::to_value(selected).map_err(|error| {
        PortError::invariant_violation("reactions.actor_state_encode", error.to_string())
    })?;
    let now = Utc::now().fixed_offset();

    if exists {
        let update = actor_state::Entity::update_many()
            .col_expr(actor_state::Column::Revision, Expr::value(revision))
            .col_expr(
                actor_state::Column::SelectedJson,
                Expr::value(selected_json),
            )
            .col_expr(actor_state::Column::UpdatedAt, Expr::value(now))
            .filter(actor_state::Column::Id.eq(state_id))
            .filter(actor_state::Column::TenantId.eq(tenant_id))
            .exec(transaction)
            .await
            .map_err(database_error)?;
        if update.rows_affected != 1 {
            return Err(PortError::invariant_violation(
                "reactions.actor_state_update_lost",
                "reaction actor state update did not affect one row",
            ));
        }
    } else {
        actor_state::ActiveModel {
            id: Set(state_id),
            tenant_id: Set(tenant_id),
            reaction_subject_id: Set(reaction_subject_id),
            actor_id: Set(actor_id),
            revision: Set(revision),
            selected_json: Set(selected_json),
            created_at: Set(created_at),
            updated_at: Set(now),
        }
        .insert(transaction)
        .await
        .map_err(database_error)?;
    }
    Ok(())
}

async fn apply_aggregate_delta(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    reaction_subject_id: Uuid,
    reaction: ReactionKey,
    delta: i64,
) -> Result<(), PortError> {
    let existing = aggregate::Entity::find()
        .filter(aggregate::Column::TenantId.eq(tenant_id))
        .filter(aggregate::Column::ReactionSubjectId.eq(reaction_subject_id))
        .filter(aggregate::Column::ReactionKey.eq(reaction.as_str()))
        .one(transaction)
        .await
        .map_err(database_error)?;
    let now = Utc::now().fixed_offset();

    match existing {
        Some(row) => {
            if row.count < 0 {
                return Err(PortError::invariant_violation(
                    "reactions.aggregate_count_invalid",
                    "stored reaction aggregate count is negative",
                ));
            }
            let next = row.count.checked_add(delta).ok_or_else(|| {
                PortError::invariant_violation(
                    "reactions.aggregate_count_exhausted",
                    "reaction aggregate count overflowed",
                )
            })?;
            if next < 0 {
                return Err(PortError::invariant_violation(
                    "reactions.aggregate_count_underflow",
                    "reaction aggregate count would become negative",
                ));
            }
            if next == 0 {
                aggregate::Entity::delete_by_id(row.id)
                    .exec(transaction)
                    .await
                    .map_err(database_error)?;
            } else {
                let update = aggregate::Entity::update_many()
                    .col_expr(aggregate::Column::Count, Expr::value(next))
                    .col_expr(aggregate::Column::UpdatedAt, Expr::value(now))
                    .filter(aggregate::Column::Id.eq(row.id))
                    .filter(aggregate::Column::TenantId.eq(tenant_id))
                    .exec(transaction)
                    .await
                    .map_err(database_error)?;
                if update.rows_affected != 1 {
                    return Err(PortError::invariant_violation(
                        "reactions.aggregate_update_lost",
                        "reaction aggregate update did not affect one row",
                    ));
                }
            }
        }
        None if delta > 0 => {
            aggregate::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                reaction_subject_id: Set(reaction_subject_id),
                reaction_key: Set(reaction.to_string()),
                count: Set(delta),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(transaction)
            .await
            .map_err(database_error)?;
        }
        None => {
            return Err(PortError::invariant_violation(
                "reactions.aggregate_missing",
                "reaction aggregate row is missing for a decrement",
            ));
        }
    }
    Ok(())
}

async fn find_subject<C>(
    connection: &C,
    subject_ref: &ReactionSubjectRef,
) -> Result<Option<subject::Model>, PortError>
where
    C: sea_orm::ConnectionTrait,
{
    subject::Entity::find()
        .filter(subject::Column::TenantId.eq(subject_ref.tenant_id()))
        .filter(subject::Column::SourceSlug.eq(subject_ref.source().as_str()))
        .filter(subject::Column::SubjectKind.eq(subject_ref.kind().as_str()))
        .filter(subject::Column::SubjectId.eq(subject_ref.subject_id()))
        .one(connection)
        .await
        .map_err(database_error)
}

fn decode_actor_state(row: actor_state::Model) -> Result<ReactionActorState, PortError> {
    let revision = i64_to_u64(row.revision, "reaction actor state revision")?;
    let selected = decode_selected(row.selected_json)?;
    ReactionActorState::try_new(revision, selected).map_err(contract_error_to_port_error)
}

fn decode_selected(value: serde_json::Value) -> Result<Vec<ReactionKey>, PortError> {
    let selected = serde_json::from_value::<Vec<ReactionKey>>(value).map_err(|error| {
        PortError::invariant_violation("reactions.actor_state_corrupt", error.to_string())
    })?;
    if selected.iter().collect::<BTreeSet<_>>().len() != selected.len() {
        return Err(PortError::invariant_violation(
            "reactions.actor_state_corrupt",
            "stored reaction actor state contains duplicate keys",
        ));
    }
    Ok(selected)
}

fn validate_subject_tenant(
    context: &PortContext,
    subject_ref: &ReactionSubjectRef,
) -> Result<(), PortError> {
    let tenant_id = parse_tenant_id(context)?;
    if tenant_id != subject_ref.tenant_id() {
        return Err(PortError::forbidden(
            "reactions.tenant_mismatch",
            "reaction subject does not belong to the port tenant",
        ));
    }
    Ok(())
}

fn authorize_actor(context: &PortContext, actor_id: Uuid) -> Result<(), PortError> {
    match &context.actor.kind {
        PortActorKind::User => {
            let context_actor = Uuid::parse_str(&context.actor.id).map_err(|_| {
                PortError::validation(
                    "reactions.actor_id_invalid",
                    "reaction user actor identity must be a UUID",
                )
            })?;
            if context_actor != actor_id {
                return Err(PortError::forbidden(
                    "reactions.actor_mismatch",
                    "reaction actor does not match the authenticated user",
                ));
            }
        }
        PortActorKind::Service | PortActorKind::System => {
            if !context
                .claims
                .iter()
                .any(|claim| claim == ACT_AS_ACTOR_CLAIM)
            {
                return Err(PortError::forbidden(
                    "reactions.actor_delegation_forbidden",
                    "service reaction access requires the act-as-actor claim",
                ));
            }
        }
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "reactions.tenant_id_invalid",
            "reaction port context must carry a UUID tenant_id",
        )
    })
}

fn provider_error_to_port_error(error: ReactionProviderError) -> PortError {
    match error {
        ReactionProviderError::CapabilityUnavailable { .. }
        | ReactionProviderError::Internal { .. } => PortError::unavailable(
            "reactions.subject_provider_failed",
            "reaction subject provider is temporarily unavailable",
        ),
        ReactionProviderError::InvalidRequest => PortError::validation(
            "reactions.subject_request_invalid",
            "reaction subject provider rejected the request shape",
        ),
        ReactionProviderError::Unavailable => PortError::not_found(
            "reactions.subject_unavailable",
            "reaction subject is unavailable",
        ),
        ReactionProviderError::Conflict => PortError::conflict(
            "reactions.subject_revision_conflict",
            "reaction subject changed concurrently",
        ),
    }
}

fn contract_error_to_port_error(error: ReactionContractError) -> PortError {
    PortError::validation("reactions.contract_invalid", error.to_string())
}

fn database_error(error: sea_orm::DbErr) -> PortError {
    PortError::unavailable("reactions.database", error.to_string())
}

fn u64_to_i64(value: u64, label: &'static str) -> Result<i64, PortError> {
    i64::try_from(value).map_err(|_| {
        PortError::invariant_violation(
            "reactions.revision_out_of_range",
            format!("{label} exceeds the persistence range"),
        )
    })
}

fn i64_to_u64(value: i64, label: &'static str) -> Result<u64, PortError> {
    u64::try_from(value).map_err(|_| {
        PortError::invariant_violation("reactions.revision_invalid", format!("{label} is negative"))
    })
}

fn decode_replay<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use rustok_reactions_api::{ReactionAction, ReactionKey, ReactionSelectionPolicy};

    use super::plan_selection;

    fn key(value: &str) -> ReactionKey {
        ReactionKey::new(value).expect("valid reaction key")
    }

    #[test]
    fn single_selection_replaces_the_previous_key_atomically() {
        let previous = key("like");
        let next = key("love");
        let (changed, selected, deltas) = plan_selection(
            ReactionSelectionPolicy::Single,
            std::slice::from_ref(&previous),
            &next,
            ReactionAction::Add,
        )
        .expect("single selection should replace");

        assert!(changed);
        assert_eq!(selected, vec![next.clone()]);
        assert_eq!(deltas.get(&previous), Some(&-1));
        assert_eq!(deltas.get(&next), Some(&1));
    }

    #[test]
    fn bounded_multiple_selection_rejects_excess_keys() {
        let selected = vec![key("like"), key("love")];
        let error = plan_selection(
            ReactionSelectionPolicy::multiple(2).expect("policy"),
            &selected,
            &key("insightful"),
            ReactionAction::Add,
        )
        .expect_err("selection must remain bounded");

        assert_eq!(error.code, "reactions.selection_limit_reached");
    }

    #[test]
    fn removing_an_absent_key_is_an_idempotent_noop() {
        let selected = vec![key("like")];
        let (changed, next, deltas) = plan_selection(
            ReactionSelectionPolicy::Single,
            &selected,
            &key("love"),
            ReactionAction::Remove,
        )
        .expect("remove should be accepted");

        assert!(!changed);
        assert_eq!(next, selected);
        assert!(deltas.is_empty());
    }
}
