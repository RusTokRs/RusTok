use super::super::query_error_boundary::BoundaryError;

pub(crate) mod error {
    use ::uuid::Uuid;

    use super::BoundaryError;

    #[derive(Clone, Debug)]
    pub enum FulfillmentError {
        FulfillmentNotFound(Uuid),
        Public(BoundaryError),
    }

    impl FulfillmentError {
        #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
        pub(crate) fn to_string(self) -> BoundaryError {
            match self {
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

mod fulfillment_query_boundary;
mod fulfillment_query_service;

pub(crate) use ::rustok_fulfillment::{
    ListAllShippingOptionProjectionsRequest, ListShippingOptionProjectionsRequest,
    ReadShippingOptionProjectionRequest,
};
pub(crate) use fulfillment_query_service::FulfillmentService;
