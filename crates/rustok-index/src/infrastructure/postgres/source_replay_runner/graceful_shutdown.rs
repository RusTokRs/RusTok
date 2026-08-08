use super::*;

impl PostgresIndexReplayRunner {
    /// Runs replay with a host-owned cooperative interruption probe.
    ///
    /// Host interruption is distinct from persisted operator cancellation. The existing worker checks
    /// the probe only at durable page boundaries: before source scan, before each mutation apply, and
    /// before checkpoint commit. When the probe interrupts a page, this runner yields its current job
    /// lease back to `pending` so a fresh attempt can resume from the last committed checkpoint.
    /// Already-durable mutations from the interrupted page are intentionally replayed and rely on the
    /// canonical inbox/source-version idempotency contract.
    pub async fn run_interruptible<Check>(
        &self,
        request: IndexReplayRunRequest,
        mut should_interrupt: Check,
    ) -> Result<IndexReplayRunOutcome, IndexReplayRunError>
    where
        Check: FnMut() -> bool,
    {
        let source_name = self
            .sources
            .source_for_schema(request.page_request().schema())
            .ok_or_else(|| {
                IndexReplayRunError::UnknownSchemaSource(
                    request.page_request().schema().clone(),
                )
            })?
            .source_name()
            .to_owned();
        let lease_request = IndexReplayJobLeaseRequest::new(
            request.page_request().tenant_id(),
            request.page_request().schema().clone(),
            source_name,
            request.worker_id().to_owned(),
            request.lease_duration(),
        )?;
        let job_store = PostgresIndexReplayJobStore::new(self.db.clone());
        let lease = match job_store.acquire(&lease_request).await? {
            IndexReplayJobAcquireOutcome::Busy => {
                return Ok(empty_outcome(IndexReplayRunStatus::Busy, None, None));
            }
            IndexReplayJobAcquireOutcome::AlreadyComplete { job_id } => {
                return Ok(empty_outcome(
                    IndexReplayRunStatus::AlreadyComplete,
                    Some(job_id),
                    None,
                ));
            }
            IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
        };

        let checkpoint_store =
            PostgresIndexReplayCheckpointStore::new(self.db.clone(), lease.clone());
        let worker = IndexReplayWorker::new(
            self.sources.clone(),
            self.schema_registry.clone(),
            PostgresMutationStore::new(self.db.clone()),
            checkpoint_store,
        );

        let mut aggregate = IndexReplayRunOutcome {
            status: IndexReplayRunStatus::Yielded,
            job_id: Some(lease.job_id()),
            attempt_count: Some(lease.attempt_count()),
            pages_processed: 0,
            heartbeat_count: 0,
            mutation_count: 0,
            applied_count: 0,
            duplicate_count: 0,
            stale_count: 0,
        };

        for page_index in 0..request.max_pages() {
            if cancel_if_requested(&self.db, &lease).await? {
                aggregate.status = IndexReplayRunStatus::Cancelled;
                return Ok(aggregate);
            }

            if page_index > 0 && page_index % request.heartbeat_every_pages() == 0 {
                heartbeat(&job_store, &lease, request.lease_duration()).await?;
                aggregate.heartbeat_count += 1;
                if cancel_if_requested(&self.db, &lease).await? {
                    aggregate.status = IndexReplayRunStatus::Cancelled;
                    return Ok(aggregate);
                }
            }

            let page = match worker
                .run_next_page_interruptible(request.page_request().clone(), || {
                    let interrupted = should_interrupt();
                    async move { Ok::<bool, crate::IndexReplayFailure>(interrupted) }
                })
                .await
            {
                Ok(page) => page,
                Err(crate::IndexReplayError::Interrupted) => {
                    return yield_after_host_interruption(&self.db, &lease, aggregate).await;
                }
                Err(error) if replay_error_is_lease_lost(&error) => {
                    return Err(lease_lost(&lease));
                }
                Err(error) => {
                    if cancel_if_requested(&self.db, &lease).await? {
                        aggregate.status = IndexReplayRunStatus::Cancelled;
                        return Ok(aggregate);
                    }
                    let details = replay_failure_details(&error);
                    match finish_failure(&self.db, &lease, details).await? {
                        TerminalWriteOutcome::Written => {
                            return Err(IndexReplayRunError::PageFailed {
                                job_id: lease.job_id(),
                                error: Box::new(error),
                            });
                        }
                        TerminalWriteOutcome::Cancelled => {
                            aggregate.status = IndexReplayRunStatus::Cancelled;
                            return Ok(aggregate);
                        }
                    }
                }
            };

            if page.status() != IndexReplayPageStatus::AlreadyComplete {
                aggregate.pages_processed += 1;
            }
            aggregate.mutation_count += page.mutation_count();
            aggregate.applied_count += page.applied_count();
            aggregate.duplicate_count += page.duplicate_count();
            aggregate.stale_count += page.stale_count();

            if cancel_if_requested(&self.db, &lease).await? {
                aggregate.status = IndexReplayRunStatus::Cancelled;
                return Ok(aggregate);
            }

            if matches!(
                page.status(),
                IndexReplayPageStatus::Complete | IndexReplayPageStatus::AlreadyComplete
            ) {
                match finish_success(&self.db, &lease).await? {
                    TerminalWriteOutcome::Written => {
                        aggregate.status = IndexReplayRunStatus::Complete;
                        return Ok(aggregate);
                    }
                    TerminalWriteOutcome::Cancelled => {
                        aggregate.status = IndexReplayRunStatus::Cancelled;
                        return Ok(aggregate);
                    }
                }
            }
        }

        match yield_for_resume(&self.db, &lease).await? {
            TerminalWriteOutcome::Written => Ok(aggregate),
            TerminalWriteOutcome::Cancelled => {
                aggregate.status = IndexReplayRunStatus::Cancelled;
                Ok(aggregate)
            }
        }
    }
}

async fn yield_after_host_interruption(
    db: &DatabaseConnection,
    lease: &IndexReplayJobLease,
    mut aggregate: IndexReplayRunOutcome,
) -> Result<IndexReplayRunOutcome, IndexReplayRunError> {
    if cancel_if_requested(db, lease).await? {
        aggregate.status = IndexReplayRunStatus::Cancelled;
        return Ok(aggregate);
    }
    match yield_for_resume(db, lease).await? {
        TerminalWriteOutcome::Written => {
            aggregate.status = IndexReplayRunStatus::Yielded;
            Ok(aggregate)
        }
        TerminalWriteOutcome::Cancelled => {
            aggregate.status = IndexReplayRunStatus::Cancelled;
            Ok(aggregate)
        }
    }
}
