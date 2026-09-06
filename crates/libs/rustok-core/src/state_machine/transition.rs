/// Transition types and validation
///
/// Provides composable transition guards and validation logic.
use std::fmt;

/// Result of a state transition
pub type TransitionResult<T, E = TransitionError> = Result<T, E>;

/// Transition error
#[derive(Debug, Clone, thiserror::Error)]
pub enum TransitionError {
    #[error("Invalid transition from {from} to {to}: {reason}")]
    Invalid {
        from: String,
        to: String,
        reason: String,
    },

    #[error("Transition guard failed: {0}")]
    GuardFailed(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Business rule violation: {0}")]
    BusinessRule(String),
}

/// Trait for state transitions
pub trait Transition<From, To> {
    /// Type of data needed for the transition
    type Input;

    /// Type of error that can occur
    type Error: fmt::Display;

    /// Execute the transition
    fn execute(from: From, input: Self::Input) -> Result<To, Self::Error>;
}

/// Trait for transition guards
///
/// Guards are predicates that must be satisfied before a transition can occur.
pub trait TransitionGuard<S> {
    /// Check if the transition is allowed
    fn can_transition(&self, state: &S) -> bool;

    /// Error message if guard fails
    fn error_message(&self) -> String {
        "Transition guard failed".to_string()
    }
}

/// Composable guard: AND
pub struct AndGuard<A, B> {
    pub a: A,
    pub b: B,
}

impl<S, A, B> TransitionGuard<S> for AndGuard<A, B>
where
    A: TransitionGuard<S>,
    B: TransitionGuard<S>,
{
    fn can_transition(&self, state: &S) -> bool {
        self.a.can_transition(state) && self.b.can_transition(state)
    }

    fn error_message(&self) -> String {
        format!("{} AND {}", self.a.error_message(), self.b.error_message())
    }
}

/// Composable guard: OR
pub struct OrGuard<A, B> {
    pub a: A,
    pub b: B,
}

impl<S, A, B> TransitionGuard<S> for OrGuard<A, B>
where
    A: TransitionGuard<S>,
    B: TransitionGuard<S>,
{
    fn can_transition(&self, state: &S) -> bool {
        self.a.can_transition(state) || self.b.can_transition(state)
    }

    fn error_message(&self) -> String {
        format!("{} OR {}", self.a.error_message(), self.b.error_message())
    }
}

/// Composable guard: NOT
pub struct NotGuard<A> {
    pub inner: A,
}

impl<S, A> TransitionGuard<S> for NotGuard<A>
where
    A: TransitionGuard<S>,
{
    fn can_transition(&self, state: &S) -> bool {
        !self.inner.can_transition(state)
    }

    fn error_message(&self) -> String {
        format!("NOT ({})", self.inner.error_message())
    }
}

/// Helper function to combine guards with AND
pub fn and<S, A, B>(a: A, b: B) -> AndGuard<A, B>
where
    A: TransitionGuard<S>,
    B: TransitionGuard<S>,
{
    AndGuard { a, b }
}

/// Helper function to combine guards with OR
pub fn or<S, A, B>(a: A, b: B) -> OrGuard<A, B>
where
    A: TransitionGuard<S>,
    B: TransitionGuard<S>,
{
    OrGuard { a, b }
}

/// Helper function to negate a guard
pub fn not<S, A>(inner: A) -> NotGuard<A>
where
    A: TransitionGuard<S>,
{
    NotGuard { inner }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysTrue;
    impl<S> TransitionGuard<S> for AlwaysTrue {
        fn can_transition(&self, _: &S) -> bool {
            true
        }
    }

    struct AlwaysFalse;
    impl<S> TransitionGuard<S> for AlwaysFalse {
        fn can_transition(&self, _: &S) -> bool {
            false
        }
    }

    #[test]
    fn test_and_guard() {
        let guard = and::<(), _, _>(AlwaysTrue, AlwaysTrue);
        assert!(guard.can_transition(&()));

        let guard = and::<(), _, _>(AlwaysTrue, AlwaysFalse);
        assert!(!guard.can_transition(&()));
    }

    #[test]
    fn test_or_guard() {
        let guard = or::<(), _, _>(AlwaysTrue, AlwaysFalse);
        assert!(guard.can_transition(&()));

        let guard = or::<(), _, _>(AlwaysFalse, AlwaysFalse);
        assert!(!guard.can_transition(&()));
    }

    #[test]
    fn test_not_guard() {
        let guard = not::<(), _>(AlwaysTrue);
        assert!(!guard.can_transition(&()));

        let guard = not::<(), _>(AlwaysFalse);
        assert!(guard.can_transition(&()));
    }
}
