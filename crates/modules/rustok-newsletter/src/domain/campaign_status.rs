use rustok_newsletter_api::CampaignStatus;

/// Campaign lifecycle state machine.
///
/// Valid transitions:
/// - Draft → Scheduled (admin schedules for delivery)
/// - Draft → Cancelled (admin discards draft)
/// - Scheduled → Sending (scheduler picks up campaign)
/// - Scheduled → Cancelled (admin cancels scheduled campaign)
/// - Sending → Sent (delivery completes)
/// - Sending → Cancelled (admin aborts in-flight campaign)
pub struct CampaignLifecycle;

/// Represents a valid campaign state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignTransition {
    Schedule,
    StartSending,
    CompleteSending,
    Cancel,
}

impl CampaignLifecycle {
    /// Validate and return the transition from current to target status.
    pub fn transition(
        current: CampaignStatus,
        target: CampaignStatus,
    ) -> Option<CampaignTransition> {
        match (current, target) {
            (CampaignStatus::Draft, CampaignStatus::Scheduled) => {
                Some(CampaignTransition::Schedule)
            }
            (CampaignStatus::Draft, CampaignStatus::Cancelled) => Some(CampaignTransition::Cancel),
            (CampaignStatus::Scheduled, CampaignStatus::Sending) => {
                Some(CampaignTransition::StartSending)
            }
            (CampaignStatus::Scheduled, CampaignStatus::Cancelled) => {
                Some(CampaignTransition::Cancel)
            }
            (CampaignStatus::Sending, CampaignStatus::Sent) => {
                Some(CampaignTransition::CompleteSending)
            }
            (CampaignStatus::Sending, CampaignStatus::Cancelled) => {
                Some(CampaignTransition::Cancel)
            }
            _ => None,
        }
    }

    /// Check if a transition is valid.
    pub fn can_transition(current: CampaignStatus, target: CampaignStatus) -> bool {
        Self::transition(current, target).is_some()
    }

    /// Check if the campaign is in a terminal state.
    pub fn is_terminal(status: CampaignStatus) -> bool {
        matches!(status, CampaignStatus::Sent | CampaignStatus::Cancelled)
    }

    /// Check if the campaign can be edited.
    pub fn is_editable(status: CampaignStatus) -> bool {
        matches!(status, CampaignStatus::Draft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_can_schedule() {
        assert!(CampaignLifecycle::can_transition(
            CampaignStatus::Draft,
            CampaignStatus::Scheduled,
        ));
    }

    #[test]
    fn draft_can_cancel() {
        assert!(CampaignLifecycle::can_transition(
            CampaignStatus::Draft,
            CampaignStatus::Cancelled,
        ));
    }

    #[test]
    fn scheduled_can_start_sending() {
        assert!(CampaignLifecycle::can_transition(
            CampaignStatus::Scheduled,
            CampaignStatus::Sending,
        ));
    }

    #[test]
    fn scheduled_can_cancel() {
        assert!(CampaignLifecycle::can_transition(
            CampaignStatus::Scheduled,
            CampaignStatus::Cancelled,
        ));
    }

    #[test]
    fn sending_can_complete() {
        assert!(CampaignLifecycle::can_transition(
            CampaignStatus::Sending,
            CampaignStatus::Sent,
        ));
    }

    #[test]
    fn sent_is_terminal() {
        assert!(CampaignLifecycle::is_terminal(CampaignStatus::Sent));
        assert!(!CampaignLifecycle::can_transition(
            CampaignStatus::Sent,
            CampaignStatus::Draft,
        ));
    }

    #[test]
    fn cancelled_is_terminal() {
        assert!(CampaignLifecycle::is_terminal(CampaignStatus::Cancelled));
    }

    #[test]
    fn only_draft_is_editable() {
        assert!(CampaignLifecycle::is_editable(CampaignStatus::Draft));
        assert!(!CampaignLifecycle::is_editable(CampaignStatus::Scheduled));
        assert!(!CampaignLifecycle::is_editable(CampaignStatus::Sending));
    }
}
