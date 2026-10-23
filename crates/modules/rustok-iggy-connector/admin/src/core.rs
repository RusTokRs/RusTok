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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_mode() {
        assert!(is_known_mode("bundled"));
        assert!(is_known_mode("external"));
        assert!(!is_known_mode("unknown"));
        assert!(!is_known_mode(""));
    }

    #[test]
    fn test_parse_addresses_empty_and_whitespace() {
        assert!(parse_addresses("").is_empty());
        assert!(parse_addresses("   \n  \t  \n ").is_empty());
    }

    #[test]
    fn test_parse_addresses_single_and_comma_separated() {
        let addresses = parse_addresses("127.0.0.1:8090");
        assert_eq!(addresses, vec!["127.0.0.1:8090"]);

        let addresses = parse_addresses("127.0.0.1:8090, 192.168.1.1:8090");
        assert_eq!(addresses, vec!["127.0.0.1:8090", "192.168.1.1:8090"]);
    }

    #[test]
    fn test_parse_addresses_multiline_mixed() {
        let input = "127.0.0.1:8090, 10.0.0.1:8090\n\n192.168.1.5:8090,  ";
        let addresses = parse_addresses(input);
        assert_eq!(
            addresses,
            vec!["127.0.0.1:8090", "10.0.0.1:8090", "192.168.1.5:8090"]
        );
    }
}
