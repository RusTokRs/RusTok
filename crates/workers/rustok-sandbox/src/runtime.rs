use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    CapabilityBroker, CapabilityObserver, ExecutionRecord, ExecutionStatus, ExecutorRegistry,
    SandboxAdmissionLimits, SandboxCancellation, SandboxContext, SandboxError, SandboxExecutor,
    SandboxHost, SandboxOutcome, SandboxRequest, SandboxResult,
};

#[async_trait]
pub trait ExecutionObserver: Send + Sync {
    async fn observe(&self, record: &ExecutionRecord) -> SandboxResult<()>;
}

pub struct NoopExecutionObserver;

#[async_trait]
impl ExecutionObserver for NoopExecutionObserver {
    async fn observe(&self, _record: &ExecutionRecord) -> SandboxResult<()> {
        Ok(())
    }
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

    pub fn supports_executor(&self, kind: crate::SandboxExecutorKind) -> bool {
        self.executors.contains(kind)
    }

    pub fn executor_placement(
        &self,
        kind: crate::SandboxExecutorKind,
    ) -> SandboxResult<crate::SandboxExecutorPlacement> {
        self.executors.placement(kind)
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
        let queue_timer = Instant::now();
        let (executor, _permit, started_at) =
            self.prepare_and_observe_execution(&request, &cancellation).await?;
        let queue_time_ms = elapsed_millis(queue_timer);
        let (result, duration_ms, calls) =
            self.run_executor(executor, &request, cancellation).await;
        let context = request.context.clone();

        match result {
            Ok(outcome) => {
                self.handle_execution_success(
                    outcome,
                    &request,
                    context,
                    started_at,
                    queue_time_ms,
                    duration_ms,
                    calls,
                )
                .await
            }
            Err(error) => {
                self.handle_execution_failure(
                    error,
                    &request,
                    context,
                    started_at,
                    queue_time_ms,
                    duration_ms,
                    calls,
                )
                .await
            }
        }
    }

    async fn prepare_and_observe_execution(
        &self,
        request: &SandboxRequest,
        cancellation: &SandboxCancellation,
    ) -> SandboxResult<(
        Arc<dyn SandboxExecutor>,
        crate::admission::AdmissionPermit,
        DateTime<Utc>,
    )> {
        request.validate()?;
        if cancellation.is_cancelled() {
            return Err(crate::SandboxError::Cancelled);
        }
        let executor = self.executors.get(request.payload.executor)?;
        let permit = self.admission.admit(request)?;
        let started_at = Utc::now();
        self.observe_started(request, &request.context, started_at).await;
        Ok((executor, permit, started_at))
    }

    async fn run_executor(
        &self,
        executor: Arc<dyn SandboxExecutor>,
        request: &SandboxRequest,
        cancellation: SandboxCancellation,
    ) -> (SandboxResult<SandboxOutcome>, u64, u32) {
        let execution_timer = Instant::now();
        let host = SandboxHost::new(
            Arc::new(request.policy.clone()),
            Arc::clone(&self.broker),
            request.subject.clone(),
            &request.context,
            Arc::clone(&self.capability_observers),
            cancellation,
        );
        let result = executor.execute(request, host.clone()).await;
        let duration_ms = elapsed_millis(execution_timer);
        let capability_calls = host.capability_calls();
        (result, duration_ms, capability_calls)
    }

    async fn observe_started(
        &self,
        request: &SandboxRequest,
        context: &SandboxContext,
        started_at: DateTime<Utc>,
    ) {
        self.observe_best_effort(ExecutionRecord {
            execution_id: context.execution_id,
            subject: request.subject.clone(),
            context: context.clone(),
            executor: request.payload.executor,
            status: ExecutionStatus::Started,
            started_at,
            finished_at: None,
            metrics: None,
            error_code: None,
        })
        .await;
    }

    async fn handle_execution_success(
        &self,
        mut outcome: SandboxOutcome,
        request: &SandboxRequest,
        context: SandboxContext,
        started_at: DateTime<Utc>,
        queue_time_ms: u64,
        duration_ms: u64,
        capability_calls: u32,
    ) -> SandboxResult<SandboxOutcome> {
        outcome.execution_id = context.execution_id;
        outcome.metrics.queue_time_ms = queue_time_ms;
        outcome.metrics.duration_ms = duration_ms;
        outcome.metrics.capability_calls = capability_calls;
        self.observe_best_effort(ExecutionRecord {
            execution_id: context.execution_id,
            subject: request.subject.clone(),
            context,
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

    async fn handle_execution_failure(
        &self,
        error: SandboxError,
        request: &SandboxRequest,
        context: SandboxContext,
        started_at: DateTime<Utc>,
        queue_time_ms: u64,
        duration_ms: u64,
        capability_calls: u32,
    ) -> SandboxResult<SandboxOutcome> {
        let metrics = crate::ExecutionMetrics {
            queue_time_ms,
            duration_ms,
            capability_calls,
            ..Default::default()
        };
        self.observe_best_effort(ExecutionRecord {
            execution_id: context.execution_id,
            subject: request.subject.clone(),
            context,
            executor: request.payload.executor,
            status: ExecutionStatus::Failed,
            started_at,
            finished_at: Some(Utc::now()),
            metrics: Some(metrics),
            error_code: Some(error.code().to_string()),
        })
        .await;
        Err(error)
    }

    async fn observe_best_effort(&self, record: ExecutionRecord) {
        for observer in &self.observers {
            if let Err(error) = observer.observe(&record).await {
                tracing::error!(
                    execution_id = %record.execution_id,
                    error = ?error,
                    "sandbox execution observer failed"
                );
            }
        }
    }

    pub async fn observe(&self, record: &ExecutionRecord) -> SandboxResult<()> {
        for observer in &self.observers {
            observer.observe(record).await?;
        }
        Ok(())
    }
}

fn elapsed_millis(timer: Instant) -> u64 {
    timer.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}
