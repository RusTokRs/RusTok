//! Same-owner operation fence; a local lock is not a server or cross-tab idempotency key.

/// A dropped request can still reach the server. Keep the owner unavailable in that case
/// instead of letting another operation reuse its busy flag. IDs never wrap or repeat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CartOperationState {
    last_id: u64,
    active: Option<u64>,
    poisoned: bool,
}

impl CartOperationState {
    pub(crate) fn is_busy(self) -> bool {
        self.poisoned || self.active.is_some()
    }

    pub(crate) fn begin(&mut self) -> Option<u64> {
        if self.is_busy() {
            return None;
        }
        let Some(id) = self.last_id.checked_add(1).filter(|id| *id < u64::MAX) else {
            self.poisoned = true;
            return None;
        };
        self.last_id = id;
        self.active = Some(id);
        Some(id)
    }

    /// A stale completion cannot clear a newer operation's lock.
    pub(crate) fn finish(&mut self, id: u64) -> bool {
        if self.poisoned || self.active != Some(id) {
            return false;
        }
        self.active = None;
        true
    }

    /// Abandonment only affects the operation that still owns the lock.
    pub(crate) fn abandon(&mut self, id: u64) {
        if self.active == Some(id) {
            self.poisoned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CartOperationState;

    #[test]
    fn overlapping_and_stale_completions_cannot_unlock_the_current_operation() {
        let mut state = CartOperationState::default();
        let first = state.begin().unwrap();
        assert!(state.is_busy());
        assert_eq!(state.begin(), None);
        assert!(state.finish(first));
        let second = state.begin().unwrap();
        assert_ne!(first, second);
        assert!(!state.finish(first));
        state.abandon(first);
        assert!(state.is_busy());
        assert_eq!(state.begin(), None);
        assert!(state.finish(second));
        assert!(!state.is_busy());
    }

    #[test]
    fn abandoned_requests_and_exhaustion_fail_closed() {
        let mut state = CartOperationState::default();
        let operation = state.begin().unwrap();
        state.abandon(operation);
        assert!(state.is_busy());
        assert!(!state.finish(operation));
        assert_eq!(state.begin(), None);

        let mut exhausted = CartOperationState {
            last_id: u64::MAX - 1,
            ..Default::default()
        };
        assert_eq!(exhausted.begin(), None);
        assert!(exhausted.is_busy());
        assert_eq!(exhausted.begin(), None);
    }
}
