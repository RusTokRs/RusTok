//! Progress tracking for batch operations.

use std::sync::Arc;

/// Callback for reporting progress during batch operations.
///
/// # Arguments
///
/// * `current` — Current item index (1-based).
/// * `total` — Total number of items.
///
/// # Example
///
/// ```rust,ignore
/// let callback = ProgressCallback::new(|current, total| {
///     println!("Progress: {}/{} ({:.1}%)", current, total, (current as f64 / total as f64) * 100.0);
/// });
/// ```
pub type ProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

/// Progress reporter that can be cloned and shared across threads.
#[derive(Clone)]
pub struct ProgressReporter {
    callback: Option<ProgressCallback>,
}

impl ProgressReporter {
    pub fn new(callback: Option<ProgressCallback>) -> Self {
        Self { callback }
    }

    pub fn report(&self, current: usize, total: usize) {
        if let Some(ref callback) = self.callback {
            callback(current, total);
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.callback.is_some()
    }
}

impl Default for ProgressReporter {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn progress_reporter_calls_callback() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let callback = Arc::new(move |current: usize, _total: usize| {
            counter_clone.store(current, Ordering::SeqCst);
        });

        let reporter = ProgressReporter::new(Some(callback));
        reporter.report(5, 10);

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn progress_reporter_handles_no_callback() {
        let reporter = ProgressReporter::new(None);
        reporter.report(5, 10); // Should not panic
        assert!(!reporter.is_enabled());
    }
}
