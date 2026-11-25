/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 * You may not remove or alter this copyright notice or license header.
 */

//! Content-free observations of the existing JWT authority; never a grant or ACK.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{FenceMac, JwtGenerationFence, unavailable};
use crate::{JwtKeyringPublisher, JwtLeaseReadiness, error::Result};

const OUTCOMES: &[(&str, &str)] = &[
    ("renew", "granted"), ("renew", "denied"), ("renew", "error"),
    ("activate", "applied"), ("activate", "denied"), ("activate", "error"),
    ("grant", "revoked"), ("grant", "not_admitted"), ("grant", "draining"),
    ("grant", "capacity"), ("grant", "contention"), ("grant", "static_conflict"),
    ("admit", "admitted"), ("admit", "draining"), ("admit", "awaiting_replicas"),
    ("admit", "rejected"), ("admit", "error"),
    ("ack", "complete"), ("ack", "deferred"), ("ack", "error"),
    ("reload", "source_unavailable"), ("reload", "rejected"),
    ("reload", "waiting_drain"), ("reload", "waiting_replicas"),
    ("reload", "waiting_grant"), ("reload", "published"),
];

/// Observed file-supervisor outcome, not a durable admission decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JwtReloadOutcome {
    Unchanged,
    SourceUnavailable,
    Rejected,
    WaitingDrain(u64),
    WaitingReplicas(u64),
    WaitingGrant(u64),
    Published,
}

impl JwtReloadOutcome {
    fn label(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::SourceUnavailable => "source_unavailable",
            Self::Rejected => "rejected",
            Self::WaitingDrain(_) => "waiting_drain",
            Self::WaitingReplicas(_) => "waiting_replicas",
            Self::WaitingGrant(_) => "waiting_grant",
            Self::Published => "published",
        }
    }

    fn pending(self) -> Option<u64> {
        match self {
            Self::WaitingDrain(generation) | Self::WaitingReplicas(generation)
            | Self::WaitingGrant(generation) => Some(generation),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct OperationMetrics(Arc<Mutex<OperationState>>);

#[derive(Default)]
struct OperationState {
    counts: BTreeMap<(&'static str, &'static str), u64>,
    ack_count: u64,
    ack_seconds_sum: f64,
    pending: Option<(u64, Instant)>,
}

impl OperationState {
    fn record(&mut self, operation: &'static str, outcome: &'static str) {
        let count = self.counts.entry((operation, outcome)).or_default();
        *count = count.saturating_add(1);
    }
}

impl OperationMetrics {
    pub(super) fn record(&self, operation: &'static str, outcome: &'static str) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .record(operation, outcome);
    }

    pub(super) fn record_ack(&self, elapsed: Duration, outcome: &'static str) {
        let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.record("ack", outcome);
        state.ack_count = state.ack_count.saturating_add(1);
        state.ack_seconds_sum += elapsed.as_secs_f64();
    }

    fn reload(&self, outcome: JwtReloadOutcome) {
        let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if outcome != JwtReloadOutcome::Unchanged {
            state.record("reload", outcome.label());
        }
        state.pending = match (state.pending, outcome.pending()) {
            (Some((old, since)), Some(next)) if old == next => Some((old, since)),
            (_, Some(next)) => Some((next, Instant::now())),
            _ => None,
        };
    }
}

/// Aggregate observation of an authenticated authority row and its
/// conservative outstanding grant registry. Not a membership/health proof.
pub struct JwtClusterObservation {
    generation: i64,
    current_grants: usize,
    stale_grants: usize,
    current_replicas: usize,
    stale_replicas: usize,
    revoked_grants: usize,
    revoked_bindings: usize,
    current_revoked: bool,
    drain_seconds: f64,
    untracked_seconds: f64,
    lease_horizon_seconds: f64,
}

/// Read-only host handle. No method exposes secrets, identities or grant authority.
#[derive(Clone)]
pub struct JwtOperationalMetrics {
    fence: JwtGenerationFence,
    mac: Arc<FenceMac>,
    local: JwtLeaseReadiness,
}

impl JwtGenerationFence {
    pub fn operational_metrics(&self, publisher: &JwtKeyringPublisher) -> Result<JwtOperationalMetrics> {
        Ok(JwtOperationalMetrics {
            fence: self.clone(),
            mac: Arc::new(self.checked_mac(publisher.lease_root())?),
            local: publisher.lease_readiness()?,
        })
    }

    /// Only the trusted host supervisor reports file-processing outcomes.
    pub fn record_reload(&self, outcome: JwtReloadOutcome) {
        self.metrics.reload(outcome);
    }
}

impl JwtOperationalMetrics {
    /// Read the authority without modifying it or renewing a lease.
    pub async fn cluster_snapshot(&self) -> Result<JwtClusterObservation> {
        let stored = self.fence.load().await?;
        let ledger = stored.ledger(&self.mac)?;
        let grants = stored.live_grants()?;
        let mut current_replicas = BTreeSet::new();
        let mut stale_replicas = BTreeSet::new();
        let mut current_grants = 0;
        let mut stale_grants = 0;
        let mut revoked_grants = 0;
        for (key, grant) in &grants {
            let (incarnation, generation) = key.split_once(':').ok_or_else(unavailable)?;
            let incarnation = uuid::Uuid::parse_str(incarnation).map_err(|_| unavailable())?;
            if generation.parse::<u64>().ok() != Some(grant.generation) {
                return Err(unavailable());
            }
            if i64::try_from(grant.generation).ok() == Some(stored.generation) {
                current_grants += 1;
                current_replicas.insert(incarnation);
            } else {
                stale_grants += 1;
                stale_replicas.insert(incarnation);
            }
            if ledger.as_ref().is_some_and(|ledger| ledger.contains_revoked(&grant.snapshot)) {
                revoked_grants += 1;
            }
        }
        let remaining = |deadline: i64| deadline.saturating_sub(stored.now_ms).max(0) as f64 / 1000.0;
        Ok(JwtClusterObservation {
            generation: stored.generation,
            current_grants,
            stale_grants,
            current_replicas: current_replicas.len(),
            stale_replicas: stale_replicas.len(),
            revoked_grants,
            revoked_bindings: ledger.as_ref().map_or(0, |ledger| ledger.seen.values()
                .flat_map(|keys| keys.values()).filter(|binding| binding.revoked_at_ms.is_some()).count()),
            current_revoked: ledger.as_ref().is_some_and(|ledger| ledger.contains_revoked(&ledger.current)),
            drain_seconds: remaining(stored.drain_until_ms),
            untracked_seconds: remaining(stored.untracked_until_ms),
            lease_horizon_seconds: remaining(stored.lease_until_ms),
        })
    }

    /// Sample the actual local gate after awaited DB IO. Unknown cluster data is NaN.
    pub fn render(&self, cluster: Option<&JwtClusterObservation>) -> String {
        let mut output = String::new();
        let local = self.local.operational_state();
        let state = self.fence.metrics.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        macro_rules! gauge {
            ($name:literal, $value:expr) => {
                writeln!(output, concat!("# TYPE rustok_auth_jwt_", $name, " gauge\nrustok_auth_jwt_", $name, " {}"), $value)
                    .expect("writing to String cannot fail")
            };
        }
        gauge!("collection_status", u8::from(cluster.is_some()));
        gauge!("independent_authority", u8::from(self.fence.binding.is_some()));
        gauge!("local_state_available", u8::from(local.is_some()));
        gauge!("local_generation", local.map_or(f64::NAN, |s| s.0 as f64));
        gauge!("local_ready", u8::from(local.is_some_and(|s| s.1)));
        gauge!("local_emergency_fenced", local.map_or(f64::NAN, |s| u8::from(s.2) as f64));
        gauge!("local_lease_remaining_seconds", local.map_or(f64::NAN, |s| s.3));
        gauge!("local_clock_fenced", local.map_or(f64::NAN, |s| u8::from(s.4) as f64));
        gauge!("authority_clock_fenced", u8::from(self.fence.clock.faulted()));
        gauge!("durable_generation", cluster.map_or(f64::NAN, |s| s.generation as f64));
        gauge!("generation_lag", cluster.zip(local).map_or(f64::NAN, |(s, l)|
            (i128::from(s.generation) - i128::from(l.0)) as f64));
        gauge!("current_grants", cluster.map_or(f64::NAN, |s| s.current_grants as f64));
        gauge!("stale_grants", cluster.map_or(f64::NAN, |s| s.stale_grants as f64));
        gauge!("current_grant_replicas", cluster.map_or(f64::NAN, |s| s.current_replicas as f64));
        gauge!("stale_grant_replicas", cluster.map_or(f64::NAN, |s| s.stale_replicas as f64));
        gauge!("revoked_grants", cluster.map_or(f64::NAN, |s| s.revoked_grants as f64));
        gauge!("revoked_bindings", cluster.map_or(f64::NAN, |s| s.revoked_bindings as f64));
        gauge!("current_revoked", cluster.map_or(f64::NAN, |s| u8::from(s.current_revoked) as f64));
        gauge!("drain_remaining_seconds", cluster.map_or(f64::NAN, |s| s.drain_seconds));
        gauge!("untracked_remaining_seconds", cluster.map_or(f64::NAN, |s| s.untracked_seconds));
        gauge!("lease_horizon_remaining_seconds", cluster.map_or(f64::NAN, |s| s.lease_horizon_seconds));
        gauge!("pending_generation", state.pending.map_or(f64::NAN, |(g, _)| g as f64));
        gauge!("pending_seconds", state.pending.map_or(0.0, |(_, since)| since.elapsed().as_secs_f64()));
        output.push_str("# TYPE rustok_auth_jwt_operations_total counter\n");
        for &(operation, outcome) in OUTCOMES {
            let count = state.counts.get(&(operation, outcome)).copied().unwrap_or(0);
            writeln!(output, "rustok_auth_jwt_operations_total{{operation=\"{operation}\",outcome=\"{outcome}\"}} {count}")
                .expect("writing to String cannot fail");
        }
        writeln!(output, "# TYPE rustok_auth_jwt_ack_attempt_duration_seconds summary\nrustok_auth_jwt_ack_attempt_duration_seconds_sum {}\nrustok_auth_jwt_ack_attempt_duration_seconds_count {}",
            state.ack_seconds_sum, state.ack_count).expect("writing to String cannot fail");
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::{ROOT, OLD, NEW, config, database};
    use super::super::JwtAdmission;

    #[tokio::test]
    async fn observations_distinguish_grants_revocation_and_local_gate() {
        let fence = JwtGenerationFence::new(database().await);
        let first = config(ROOT, "private-kid", OLD, None, None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (_, publisher) = first.clone().into_cluster_jwt(1).unwrap();
        let (_, other) = first.into_cluster_jwt(1).unwrap();
        let metrics = fence.operational_metrics(&publisher).unwrap();
        assert!(fence.renew(&publisher).await.unwrap());
        assert!(fence.renew(&other).await.unwrap());
        let staged = config(ROOT, "private-kid", OLD,
            Some(("staged-kid", NEW, chrono::Utc::now().timestamp() + 3600)), None);
        assert_eq!(fence.admit(2, &staged).await.unwrap(), JwtAdmission::Admitted);
        assert!(fence.activate(&publisher, 2, staged).await.unwrap());
        let cluster = metrics.cluster_snapshot().await.unwrap();
        assert_eq!((cluster.generation, cluster.current_grants, cluster.stale_grants), (2, 1, 1));
        assert_eq!((cluster.current_replicas, cluster.stale_replicas), (1, 1));
        let rendered = metrics.render(Some(&cluster));
        assert!(rendered.contains("rustok_auth_jwt_local_ready 1\n"));
        assert!(rendered.contains("rustok_auth_jwt_ack_attempt_duration_seconds_count 1\n"));
        assert!(!rendered.contains("private-kid"));
        assert!(!rendered.contains(ROOT));
        assert!(!rendered.contains(&publisher.incarnation().to_string()));
        let other_fence = JwtGenerationFence::new(fence.db.clone());
        other_fence.revoke_key(ROOT, "access", "private-kid").await.unwrap();
        let cluster = metrics.cluster_snapshot().await.unwrap();
        assert_eq!(cluster.revoked_bindings, 1);
        assert_eq!(cluster.revoked_grants, 2);
        assert!(cluster.current_revoked);
        assert!(!fence.renew(&publisher).await.unwrap());
        let rendered = metrics.render(Some(&cluster));
        assert!(rendered.contains("rustok_auth_jwt_local_ready 0\n"));
        assert!(rendered.contains("rustok_auth_jwt_local_emergency_fenced 1\n"));
        assert!(rendered.contains("operation=\"grant\",outcome=\"revoked\"} 1\n"));
        let unknown = metrics.render(None);
        assert!(unknown.contains("rustok_auth_jwt_collection_status 0\n"));
        assert!(unknown.contains("rustok_auth_jwt_durable_generation NaN\n"));
        assert!(unknown.contains("rustok_auth_jwt_local_ready 0\n"));
    }

    #[tokio::test]
    async fn authority_clock_fault_closes_active_gate_and_preserves_emergency_revocation() {
        let fence = JwtGenerationFence::new(database().await);
        let first = config(ROOT, "clock-key", OLD, None, None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (_, publisher) = first.clone().into_cluster_jwt(1).unwrap();
        let metrics = fence.operational_metrics(&publisher).unwrap();
        assert!(fence.renew(&publisher).await.unwrap());
        assert!(publisher.lease_readiness().unwrap().is_ready());

        fence.clock.trip();
        assert!(metrics.cluster_snapshot().await.is_err());
        let fault = metrics.render(None);
        assert!(fault.contains("rustok_auth_jwt_authority_clock_fenced 1\n"));
        assert!(fault.contains("rustok_auth_jwt_local_clock_fenced 1\n"));
        assert!(fault.contains("rustok_auth_jwt_local_ready 0\n"));
        assert!(!publisher.lease_readiness().unwrap().is_ready());
        assert!(fence.renew(&publisher).await.is_err());
        assert!(fence.admit(1, &first).await.is_err());
        assert!(fence.revoke_key(ROOT, "access", "clock-key").await.unwrap().newly_revoked);
    }

    #[test]
    fn pending_timer_survives_retry_and_resets_on_new_candidate_or_abandonment() {
        let metrics = OperationMetrics::default();
        metrics.reload(JwtReloadOutcome::WaitingReplicas(2));
        let since = metrics.0.lock().unwrap().pending.unwrap().1;
        metrics.reload(JwtReloadOutcome::WaitingGrant(2));
        assert_eq!(metrics.0.lock().unwrap().pending, Some((2, since)));
        metrics.reload(JwtReloadOutcome::WaitingDrain(3));
        assert_eq!(metrics.0.lock().unwrap().pending.unwrap().0, 3);
        metrics.reload(JwtReloadOutcome::SourceUnavailable);
        assert!(metrics.0.lock().unwrap().pending.is_none());
        metrics.reload(JwtReloadOutcome::WaitingGrant(3));
        metrics.reload(JwtReloadOutcome::Published);
        assert!(metrics.0.lock().unwrap().pending.is_none());
    }
}
