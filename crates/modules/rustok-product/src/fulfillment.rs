pub use rustok_product_ports::dto::ProductFulfillmentRequirement;

#[cfg(test)]
mod tests {
    use super::ProductFulfillmentRequirement;

    #[test]
    fn digital_product_type_is_the_only_current_digital_kind() {
        assert_eq!(
            ProductFulfillmentRequirement::from_product_type(Some("Digital")),
            ProductFulfillmentRequirement::Digital
        );
        assert_eq!(
            ProductFulfillmentRequirement::from_product_type(Some(" digital ")),
            ProductFulfillmentRequirement::Digital
        );
        assert_eq!(
            ProductFulfillmentRequirement::from_product_type(Some("physical")),
            ProductFulfillmentRequirement::Physical
        );
        assert_eq!(
            ProductFulfillmentRequirement::from_product_type(None),
            ProductFulfillmentRequirement::Physical
        );
    }
}
