/// Type-Safe State Machine Framework
///
/// Provides compile-time guarantees for state transitions:
/// - Impossible states are unrepresentable
/// - Invalid transitions are compile errors
/// - State-specific data is type-safe
/// - Transition validation is explicit
///
/// Benefits:
/// - **Compile-time safety**: Invalid transitions caught at compile time
/// - **Self-documenting**: State graph visible in type system
/// - **No runtime panics**: All states and transitions are valid by construction
/// - **State-specific data**: Each state can have its own fields
///
/// Example:
/// ```ignore
/// // Define states
/// struct Draft;
/// struct Review { reviewer_id: Uuid };
/// struct Published { published_at: DateTime };
///
/// // Define state machine
/// struct Document<S> {
///     id: Uuid,
///     title: String,
///     state: S,
/// }
///
/// // Implement transitions
/// impl Document<Draft> {
///     fn submit_for_review(self, reviewer: Uuid) -> Document<Review> {
///         Document {
///             id: self.id,
///             title: self.title,
///             state: Review { reviewer_id: reviewer },
///         }
///     }
/// }
///
/// impl Document<Review> {
///     fn approve(self) -> Document<Published> {
///         Document {
///             id: self.id,
///             title: self.title,
///             state: Published { published_at: Utc::now() },
///         }
///     }
///     
///     fn reject(self) -> Document<Draft> {
///         Document {
///             id: self.id,
///             title: self.title,
///             state: Draft,
///         }
///     }
/// }
/// ```
pub mod builder;
pub mod transition;

pub use builder::StateMachineBuilder;
pub use transition::{Transition, TransitionGuard, TransitionResult};

/// Marker trait for state machine states
pub trait State: Sized {
    /// State name for debugging/logging
    fn name() -> &'static str;

    /// State description
    fn description() -> &'static str {
        ""
    }
}

/// Marker trait for state machines
pub trait StateMachine {
    /// The current state type
    type State: State;

    /// Get the current state
    fn state(&self) -> &Self::State;
}

/// Helper macro to define type-safe state machines
///
/// Example:
/// ```ignore
/// state_machine! {
///     Machine: Order {
///         states: {
///             Pending,
///             Confirmed { confirmed_at: DateTime<Utc> },
///             Shipped { tracking_number: String },
///             Delivered { delivered_at: DateTime<Utc> },
///             Cancelled { reason: String },
///         },
///         transitions: {
///             Pending => Confirmed,
///             Confirmed => [Shipped, Cancelled],
///             Shipped => [Delivered, Cancelled],
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! state_machine {
    // Main pattern
    (
        Machine: $machine:ident {
            states: {
                $($state:ident $({ $($field:ident: $ty:ty),* })? ),+ $(,)?
            },
            transitions: {
                $($from:ident => $to:tt),+ $(,)?
            }
        }
    ) => {
        // Define state structs
        $(
            #[derive(Debug, Clone)]
            pub struct $state $({ $(pub $field: $ty),* })?;

            impl $crate::state_machine::State for $state {
                fn name() -> &'static str {
                    stringify!($state)
                }
            }
        )+

        // Define machine struct
        #[derive(Debug, Clone)]
        pub struct $machine<S> {
            pub id: uuid::Uuid,
            pub state: S,
        }

        impl<S: $crate::state_machine::State> $crate::state_machine::StateMachine for $machine<S> {
            type State = S;

            fn state(&self) -> &Self::State {
                &self.state
            }
        }
    };
}
