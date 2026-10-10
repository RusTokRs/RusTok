//! Revision tracking configuration.

use crate::{ChangeSource, RevisionEvent, RetentionPolicy};

/// Builder for configuring revision tracking behavior.
#[derive(Debug, Clone)]
pub struct RevisionTracker {
    /// Whether tracking is enabled.
    pub enabled: bool,

    /// Events to track.
    pub track_on: Vec<RevisionEvent>,

    /// Retention policy for this content.
    pub retention: RetentionPolicy,

    /// Source of changes.
    pub source: ChangeSource,

    /// Whether to create snapshots automatically.
    pub auto_snapshot: bool,

    /// Interval for automatic snapshots (in revisions).
    pub snapshot_interval: Option<u32>,
}

impl RevisionTracker {
    /// Create a new builder with default settings.
    pub fn builder() -> RevisionTrackerBuilder {
        RevisionTrackerBuilder::default()
    }
}

/// Builder for RevisionTracker.
#[derive(Debug, Default)]
pub struct RevisionTrackerBuilder {
    enabled: Option<bool>,
    track_on: Option<Vec<RevisionEvent>>,
    retention: Option<RetentionPolicy>,
    source: Option<ChangeSource>,
    auto_snapshot: Option<bool>,
    snapshot_interval: Option<u32>,
}

impl RevisionTrackerBuilder {
    /// Enable or disable tracking.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Set which events to track.
    pub fn track_on(mut self, events: Vec<RevisionEvent>) -> Self {
        self.track_on = Some(events);
        self
    }

    /// Set the retention policy.
    pub fn retention(mut self, policy: RetentionPolicy) -> Self {
        self.retention = Some(policy);
        self
    }

    /// Set the change source.
    pub fn source(mut self, source: ChangeSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Enable or disable automatic snapshots.
    pub fn auto_snapshot(mut self, enabled: bool) -> Self {
        self.auto_snapshot = Some(enabled);
        self
    }

    /// Set the snapshot interval.
    pub fn snapshot_interval(mut self, interval: u32) -> Self {
        self.snapshot_interval = Some(interval);
        self
    }

    /// Build the RevisionTracker.
    pub fn build(self) -> RevisionTracker {
        RevisionTracker {
            enabled: self.enabled.unwrap_or(true),
            track_on: self.track_on.unwrap_or_else(|| {
                vec![
                    RevisionEvent::Create,
                    RevisionEvent::Update,
                    RevisionEvent::Delete,
                ]
            }),
            retention: self.retention.unwrap_or(RetentionPolicy::KeepLast(100)),
            source: self.source.unwrap_or(ChangeSource::Web),
            auto_snapshot: self.auto_snapshot.unwrap_or(false),
            snapshot_interval: self.snapshot_interval,
        }
    }
}
