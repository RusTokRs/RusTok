pub const CONNECTOR_MODES: [(&str, &str, &str); 2] = [
    (
        "bundled",
        "Bundled",
        "Use the Iggy server artifact installed with this module.",
    ),
    (
        "external",
        "External",
        "Connect to an operator-managed Iggy deployment.",
    ),
];

pub fn is_known_mode(value: &str) -> bool {
    CONNECTOR_MODES.iter().any(|(mode, _, _)| *mode == value)
}

pub fn parse_addresses(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .map(ToString::to_string)
        .collect()
}
