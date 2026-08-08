use std::{future::Future, time::Duration};

use tokio::time::timeout;

use crate::IndexReplayFailure;

/// Canonical upper bound for one replay mutation persistence call or one replay checkpoint commit.
///
/// This is an outer future bound, not a claim that the underlying database operation is rolled back
/// or cancelled after the future is dropped. Replay correctness must therefore continue to rely on
/// stable delivery identity, monotonic source versions, durable checkpoint reads, and the job lease
/// fence after any timeout.
const DEFAULT_INDEX_REPLAY_STORAGE_FUTURE_TIMEOUT: Duration = Duration::from_secs(30);

const INDEX_REPLAY_MUTATION_TIMEOUT_CODE: &str = "index_replay_mutation_timeout";
const INDEX_REPLAY_CHECKPOINT_COMMIT_TIMEOUT_CODE: &str =
    "index_replay_checkpoint_commit_timeout";

pub(super) async fn bounded_replay_mutation<T, F>(future: F) -> Result<T, IndexReplayFailure>
where
    F: Future<Output = Result<T, IndexReplayFailure>>,
{
    bounded_replay_storage_future(
        DEFAULT_INDEX_REPLAY_STORAGE_FUTURE_TIMEOUT,
        INDEX_REPLAY_MUTATION_TIMEOUT_CODE,
        "mutation",
        future,
    )
    .await
}

pub(super) async fn bounded_replay_checkpoint_commit<T, F>(
    future: F,
) -> Result<T, IndexReplayFailure>
where
    F: Future<Output = Result<T, IndexReplayFailure>>,
{
    bounded_replay_storage_future(
        DEFAULT_INDEX_REPLAY_STORAGE_FUTURE_TIMEOUT,
        INDEX_REPLAY_CHECKPOINT_COMMIT_TIMEOUT_CODE,
        "checkpoint_commit",
        future,
    )
    .await
}

async fn bounded_replay_storage_future<T, F>(
    call_timeout: Duration,
    timeout_code: &'static str,
    phase: &'static str,
    future: F,
) -> Result<T, IndexReplayFailure>
where
    F: Future<Output = Result<T, IndexReplayFailure>>,
{
    debug_assert!(!call_timeout.is_zero());
    match timeout(call_timeout, future).await {
        Ok(result) => result,
        Err(_) => {
            let timeout_ms = u64::try_from(call_timeout.as_millis()).unwrap_or(u64::MAX);
            tracing::error!(
                replay_phase = phase,
                replay_failure_code = timeout_code,
                replay_failure_retryable = true,
                timeout_ms,
                "Index replay storage future exceeded its outer timeout"
            );
            Err(IndexReplayFailure::retryable_static(timeout_code))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use super::*;
    use crate::IndexReplayFailureKind;

    #[tokio::test]
    async fn pending_mutation_future_times_out_as_retryable() {
        let failure = bounded_replay_storage_future(
            Duration::from_millis(1),
            INDEX_REPLAY_MUTATION_TIMEOUT_CODE,
            "mutation",
            pending::<Result<(), IndexReplayFailure>>(),
        )
        .await
        .expect_err("pending replay mutation should hit the outer timeout");

        assert_eq!(failure.kind(), IndexReplayFailureKind::Retryable);
        assert_eq!(failure.code(), INDEX_REPLAY_MUTATION_TIMEOUT_CODE);
    }

    #[tokio::test]
    async fn pending_checkpoint_commit_future_times_out_as_retryable() {
        let failure = bounded_replay_storage_future(
            Duration::from_millis(1),
            INDEX_REPLAY_CHECKPOINT_COMMIT_TIMEOUT_CODE,
            "checkpoint_commit",
            pending::<Result<(), IndexReplayFailure>>(),
        )
        .await
        .expect_err("pending replay checkpoint commit should hit the outer timeout");

        assert_eq!(failure.kind(), IndexReplayFailureKind::Retryable);
        assert_eq!(
            failure.code(),
            INDEX_REPLAY_CHECKPOINT_COMMIT_TIMEOUT_CODE
        );
    }

    #[tokio::test]
    async fn dependency_failure_is_preserved_before_timeout() {
        let expected = IndexReplayFailure::permanent("mutation_rejected")
            .expect("fixture failure code should be valid");
        let actual = bounded_replay_storage_future(
            Duration::from_millis(1),
            INDEX_REPLAY_MUTATION_TIMEOUT_CODE,
            "mutation",
            async { Err::<(), _>(expected.clone()) },
        )
        .await
        .expect_err("dependency failure should pass through unchanged");

        assert_eq!(actual, expected);
    }
}
