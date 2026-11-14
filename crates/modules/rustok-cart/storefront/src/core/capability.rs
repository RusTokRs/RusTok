//! A host-supplied Cart availability snapshot is a UI fence, not authorization.

/// Every host update changes the generation, even when the final value is the same.
/// This keeps an earlier request stale across an enabled → disabled → enabled cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CartCapabilityState {
    pub enabled: bool,
    pub generation: u64,
}

impl CartCapabilityState {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            generation: 0,
        }
    }

    pub fn fail_closed(&mut self) {
        self.generation = u64::MAX;
        self.enabled = false;
    }

    pub fn replace(&mut self, enabled: bool) {
        match self.generation.checked_add(1) {
            Some(generation) if generation < u64::MAX => {
                self.generation = generation;
                self.enabled = enabled;
            }
            _ => {
                // Exhaustion must not make an old generation current again.
                self.generation = u64::MAX;
                self.enabled = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CartCapabilityState;

    #[test]
    fn transitions_fence_aba_and_repeated_host_snapshots() {
        let mut state = CartCapabilityState::new(true);
        let original = state;
        state.replace(false);
        assert!(!state.enabled);
        state.replace(true);
        assert!(state.enabled);
        assert_ne!(state, original);
        let previous = state;
        state.replace(true);
        assert_ne!(state, previous);
    }

    #[test]
    fn exhausted_generation_is_permanently_fail_closed() {
        let mut state = CartCapabilityState {
            enabled: true,
            generation: u64::MAX - 1,
        };
        state.replace(true);
        assert!(!state.enabled);
        assert_eq!(state.generation, u64::MAX);
        state.replace(true);
        assert!(!state.enabled);
        let mut missing_listener = CartCapabilityState::new(true);
        missing_listener.fail_closed();
        missing_listener.replace(true);
        assert_eq!(missing_listener.generation, u64::MAX);
        assert!(!missing_listener.enabled);
    }
}
