# rustok-blog implementation plan — slice 96 continuation

Status: source_verified_no_compile / source-ready / maintainer execution pending.
Compile policy: not_run_by_request. Runtime status: not_run.
Tests, Cargo, formatting, and CI were not run by the implementation agent; no runtime result is claimed.

## Retained source contract

- Status: `restart_ambiguity_postgres_evidence_source_ready_maintainer_execution_pending`.
- Active claim acknowledgement
- Expired claim restart recovery
- Publication acknowledgement
- Requeue acknowledgement
- does not inject a database-driver failure into production code
- No relay task is started
- --ignored --nocapture --test-threads=1
- were not executed by the implementation agent
- Retain canonical relay restart, delivery acknowledgement, retry, and DLQ evidence
- `rustok-outbox`-owned slice
- self.mark_failed_attempt(model, err).await
