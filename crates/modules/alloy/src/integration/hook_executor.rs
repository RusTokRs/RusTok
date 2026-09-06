use std::collections::HashMap;
use std::sync::Arc;

use rhai::Dynamic;

use crate::error::ScriptResult;
use crate::model::{EntityProxy, EventType};
use crate::runner::{HookOutcome, ScriptOrchestrator};
use crate::storage::ScriptRegistry;

pub enum BeforeHookResult {
    Continue(HashMap<String, Dynamic>),
    Rejected(String),
}

pub struct HookExecutor<S: ScriptRegistry> {
    orchestrator: Arc<ScriptOrchestrator<S>>,
}

impl<S: ScriptRegistry> HookExecutor<S> {
    pub fn new(orchestrator: Arc<ScriptOrchestrator<S>>) -> Self {
        Self { orchestrator }
    }

    pub async fn run_before(
        &self,
        entity_type: &str,
        event: EventType,
        entity_id: &str,
        data: HashMap<String, Dynamic>,
        user_id: Option<String>,
    ) -> ScriptResult<BeforeHookResult> {
        let proxy = EntityProxy::new(entity_id, entity_type, data);

        match self
            .orchestrator
            .run_before(entity_type, event, proxy, user_id)
            .await
        {
            HookOutcome::Continue { changes } => Ok(BeforeHookResult::Continue(changes)),
            HookOutcome::Rejected { reason } => Ok(BeforeHookResult::Rejected(reason)),
            HookOutcome::Error { error } => Err(error),
        }
    }

    pub async fn run_after(
        &self,
        entity_type: &str,
        event: EventType,
        entity_id: &str,
        data: HashMap<String, Dynamic>,
        user_id: Option<String>,
    ) -> HookOutcome {
        let proxy = EntityProxy::new(entity_id, entity_type, data);

        self.orchestrator
            .run_after(entity_type, event, proxy, None, user_id)
            .await
    }

    pub async fn run_on_commit(
        &self,
        entity_type: &str,
        entity_id: &str,
        data: HashMap<String, Dynamic>,
        user_id: Option<String>,
    ) -> Vec<crate::runner::ExecutionResult> {
        let proxy = EntityProxy::new(entity_id, entity_type, data);

        self.orchestrator
            .run_on_commit(entity_type, proxy, user_id)
            .await
    }
}
