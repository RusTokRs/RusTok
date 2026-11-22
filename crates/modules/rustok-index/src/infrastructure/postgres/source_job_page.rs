use std::{future::Future, time::Duration};

use tokio::time::{Instant, sleep_until};

const PAGE_DEADLINE: Duration = Duration::from_secs(90);

pub(super) fn heartbeat_interval(lease_duration: Duration) -> Duration {
    lease_duration / 3
}

/// Drive both the page and a pending heartbeat. A mutation can hold the job row
/// while it commits, so waiting for the heartbeat alone could deadlock the page.
/// Dropping the page at its deadline is safe only with the durable write fences.
pub(super) async fn drive_page<T, E, Page, Heartbeat, Refresh>(
    lease_duration: Duration,
    page: Page,
    mut refresh: Refresh,
) -> Result<(Option<T>, usize), E>
where
    Page: Future<Output = T>,
    Heartbeat: Future<Output = Result<(), E>>,
    Refresh: FnMut() -> Heartbeat,
{
    let interval = heartbeat_interval(lease_duration);
    let deadline = Instant::now() + PAGE_DEADLINE;
    let mut next_heartbeat = Instant::now() + interval;
    let mut heartbeats = 0;
    tokio::pin!(page);
    loop {
        tokio::select! {
            biased;
            _ = sleep_until(deadline) => return Ok((None, heartbeats)),
            result = &mut page => return Ok((Some(result), heartbeats)),
            _ = sleep_until(next_heartbeat) => {
                let heartbeat = refresh();
                tokio::pin!(heartbeat);
                tokio::select! {
                    biased;
                    _ = sleep_until(deadline) => return Ok((None, heartbeats)),
                    result = &mut page => return Ok((Some(result), heartbeats)),
                    result = &mut heartbeat => {
                        result?;
                        heartbeats += 1;
                        next_heartbeat = Instant::now() + interval;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::{pending, ready};

    use tokio::sync::Notify;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn page_progress_does_not_depend_on_a_heartbeat_waiting_for_its_row() {
        let heartbeat_started = Notify::new();
        let result = drive_page(
            Duration::from_secs(60),
            async {
                heartbeat_started.notified().await;
                "page released the job row"
            },
            || async {
                heartbeat_started.notify_one();
                pending::<Result<(), ()>>().await
            },
        )
        .await
        .unwrap();
        assert_eq!(result, (Some("page released the job row"), 0));
    }

    #[tokio::test(start_paused = true)]
    async fn lease_renewals_do_not_extend_the_whole_page_deadline() {
        let (result, heartbeats) = drive_page(Duration::from_secs(60), pending::<()>(), || {
            ready(Ok::<_, ()>(()))
        })
        .await
        .unwrap();
        assert_eq!(result, None);
        assert_eq!(heartbeats, 4);
    }
}
