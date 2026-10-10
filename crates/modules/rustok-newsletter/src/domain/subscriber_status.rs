use rustok_newsletter_api::SubscriberStatus;

/// Subscriber lifecycle state machine.
///
/// Valid transitions:
/// - Pending → Active (confirmed)
/// - Pending → Unsubscribed (explicit cancel before confirm)
/// - Active → Unsubscribed (user unsubscribes)
/// - Active → Suppressed (bounce/complaint)
/// - Unsubscribed → Active (re-subscribe with new confirmation)
/// - Suppressed → Active (manual admin re-activation)
pub struct SubscriberLifecycle;

/// Represents a valid subscriber state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriberTransition {
    Confirm,
    Unsubscribe,
    Suppress,
    Reactivate,
}

impl SubscriberLifecycle {
    /// Validate and return the transition from current to target status.
    pub fn transition(
        current: SubscriberStatus,
        target: SubscriberStatus,
    ) -> Option<SubscriberTransition> {
        match (current, target) {
            (SubscriberStatus::Pending, SubscriberStatus::Active) => {
                Some(SubscriberTransition::Confirm)
            }
            (SubscriberStatus::Pending, SubscriberStatus::Unsubscribed) => {
                Some(SubscriberTransition::Unsubscribe)
            }
            (SubscriberStatus::Active, SubscriberStatus::Unsubscribed) => {
                Some(SubscriberTransition::Unsubscribe)
            }
            (SubscriberStatus::Active, SubscriberStatus::Suppressed) => {
                Some(SubscriberTransition::Suppress)
            }
            (SubscriberStatus::Unsubscribed, SubscriberStatus::Active) => {
                Some(SubscriberTransition::Reactivate)
            }
            (SubscriberStatus::Suppressed, SubscriberStatus::Active) => {
                Some(SubscriberTransition::Reactivate)
            }
            _ => None,
        }
    }

    /// Check if a transition is valid without returning the transition enum.
    pub fn can_transition(current: SubscriberStatus, target: SubscriberStatus) -> bool {
        Self::transition(current, target).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_can_confirm_to_active() {
        assert!(SubscriberLifecycle::can_transition(
            SubscriberStatus::Pending,
            SubscriberStatus::Active,
        ));
    }

    #[test]
    fn pending_cannot_skip_to_suppressed() {
        assert!(!SubscriberLifecycle::can_transition(
            SubscriberStatus::Pending,
            SubscriberStatus::Suppressed,
        ));
    }

    #[test]
    fn active_can_unsubscribe() {
        assert!(SubscriberLifecycle::can_transition(
            SubscriberStatus::Active,
            SubscriberStatus::Unsubscribed,
        ));
    }

    #[test]
    fn suppressed_can_reactivate() {
        assert!(SubscriberLifecycle::can_transition(
            SubscriberStatus::Suppressed,
            SubscriberStatus::Active,
        ));
    }

    #[test]
    fn same_state_is_not_a_transition() {
        assert!(!SubscriberLifecycle::can_transition(
            SubscriberStatus::Active,
            SubscriberStatus::Active,
        ));
    }
}
