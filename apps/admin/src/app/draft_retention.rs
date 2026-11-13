/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Retention of SSR draft sessions in the standalone admin host.
//!
//! `SsrDraftSessionStore::prune` is the only thing that enforces the store's TTL and its global
//! session cap, and nothing called it: the editor only touches a session when a browser asks for
//! one, so an abandoned draft — and every record of a closed tab — stayed on disk (or in memory)
//! until the process restarted, and the cap never applied. This module is the host caller.
//!
//! The loop is deliberately boring: it owns no state, reads the store the intent endpoint
//! resolves (so a host that installed a custom or distributed store is pruned through that same
//! store), and treats a prune failure as one bad pass rather than a reason to stop. Draft storage
//! that cannot be pruned is a retention problem to log, not a reason to take the editor down.

use std::time::Duration;

use rustok_pages_admin::{SsrDraftSessionStore, pages_browser_draft_store};

/// How often an idle admin host collects expired draft sessions.
///
/// The store's own TTL is the authority on when a session expires; the interval only bounds how
/// long an expired record survives, so it is short enough to matter on a small deployment and
/// long enough that a prune of a mostly empty directory costs nothing.
pub const DRAFT_SESSION_PRUNE_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Prunes the resolved draft store, logging what it did.
///
/// Returns the number of records the store deleted. Kept separate from the loop so a host can run
/// one pass (an operator `--once` path, a test) without starting a task.
pub fn prune_draft_sessions_once(store: &dyn SsrDraftSessionStore) -> usize {
    match store.prune() {
        Ok(0) => 0,
        Ok(removed) => {
            tracing::info!(removed, "pruned expired Page Builder draft sessions");
            removed
        }
        Err(error) => {
            tracing::warn!(%error, "Page Builder draft session prune failed; retrying next pass");
            0
        }
    }
}

/// Starts the retention loop on the current runtime.
///
/// The loop holds a borrowed `'static` store, which is what the intent endpoint reads: a custom
/// store registered through `set_pages_browser_draft_store` before boot is therefore the one that
/// is pruned, instead of a second instance nobody reads.
pub fn spawn_draft_session_prune(interval: Duration) {
    tokio::spawn(async move {
        loop {
            prune_draft_sessions_once(pages_browser_draft_store());
            tokio::time::sleep(interval).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rustok_page_builder_admin::{
        AdminCanvasController, SsrDraftSessionError, SsrDraftSessionSnapshot,
    };
    use rustok_pages_admin::DraftSessionOwner;

    /// A store that reports whatever a prune run should report, and counts the calls: the loop's
    /// contract is that it reaches the store at all, and that a failure is not fatal.
    struct CountingStore {
        prunes: AtomicUsize,
        removed: Result<usize, ()>,
    }

    impl SsrDraftSessionStore for CountingStore {
        fn load(
            &self,
            _token: &str,
            _page_id: &str,
            _owner: Option<&DraftSessionOwner>,
        ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError> {
            Ok(None)
        }

        fn commit(
            &self,
            _token: Option<&str>,
            _expected_generation: Option<u64>,
            _controller: AdminCanvasController,
            _owner: Option<&DraftSessionOwner>,
        ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
            Err(SsrDraftSessionError::Poisoned)
        }

        fn remove(
            &self,
            _token: &str,
            _owner: Option<&DraftSessionOwner>,
        ) -> Result<(), SsrDraftSessionError> {
            Ok(())
        }

        fn prune(&self) -> Result<usize, SsrDraftSessionError> {
            self.prunes.fetch_add(1, Ordering::SeqCst);
            match self.removed {
                Ok(removed) => Ok(removed),
                Err(()) => Err(SsrDraftSessionError::Io("draft directory is gone".to_string())),
            }
        }
    }

    fn store(removed: Result<usize, ()>) -> Arc<CountingStore> {
        Arc::new(CountingStore {
            prunes: AtomicUsize::new(0),
            removed,
        })
    }

    #[test]
    fn one_pass_reports_what_the_store_deleted() {
        let store = store(Ok(3));
        assert_eq!(prune_draft_sessions_once(store.as_ref()), 3);
        assert_eq!(store.prunes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_failing_prune_is_reported_as_zero_and_keeps_the_host_alive() {
        let store = store(Err(()));
        assert_eq!(prune_draft_sessions_once(store.as_ref()), 0);
        assert_eq!(store.prunes.load(Ordering::SeqCst), 1);
    }
}
