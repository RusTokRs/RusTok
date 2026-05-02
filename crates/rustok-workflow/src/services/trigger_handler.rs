use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;
use tracing::{error, info};

use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};

use crate::entities::{workflow, WorkflowEntity, WorkflowStatus};
use crate::services::{WorkflowEngine, WorkflowService};

/// Subscribes to all domain events and triggers matching active workflows.
pub struct WorkflowTriggerHandler {
    db: DatabaseConnection,
    engine: Arc<WorkflowEngine>,
    service: Arc<WorkflowService>,
}

impl WorkflowTriggerHandler {
    pub fn new(db: DatabaseConnection) -> Self {
        let engine = Arc::new(WorkflowEngine::new(db.clone()));
        let service = Arc::new(WorkflowService::new(db.clone()));
        Self {
            db,
            engine,
            service,
        }
    }
}

#[async_trait]
impl EventHandler for WorkflowTriggerHandler {
    fn name(&self) -> &'static str {
        "WorkflowTriggerHandler"
    }

    fn handles(&self, _event: &rustok_core::events::DomainEvent) -> bool {
        // Subscribe to all events — filtering is done in handle() by matching trigger_config
        true
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let event_type = &envelope.event_type;
        let tenant_id = envelope.tenant_id;

        // Find all active workflows for this tenant with an event trigger matching this event type
        let workflows = WorkflowEntity::find()
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .filter(workflow::Column::Status.eq(WorkflowStatus::Active.to_string()))
            .all(&self.db)
            .await
            .map_err(|e| {
                rustok_core::Error::External(format!("DB error in WorkflowTriggerHandler: {e}"))
            })?;

        let matching: Vec<_> = workflows
            .into_iter()
            .filter(|w| {
                // Check if trigger_config matches: {"type": "event", "event_type": "<pattern>"}
                matches_event_trigger(&w.trigger_config, event_type)
            })
            .collect();

        if matching.is_empty() {
            return Ok(());
        }

        info!(
            event_type = event_type,
            tenant_id = %tenant_id,
            count = matching.len(),
            "Triggering workflows for event"
        );

        // Build initial context from the event envelope
        let initial_context = json!({
            "event": {
                "id": envelope.id,
                "type": envelope.event_type,
                "tenant_id": envelope.tenant_id,
                "timestamp": envelope.timestamp,
                "actor_id": envelope.actor_id,
            }
        });

        for workflow in matching {
            let workflow_id = workflow.id;
            let steps = match self.service.load_steps(workflow_id).await {
                Ok(s) => s,
                Err(e) => {
                    error!(workflow_id = %workflow_id, error = %e, "Failed to load steps");
                    continue;
                }
            };

            let engine = self.engine.clone();
            let ctx = initial_context.clone();
            let event_id = envelope.id;

            tokio::spawn(async move {
                if let Err(e) = engine
                    .execute(workflow_id, tenant_id, Some(event_id), steps, ctx)
                    .await
                {
                    error!(workflow_id = %workflow_id, error = %e, "Workflow execution failed");
                }
            });
        }

        Ok(())
    }
}

fn matches_event_trigger(trigger_config: &serde_json::Value, event_type: &str) -> bool {
    let trigger_type = trigger_config.get("type").and_then(|v| v.as_str());
    if trigger_type != Some("event") {
        return false;
    }
    match trigger_config.get("event_type").and_then(|v| v.as_str()) {
        Some(pattern) => {
            if let Some(prefix) = pattern.strip_suffix('*') {
                event_type.starts_with(prefix)
            } else {
                event_type == pattern
            }
        }
        None => false,
    }
}
