//! Non-secret client auth lifecycle stamp. It is not an authentication credential.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientAuthScope {
    pub generation: u64,
    pending: bool,
    verified: bool,
}

impl ClientAuthScope {
    pub fn new(verified: bool) -> Self {
        Self {
            generation: 0,
            pending: false,
            verified,
        }
    }

    pub fn is_ready(self) -> bool {
        self.verified && !self.pending && self.generation < u64::MAX
    }

    /// An auth provider must not issue overlapping cookie-changing operations: response order
    /// otherwise makes the in-memory session and HttpOnly cookie disagree.
    pub(crate) fn begin(&mut self) -> bool {
        if self.pending {
            return false;
        }
        if self.generation >= u64::MAX - 1 {
            self.invalidate_permanently();
            return false;
        }
        self.advance();
        self.pending = true;
        true
    }

    pub(crate) fn finish(&mut self, verified: bool) {
        if !self.pending {
            self.invalidate_permanently();
            return;
        }
        self.advance();
        self.pending = false;
        self.verified = verified && self.generation < u64::MAX;
    }

    pub(crate) fn set_verified(&mut self, verified: bool) {
        self.advance();
        self.verified = verified && self.generation < u64::MAX;
    }

    /// A dropped browser future does not prove that its request or Set-Cookie response stopped.
    /// Never start another transition in this provider after that uncertainty; a new document
    /// must restore the cookie-backed session instead.
    pub(crate) fn abandon(&mut self) {
        self.invalidate_permanently();
    }

    fn advance(&mut self) {
        if let Some(generation) = self.generation.checked_add(1) {
            self.generation = generation;
        } else {
            self.invalidate_permanently();
        }
    }

    fn invalidate_permanently(&mut self) {
        self.generation = u64::MAX;
        self.verified = false;
    }
}

#[cfg(test)]
mod tests {
    use super::ClientAuthScope;

    #[test]
    fn transitions_are_exclusive_and_aba_cannot_reuse_a_scope() {
        let mut scope = ClientAuthScope::new(true);
        let original = scope;
        assert!(scope.begin());
        assert!(!scope.is_ready());
        assert!(!scope.begin());
        scope.finish(true);
        assert!(scope.is_ready());
        assert_ne!(scope, original);
        assert!(scope.begin());
        scope.finish(true);
        assert_ne!(scope, original);
    }

    #[test]
    fn completed_failed_transition_can_be_reverified() {
        let mut scope = ClientAuthScope::new(true);
        assert!(scope.begin());
        scope.finish(false);
        assert!(!scope.is_ready());
        assert!(scope.begin());
        scope.finish(true);
        assert!(scope.is_ready());
    }

    #[test]
    fn abandoning_a_request_permanently_prevents_cookie_transition_overlap() {
        let mut scope = ClientAuthScope::new(true);
        assert!(scope.begin());
        scope.abandon();
        assert!(!scope.is_ready());
        assert!(!scope.begin());
        scope.finish(true);
        scope.set_verified(true);
        assert!(!scope.is_ready());
        assert!(!scope.begin());
    }

    #[test]
    fn generation_exhaustion_and_unbalanced_finish_are_permanently_fail_closed() {
        let mut scope = ClientAuthScope::new(true);
        scope.generation = u64::MAX - 1;
        assert!(!scope.begin());
        assert!(!scope.is_ready());
        assert!(!scope.begin());
        scope.set_verified(true);
        assert!(!scope.is_ready());
        let mut unbalanced = ClientAuthScope::new(true);
        unbalanced.finish(true);
        assert!(!unbalanced.is_ready());
        assert!(!unbalanced.begin());
    }
}
