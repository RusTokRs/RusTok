use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Product-owned fulfillment requirement derived from the canonical product kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductFulfillmentRequirement {
    Digital,
    Physical,
}

impl ProductFulfillmentRequirement {
    pub fn from_product_type(product_type: Option<&str>) -> Self {
        match product_type.map(str::trim) {
            Some(value) if value.eq_ignore_ascii_case("digital") => Self::Digital,
            _ => Self::Physical,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Digital => "digital",
            Self::Physical => "physical",
        }
    }
}

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
