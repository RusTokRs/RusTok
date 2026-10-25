use std::{env, time::Duration};

use rustok_iggy::{
    IggyDedupRecoveryWindowPolicy, IggyDedupRecoveryWindowStatus, IggyDeduplicationConfiguration,
};

const SKIP_MESSAGE: &str =
    "skipping Iggy dedup recovery-window retained calibration: required environment is absent";

const PUBLICATION_LEASE_MS: &str = "RUSTOK_IGGY_DEDUP_RECOVERY_PUBLICATION_LEASE_MS";
const PROCESS_RESTART_MS: &str = "RUSTOK_IGGY_DEDUP_RECOVERY_PROCESS_RESTART_MS";
const TRANSPORT_RECONNECT_MS: &str = "RUSTOK_IGGY_DEDUP_RECOVERY_TRANSPORT_RECONNECT_MS";
const OPERATOR_RECOVERY_MS: &str = "RUSTOK_IGGY_DEDUP_RECOVERY_OPERATOR_RECOVERY_MS";
const REQUIRED_MAX_ENTRIES: &str = "RUSTOK_IGGY_DEDUP_RECOVERY_REQUIRED_MAX_ENTRIES_PER_PARTITION";
const CONFIGURED_MAX_ENTRIES: &str = "RUSTOK_IGGY_DEDUP_RECOVERY_CONFIGURED_MAX_ENTRIES";
const CONFIGURED_EXPIRY_MS: &str = "RUSTOK_IGGY_DEDUP_RECOVERY_CONFIGURED_EXPIRY_MS";

fn required_environment() -> Option<Vec<(&'static str, String)>> {
    let names = [
        PUBLICATION_LEASE_MS,
        PROCESS_RESTART_MS,
        TRANSPORT_RECONNECT_MS,
        OPERATOR_RECOVERY_MS,
        REQUIRED_MAX_ENTRIES,
        CONFIGURED_MAX_ENTRIES,
        CONFIGURED_EXPIRY_MS,
    ];
    let values = names
        .iter()
        .map(|name| (*name, env::var(name).ok()))
        .collect::<Vec<_>>();

    if values.iter().all(|(_, value)| value.is_none()) {
        return None;
    }

    Some(
        values
            .into_iter()
            .map(|(name, value)| {
                (
                    name,
                    value.unwrap_or_else(|| {
                        panic!("{name} must be set when retained calibration is requested")
                    }),
                )
            })
            .collect(),
    )
}

fn parse_u64(values: &[(&str, String)], name: &str, allow_zero: bool) -> u64 {
    let raw = values
        .iter()
        .find_map(|(candidate, value)| (*candidate == name).then_some(value.as_str()))
        .expect("required retained calibration environment must exist");
    assert_eq!(
        raw.trim(),
        raw,
        "{name} must not contain surrounding whitespace"
    );
    let parsed = raw
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{name} must be an unsigned integer"));
    assert!(allow_zero || parsed > 0, "{name} must be positive");
    parsed
}

#[test]
fn reviewed_configuration_covers_recovery_window() {
    let Some(values) = required_environment() else {
        eprintln!("{SKIP_MESSAGE}");
        return;
    };

    let publication_lease_ms = parse_u64(&values, PUBLICATION_LEASE_MS, false);
    let process_restart_ms = parse_u64(&values, PROCESS_RESTART_MS, true);
    let transport_reconnect_ms = parse_u64(&values, TRANSPORT_RECONNECT_MS, true);
    let operator_recovery_ms = parse_u64(&values, OPERATOR_RECOVERY_MS, true);
    let required_max_entries = parse_u64(&values, REQUIRED_MAX_ENTRIES, false);
    let configured_max_entries = parse_u64(&values, CONFIGURED_MAX_ENTRIES, false);
    let configured_expiry_ms = parse_u64(&values, CONFIGURED_EXPIRY_MS, false);

    let policy = IggyDedupRecoveryWindowPolicy::new(
        Duration::from_millis(publication_lease_ms),
        Duration::from_millis(process_restart_ms),
        Duration::from_millis(transport_reconnect_ms),
        Duration::from_millis(operator_recovery_ms),
        required_max_entries,
    )
    .expect("reviewed recovery-window bounds must be valid");
    let configuration = IggyDeduplicationConfiguration::enabled(
        configured_max_entries,
        Duration::from_millis(configured_expiry_ms),
    )
    .expect("reviewed Iggy deduplication configuration must be valid");
    let assessment = policy.assess(configuration);

    assert_eq!(
        assessment.status(),
        IggyDedupRecoveryWindowStatus::Sufficient,
        "reviewed Iggy deduplication configuration does not cover the supplied recovery model"
    );

    println!(
        "RUSTOK_DEDUP_RECOVERY_CALIBRATION status={} required_expiry_ms={} configured_expiry_ms={} required_max_entries_per_partition={} configured_max_entries={}",
        assessment.status().stable_code(),
        assessment.required_expiry().as_millis(),
        assessment
            .configured_expiry()
            .expect("enabled configuration retains expiry")
            .as_millis(),
        assessment.required_max_entries_per_partition(),
        assessment
            .configured_max_entries()
            .expect("enabled configuration retains max_entries"),
    );
}
