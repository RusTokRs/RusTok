//! Multi-step form / wizard step tracker.
//!
//! Provides step progression tracking, step completion verification, and
//! navigation bounds checking for multi-step forms and checkouts.

use serde::{Deserialize, Serialize};

/// Tracks active step index, step labels, and completed step history in multi-step wizards.
///
/// # Example
///
/// ```rust
/// use rustok_forms::step::FormStepTracker;
///
/// let mut tracker = FormStepTracker::new(&["Personal", "Address", "Payment", "Review"]);
/// assert_eq!(tracker.current_step_index(), 0);
/// assert_eq!(tracker.current_step_name(), Some("Personal"));
/// assert!(!tracker.is_last_step());
///
/// tracker.complete_and_next();
/// assert_eq!(tracker.current_step_index(), 1);
/// assert!(tracker.is_step_completed(0));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormStepTracker {
    /// Ordered list of step names or IDs.
    steps: Vec<String>,
    /// Currently active 0-based step index.
    current_step: usize,
    /// Bitset or vector recording which step indices have been successfully completed.
    completed_steps: Vec<bool>,
}

impl FormStepTracker {
    /// Create a new `FormStepTracker` with the given step identifiers.
    ///
    /// # Panics
    ///
    /// Panics if `steps` is empty.
    pub fn new<I, S>(steps: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let steps_vec: Vec<String> = steps.into_iter().map(Into::into).collect();
        assert!(!steps_vec.is_empty(), "FormStepTracker requires at least one step");
        let count = steps_vec.len();
        Self {
            steps: steps_vec,
            current_step: 0,
            completed_steps: vec![false; count],
        }
    }

    /// Total number of steps.
    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    /// Currently active step index (0-based).
    pub fn current_step_index(&self) -> usize {
        self.current_step
    }

    /// Currently active step human-readable name or ID.
    pub fn current_step_name(&self) -> Option<&str> {
        self.steps.get(self.current_step).map(String::as_str)
    }

    /// Step name at a specific 0-based index.
    pub fn step_name_at(&self, index: usize) -> Option<&str> {
        self.steps.get(index).map(String::as_str)
    }

    /// Returns `true` if currently on the first step (index 0).
    pub fn is_first_step(&self) -> bool {
        self.current_step == 0
    }

    /// Returns `true` if currently on the final step.
    pub fn is_last_step(&self) -> bool {
        self.current_step + 1 >= self.steps.len()
    }

    /// Returns `true` if the specified step index has been marked completed.
    pub fn is_step_completed(&self, index: usize) -> bool {
        self.completed_steps.get(index).copied().unwrap_or(false)
    }

    /// Returns `true` if all steps prior to the current one are completed.
    pub fn are_previous_steps_completed(&self) -> bool {
        self.completed_steps[..self.current_step].iter().all(|&c| c)
    }

    /// Returns `true` if all steps in the wizard are marked completed.
    pub fn are_all_steps_completed(&self) -> bool {
        self.completed_steps.iter().all(|&c| c)
    }

    /// Completion progress percentage as a float between 0.0 and 100.0.
    pub fn progress_percentage(&self) -> f64 {
        if self.steps.is_empty() {
            return 100.0;
        }
        let completed = self.completed_steps.iter().filter(|&&c| c).count();
        (completed as f64 / self.steps.len() as f64) * 100.0
    }

    /// Mark the current step as completed.
    pub fn complete_current_step(&mut self) {
        if self.current_step < self.completed_steps.len() {
            self.completed_steps[self.current_step] = true;
        }
    }

    /// Mark a specific step index as completed.
    pub fn set_step_completed(&mut self, index: usize, completed: bool) {
        if index < self.completed_steps.len() {
            self.completed_steps[index] = completed;
        }
    }

    /// Advance to the next step without marking current as completed.
    ///
    /// Returns `true` if advanced, or `false` if already on the last step.
    pub fn next_step(&mut self) -> bool {
        if !self.is_last_step() {
            self.current_step += 1;
            true
        } else {
            false
        }
    }

    /// Mark current step completed and advance to next step.
    ///
    /// Returns `true` if advanced, or `false` if already on the last step.
    pub fn complete_and_next(&mut self) -> bool {
        self.complete_current_step();
        self.next_step()
    }

    /// Navigate back to the previous step.
    ///
    /// Returns `true` if moved back, or `false` if already on the first step.
    pub fn prev_step(&mut self) -> bool {
        if !self.is_first_step() {
            self.current_step -= 1;
            true
        } else {
            false
        }
    }

    /// Go directly to a step by index, if within range.
    ///
    /// Returns `true` if successfully navigated, `false` otherwise.
    pub fn go_to_step(&mut self, index: usize) -> bool {
        if index < self.steps.len() {
            self.current_step = index;
            true
        } else {
            false
        }
    }

    /// Reset tracker to step 0 and clear completed history.
    pub fn reset(&mut self) {
        self.current_step = 0;
        for c in &mut self.completed_steps {
            *c = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_lifecycle() {
        let mut tracker = FormStepTracker::new(vec!["A", "B", "C"]);
        assert_eq!(tracker.total_steps(), 3);
        assert_eq!(tracker.current_step_index(), 0);
        assert_eq!(tracker.current_step_name(), Some("A"));
        assert!(tracker.is_first_step());
        assert!(!tracker.is_last_step());
        assert_eq!(tracker.progress_percentage(), 0.0);

        assert!(tracker.complete_and_next());
        assert_eq!(tracker.current_step_index(), 1);
        assert_eq!(tracker.current_step_name(), Some("B"));
        assert!(tracker.is_step_completed(0));
        assert!(!tracker.is_first_step());
        assert!(!tracker.is_last_step());

        assert!(tracker.complete_and_next());
        assert_eq!(tracker.current_step_index(), 2);
        assert_eq!(tracker.current_step_name(), Some("C"));
        assert!(tracker.is_last_step());

        // Cannot advance past last step
        assert!(!tracker.complete_and_next());
        assert_eq!(tracker.current_step_index(), 2);

        assert!(tracker.prev_step());
        assert_eq!(tracker.current_step_index(), 1);

        tracker.reset();
        assert_eq!(tracker.current_step_index(), 0);
        assert!(!tracker.is_step_completed(0));
    }
}
