use super::super::query_error_boundary::BoundaryError;

pub(crate) mod error {
    use ::uuid::Uuid;

    use super::BoundaryError;

    #[derive(Clone, Debug)]
    pub enum FulfillmentError {
        ShippingOptionNotFound(Uuid),
        FulfillmentNotFound(Uuid),
        Public(BoundaryError),
    }

    impl FulfillmentError {
        #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
        pub(crate) fn to_string(self) -> BoundaryError {
            match self {
                Self::ShippingOptionNotFound(id) => {
                    tracing::debug!(shipping_option_id = %id, "shipping option not found");
                    BoundaryError::Public {
                        message: "Fulfillment resource was not found",
                        code: "FULFILLMENT_RESOURCE_NOT_FOUND",
                        retryable: false,
                    }
                }
                Self::FulfillmentNotFound(id) => {
                    tracing::debug!(fulfillment_id = %id, "fulfillment not found");
                    BoundaryError::Public {
                        message: "Fulfillment resource was not found",
                        code: "FULFILLMENT_RESOURCE_NOT_FOUND",
                        retryable: false,
                    }
                }
                Self::Public(error) => error,
            }
        }
    }
}

use error::FulfillmentError;

pub type FulfillmentResult<T> = Result<T, FulfillmentError>;

const GRAPHQL_QUERY_FULFILLMENT_BOUNDARY: &str = "commerce_graphql_query_fulfillment_facade";

pub(crate) struct ShippingOptionAdminQueryError(BoundaryError);

impl ShippingOptionAdminQueryError {
    #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
    pub(crate) fn to_string(self) -> BoundaryError {
        self.0
    }
}

mod fulfillment_query_boundary;
mod fulfillment_query_service;

pub(crate) use fulfillment_query_service::FulfillmentService;
