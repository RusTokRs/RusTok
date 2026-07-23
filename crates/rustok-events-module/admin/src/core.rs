pub const DELIVERY_PROFILES: [(&str, &str, &str); 3] = [
    (
        "memory",
        "Memory",
        "Process-local delivery for development, tests, blogs, and simple deployments.",
    ),
    (
        "outbox_local",
        "Outbox",
        "Transactional database outbox with a lightweight in-process relay.",
    ),
    (
        "outbox_iggy",
        "Outbox + Iggy",
        "Transactional outbox with Iggy relay for high-throughput workloads.",
    ),
];

pub fn is_known_profile(value: &str) -> bool {
    DELIVERY_PROFILES
        .iter()
        .any(|(profile, _, _)| *profile == value)
}
