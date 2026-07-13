use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;

use crate::{
    CapabilityBroker, CapabilityObserver, ExecutionRecord, ExecutionStatus, ExecutorRegistry,
    SandboxAdmissionLimits, SandboxCancellation, SandboxHost, SandboxOutcome, SandboxRequest,
    SandboxResult,
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
    capability_observers: Arc<Vec<Arc<dyn CapabilityObserver>>>,
    admission: crate::admission::AdmissionController,
}

impl SandboxRuntime {
    pub fn new(executors: ExecutorRegistry, broker: Arc<dyn CapabilityBroker>) -> Self {
        Self {
            executors,
            broker,
            observers: Vec::new(),
            capability_observers: Arc::new(Vec::new()),
            admission: crate::admission::AdmissionController::new(SandboxAdmissionLimits::default()),
        }
    }

    pub fn with_observer(mut self, observer: Arc<dyn ExecutionObserver>) -> Self {
        self.observers.push(observer);
        self
    }

    pub fn with_capability_observer(mut self, observer: Arc<dyn CapabilityObserver>) -> Self {
        Arc::make_mut(&mut self.capability_observers).push(observer);
        self
    }

    pub fn with_admission_limits(mut self, limits: SandboxAdmissionLimits) -> Self {
        self.admission = crate::admission::AdmissionController::new(limits);
        self
    }

    pub async fn execute(&self, request: SandboxRequest) -> SandboxResult<SandboxOutcome> {
        self.execute_with_cancellation(request, SandboxCancellation::new())
            .await
    }

    pub async fn execute_with_cancellation(
        &self,
        request: SandboxRequest,
        cancellation: SandboxCancellation,
    ) -> SandboxResult<SandboxOutcome> {
        request.validate()?;
        if cancellation.is_cancelled() {
            return Err(crate::SandboxError::Cancelled);
        }
        let executor = self.executors.get(request.payload.executor)?;
        let _permit = self.admission.admit(&request)?;
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
        })
        .await;

        let timer = Instant::now();
        let host = SandboxHost::new(
            Arc::new(request.policy.clone()),
            Arc::clone(&self.broker),
            request.subject.clone(),
            &request.context,
            Arc::clone(&self.capability_observers),
            cancellation,
        );
        let result = executor.execute(&request, host).await;

        match result {
            Ok(mut outcome) => {
                outcome.execution_id = request.context.execution_id;
                outcome.metrics.duration_ms =
                    timer.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
                self.observe(ExecutionRecord {
                    execution_id: request.context.execution_id,
                    subject: request.subject,
                    executor: request.payload.executor,
                    status: ExecutionStatus::Succeeded,
                    started_at,
                    finished_at: Some(Utc::now()),
                    metrics: Some(outcome.metrics.clone()),
                    error_code: None,
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
