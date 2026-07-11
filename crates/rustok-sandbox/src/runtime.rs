use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;

use crate::{
    CapabilityBroker, ExecutionRecord, ExecutionStatus, ExecutorRegistry, SandboxHost,
    SandboxOutcome, SandboxRequest, SandboxResult,
};

#[async_trait]
pub trait ExecutionObserver: Send + Sync {
    async fn observe(&self, record: &ExecutionRecord);
}

pub struct NoopExecutionObserver;

#[async_trait]
impl ExecutionObserver for NoopExecutionObserver {
    async fn observe(&self, _record: &ExecutionRecord) {}
}

#[derive(Clone)]
pub struct SandboxRuntime {
    executors: ExecutorRegistry,
    broker: Arc<dyn CapabilityBroker>,
    observers: Vec<Arc<dyn ExecutionObserver>>,
}

impl SandboxRuntime {
    pub fn new(executors: ExecutorRegistry, broker: Arc<dyn CapabilityBroker>) -> Self {
        Self {
            executors,
            broker,
            observers: Vec::new(),
        }
    }

    pub fn with_observer(mut self, observer: Arc<dyn ExecutionObserver>) -> Self {
        self.observers.push(observer);
        self
    }

    pub async fn execute(&self, request: SandboxRequest) -> SandboxResult<SandboxOutcome> {
        request.validate()?;
        let executor = self.executors.get(request.payload.executor)?;
        let started_at = Utc::now();
        self.observe(ExecutionRecord {
            execution_id: request.context.execution_id,
            subject: request.subject.clone(),
            executor: request.payload.executor,
            status: ExecutionStatus::Started,
            started_at,
            finished_at: None,
            metrics: None,
            error_code: None,
            error_message: None,
        })
        .await;

        let timer = Instant::now();
        let host = SandboxHost::new(Arc::new(request.policy.clone()), Arc::clone(&self.broker));
        let result = executor.execute(&request, host).await;

        match result {
            Ok(mut outcome) => {
                outcome.execution_id = request.context.execution_id;
                outcome.metrics.duration_ms = timer.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
                self.observe(ExecutionRecord {
                    execution_id: request.context.execution_id,
                    subject: request.subject,
                    executor: request.payload.executor,
                    status: ExecutionStatus::Succeeded,
                    started_at,
                    finished_at: Some(Utc::now()),
                    metrics: Some(outcome.metrics.clone()),
                    error_code: None,
                    error_message: None,
                })
                .await;
                Ok(outcome)
            }
            Err(error) => {
                self.observe(ExecutionRecord {
                    execution_id: request.context.execution_id,
                    subject: request.subject,
                    executor: request.payload.executor,
                    status: ExecutionStatus::Failed,
                    started_at,
                    finished_at: Some(Utc::now()),
                    metrics: None,
                    error_code: Some(error.code().to_string()),
                    error_message: Some(error.to_string()),
                })
                .await;
                Err(error)
            }
        }
    }

    async fn observe(&self, record: ExecutionRecord) {
        for observer in &self.observers {
            observer.observe(&record).await;
        }
    }
}

