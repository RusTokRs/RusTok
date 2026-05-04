mod hook_executor;
mod scriptable;

pub use hook_executor::{BeforeHookResult, HookExecutor};
pub use scriptable::ScriptableEntity;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use rhai::Dynamic;
    use uuid::Uuid;

    use crate::create_default_engine;
    use crate::integration::ScriptableEntity;
    use crate::model::{EventType, Script, ScriptTrigger};
    use crate::runner::{ExecutionOutcome, ScriptOrchestrator};
    use crate::storage::{InMemoryStorage, ScriptRegistry};
    use crate::EntityProxy;

    #[derive(Debug, Clone)]
    struct Deal {
        id: String,
        name: String,
        amount: f64,
        status: String,
        assigned_to: Option<String>,
    }

    impl ScriptableEntity for Deal {
        fn entity_type(&self) -> &'static str {
            "deal"
        }

        fn id(&self) -> String {
            self.id.clone()
        }

        fn to_dynamic_map(&self) -> HashMap<String, Dynamic> {
            let mut map = HashMap::new();
            map.insert("id".into(), Dynamic::from(self.id.clone()));
            map.insert("name".into(), Dynamic::from(self.name.clone()));
            map.insert("amount".into(), Dynamic::from(self.amount));
            map.insert("status".into(), Dynamic::from(self.status.clone()));
            if let Some(ref assigned_to) = self.assigned_to {
                map.insert("assigned_to".into(), Dynamic::from(assigned_to.clone()));
            }
            map
        }

        fn apply_changes(&mut self, changes: HashMap<String, Dynamic>) {
            for (key, value) in changes {
                match key.as_str() {
                    "name" => {
                        if let Some(v) = value.clone().try_cast::<String>() {
                            self.name = v;
                        }
                    }
                    "amount" => {
                        if let Some(v) = value.clone().try_cast::<f64>() {
                            self.amount = v;
                        }
                    }
                    "status" => {
                        if let Some(v) = value.clone().try_cast::<String>() {
                            self.status = v;
                        }
                    }
                    "assigned_to" => {
                        self.assigned_to = value.clone().try_cast::<String>();
                    }
                    _ => {}
                }
            }
        }
    }

    struct DealService<R: ScriptRegistry> {
        orchestrator: Arc<ScriptOrchestrator<R>>,
    }

    impl<R: crate::storage::ScriptRegistry> DealService<R> {
        fn new(orchestrator: Arc<ScriptOrchestrator<R>>) -> Self {
            Self { orchestrator }
        }

        async fn create(&self, mut deal: Deal) -> Result<Deal, ServiceError> {
            if deal.id.is_empty() {
                deal.id = Uuid::new_v4().to_string();
            }

            let proxy = EntityProxy::new(&deal.id, deal.entity_type(), deal.to_dynamic_map());

            let before = self
                .orchestrator
                .run_before(deal.entity_type(), EventType::BeforeCreate, proxy, None)
                .await;

            match before {
                crate::runner::HookOutcome::Continue { changes } => {
                    if !changes.is_empty() {
                        deal.apply_changes(changes);
                    }
                }
                crate::runner::HookOutcome::Rejected { reason } => {
                    return Err(ServiceError::ValidationFailed(reason));
                }
                crate::runner::HookOutcome::Error { error } => {
                    return Err(ServiceError::ScriptError(error.to_string()));
                }
            }

            // simulate DB insert

            let after_proxy = EntityProxy::new(&deal.id, deal.entity_type(), deal.to_dynamic_map());

            let after = self
                .orchestrator
                .run_after(
                    deal.entity_type(),
                    EventType::AfterCreate,
                    after_proxy,
                    None,
                    None,
                )
                .await;

            if let crate::runner::HookOutcome::Error { error } = after {
                return Err(ServiceError::ScriptError(error.to_string()));
            }

            let on_commit_proxy =
                EntityProxy::new(&deal.id, deal.entity_type(), deal.to_dynamic_map());

            let results = self
                .orchestrator
                .run_on_commit(deal.entity_type(), on_commit_proxy, None)
                .await;

            if results
                .iter()
                .any(|result| matches!(result.outcome, ExecutionOutcome::Failed { .. }))
            {
                return Err(ServiceError::ScriptError(
                    "on_commit scripts failed".to_string(),
                ));
            }

            Ok(deal)
        }
    }

    #[derive(Debug, thiserror::Error)]
    enum ServiceError {
        #[error("Validation failed: {0}")]
        ValidationFailed(String),
        #[error("Script error: {0}")]
        ScriptError(String),
    }

    #[tokio::test]
    async fn test_deal_creation_with_scripts() {
        let engine = Arc::new(create_default_engine());
        let storage = Arc::new(InMemoryStorage::new());
        let orchestrator = Arc::new(ScriptOrchestrator::new(engine, storage.clone()));
        let service = DealService::new(orchestrator);

        let mut validation_script = Script::new(
            "validate_deal",
            r#"
                if entity.amount < 100.0 {
                    abort("Minimum deal amount is 100");
                }
                if entity.amount > 100000.0 {
                    entity["status"] = "needs_approval";
                    entity["assigned_to"] = "senior_manager";
                }
            "#,
            ScriptTrigger::Event {
                entity_type: "deal".into(),
                event: EventType::BeforeCreate,
            },
        );
        validation_script.activate();
        storage.save(validation_script).await.unwrap();

        let mut commit_script = Script::new(
            "notify_commit",
            r#"
                log("commit hook ran");
            "#,
            ScriptTrigger::Event {
                entity_type: "deal".into(),
                event: EventType::OnCommit,
            },
        );
        commit_script.activate();
        storage.save(commit_script).await.unwrap();

        let small_deal = Deal {
            id: String::new(),
            name: "Small deal".into(),
            amount: 50.0,
            status: "new".into(),
            assigned_to: None,
        };

        let result = service.create(small_deal).await;
        match result {
            Err(ServiceError::ValidationFailed(_)) | Err(ServiceError::ScriptError(_)) => {}
            other => panic!("expected validation failure for small deal, got: {other:?}"),
        }

        let big_deal = Deal {
            id: String::new(),
            name: "Big deal".into(),
            amount: 200000.0,
            status: "new".into(),
            assigned_to: None,
        };

        let result = service.create(big_deal).await.unwrap();
        assert_eq!(result.status, "needs_approval");
        assert_eq!(result.assigned_to.as_deref(), Some("senior_manager"));
    }
}
