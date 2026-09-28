//! Multi-step wizard and funnel state tracking.

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};

/// Tracks progress through a multi-step form or onboarding wizard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormStepTracker {
    /// Zero-based current active step index.
    pub current_step: usize,
    /// Total number of steps in the flow.
    pub total_steps: usize,
    /// Set of step indices that have been validated and marked completed.
    pub completed_steps: BTreeSet<usize>,
}

impl Default for FormStepTracker {
    fn default() -> Self {
        Self::new(1)
    }
}

impl FormStepTracker {
    /// Create a new step tracker with the given total count of steps (at least 1).
    pub fn new(total_steps: usize) -> Self {
        Self {
            current_step: 0,
            total_steps: total_steps.max(1),
            completed_steps: BTreeSet::new(),
        }
    }

    /// Whether navigation to the next step is possible.
    pub fn can_go_next(&self) -> bool {
        self.current_step + 1 < self.total_steps
    }

    /// Whether navigation to the previous step is possible.
    pub fn can_go_back(&self) -> bool {
        self.current_step > 0
    }

    /// Advance to the next step if not already on the last step.
    ///
    /// Automatically marks the current step as completed.
    pub fn next_step(&mut self) -> bool {
        if self.can_go_next() {
            self.completed_steps.insert(self.current_step);
            self.current_step += 1;
            true
        } else {
            false
        }
    }

    /// Return to the previous step if not already on the first step.
    pub fn prev_step(&mut self) -> bool {
        if self.can_go_back() {
            self.current_step -= 1;
            true
        } else {
            false
        }
    }

    /// Jump directly to a specific step index.
    pub fn go_to_step(&mut self, step: usize) -> bool {
        if step < self.total_steps {
            self.current_step = step;
            true
        } else {
            false
        }
    }

    /// Mark the current step as completed.
    pub fn mark_current_completed(&mut self) {
        self.completed_steps.insert(self.current_step);
    }

    /// Mark a specific step index as completed.
    pub fn mark_completed(&mut self, step: usize) {
        if step < self.total_steps {
            self.completed_steps.insert(step);
        }
    }

    /// Check whether a specific step index is completed.
    pub fn is_completed(&self, step: usize) -> bool {
        self.completed_steps.contains(&step)
    }

    /// Whether all steps have been marked completed.
    pub fn are_all_completed(&self) -> bool {
        self.completed_steps.len() >= self.total_steps
    }

    /// Whether the tracker is currently on the first step.
    pub fn is_first_step(&self) -> bool {
        self.current_step == 0
    }

    /// Whether the tracker is currently on the last step.
    pub fn is_last_step(&self) -> bool {
        self.current_step + 1 == self.total_steps
    }

    /// Calculate completion progress as a percentage between `0.0` and `100.0`.
    pub fn progress_percent(&self) -> f64 {
        if self.total_steps <= 1 {
            if self.is_completed(0) { 100.0 } else { 0.0 }
        } else {
            (self.completed_steps.len() as f64 / self.total_steps as f64) * 100.0
        }
    }

    /// Reset to the first step and clear completed steps.
    pub fn reset(&mut self) {
        self.current_step = 0;
        self.completed_steps.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_tracker_navigation() {
        let mut tracker = FormStepTracker::new(3);
        assert!(tracker.is_first_step());
        assert!(!tracker.is_last_step());
        assert!(tracker.can_go_next());
        assert!(!tracker.can_go_back());

        assert!(tracker.next_step());
        assert_eq!(tracker.current_step, 1);
        assert!(tracker.is_completed(0));
        assert!(tracker.can_go_back());

        assert!(tracker.next_step());
        assert_eq!(tracker.current_step, 2);
        assert!(tracker.is_last_step());
        assert!(!tracker.can_go_next());

        assert!(tracker.prev_step());
        assert_eq!(tracker.current_step, 1);

        tracker.reset();
        assert_eq!(tracker.current_step, 0);
        assert_eq!(tracker.completed_steps.len(), 0);
    }

    #[test]
    fn step_tracker_progress() {
        let mut tracker = FormStepTracker::new(4);
        assert_eq!(tracker.progress_percent(), 0.0);

        tracker.mark_completed(0);
        assert_eq!(tracker.progress_percent(), 25.0);

        tracker.mark_completed(1);
        tracker.mark_completed(2);
        tracker.mark_completed(3);
        assert_eq!(tracker.progress_percent(), 100.0);
        assert!(tracker.are_all_completed());
    }
}
