pub fn inventory_policy_allows_backorder(inventory_policy: &str) -> bool {
    inventory_policy.eq_ignore_ascii_case("continue")
}

#[cfg(test)]
mod tests {
    use super::inventory_policy_allows_backorder;

    #[test]
    fn inventory_policy_backorder_matching_is_case_insensitive() {
        assert!(inventory_policy_allows_backorder("continue"));
        assert!(inventory_policy_allows_backorder("CONTINUE"));
        assert!(inventory_policy_allows_backorder("Continue"));
        assert!(!inventory_policy_allows_backorder("deny"));
    }
}
