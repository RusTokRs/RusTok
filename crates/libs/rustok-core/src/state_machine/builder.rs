/// Builder pattern for state machines
///
/// Provides a fluent API for constructing state machines with validation.
use super::TransitionGuard;
use std::marker::PhantomData;

/// State machine builder
pub struct StateMachineBuilder<M, S> {
    _machine: PhantomData<M>,
    _state: PhantomData<S>,
}

impl<M, S> StateMachineBuilder<M, S> {
    pub fn new() -> Self {
        Self {
            _machine: PhantomData,
            _state: PhantomData,
        }
    }
}

impl<M, S> Default for StateMachineBuilder<M, S> {
    fn default() -> Self {
        Self::new()
    }
}

type TransitionGuardFn<From> = Box<dyn Fn(&From) -> bool>;

/// Transition builder with guards
pub struct TransitionBuilder<From, To> {
    _from: PhantomData<From>,
    _to: PhantomData<To>,
    guards: Vec<TransitionGuardFn<From>>,
}

impl<From, To> TransitionBuilder<From, To> {
    pub fn new() -> Self {
        Self {
            _from: PhantomData,
            _to: PhantomData,
            guards: Vec::new(),
        }
    }

    pub fn guard<G>(mut self, guard: G) -> Self
    where
        G: TransitionGuard<From> + 'static,
    {
        self.guards
            .push(Box::new(move |state| guard.can_transition(state)));
        self
    }

    pub fn can_transition(&self, from: &From) -> bool {
        self.guards.iter().all(|g| g(from))
    }
}

impl<From, To> Default for TransitionBuilder<From, To> {
    fn default() -> Self {
        Self::new()
    }
}
